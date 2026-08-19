param(
  [string]$CommandPath = 'cargo',
  [string[]]$CommandArguments = @('test', '--all-targets', '--manifest-path', 'src-tauri/Cargo.toml'),
  [string]$CommandArgumentsBase64 = '',
  [ValidateRange(1, 7200)][int]$TimeoutSeconds = 3600,
  [ValidateRange(20, 5000)][int]$PollMilliseconds = 100
)

$ErrorActionPreference = 'Stop'
Set-StrictMode -Version Latest
. (Join-Path $PSScriptRoot 'windows-job-object.ps1')

$resolvedCommand = Get-Command $CommandPath -ErrorAction Stop
if (-not $resolvedCommand.Path -or -not (Test-Path -LiteralPath $resolvedCommand.Path -PathType Leaf)) {
  throw "Could not resolve executable '$CommandPath'."
}
if ($CommandArgumentsBase64) {
  try {
    $decodedArguments = [Text.Encoding]::UTF8.GetString([Convert]::FromBase64String($CommandArgumentsBase64))
    $CommandArguments = [string[]]($decodedArguments | ConvertFrom-Json)
  }
  catch {
    throw 'CommandArgumentsBase64 must contain a Base64-encoded JSON string array.'
  }
}

$runId = [guid]::NewGuid().ToString('N')
$controlPrefix = "__MISTAKE_TRAINER_WARNING_COUNTS_$runId`__"
$runRoot = Join-Path ([IO.Path]::GetTempPath()) "mistake-trainer-rust-test-$runId"
$wrapperPath = Join-Path $runRoot 'invoke.ps1'
$outputPath = Join-Path $runRoot 'output.log'
$exitPath = Join-Path $runRoot 'exit-code.txt'
$job = $null
$process = $null
$reader = $null
$timedOut = $false
$exitCode = 1
$virtualLockCount = 0
$opensslPdbCount = 0

function Write-FilteredRustLine {
  param([AllowEmptyString()][string]$Line)
  if ($Line.StartsWith("$script:controlPrefix|", [StringComparison]::Ordinal)) {
    $counts = $Line.Substring($script:controlPrefix.Length + 1).Split('|')
    if ($counts.Length -eq 2) {
      $script:virtualLockCount = [long]$counts[0]
      $script:opensslPdbCount = [long]$counts[1]
    }
    return
  }
  if ($Line -match 'sqlcipher_mlock: VirtualLock\(\) returned 0 LastError=1453') {
    $script:virtualLockCount += 1
    if ($script:virtualLockCount -eq 1) { Write-Output $Line }
    return
  }
  if ($Line -match "warning LNK4099: PDB 'ossl_static\.pdb' was not found") {
    $script:opensslPdbCount += 1
    if ($script:opensslPdbCount -eq 1) { Write-Output $Line }
    return
  }
  Write-Output $Line
}

function Read-AvailableRustOutput {
  if (-not $script:reader -and (Test-Path -LiteralPath $script:outputPath -PathType Leaf)) {
    $stream = [IO.File]::Open($script:outputPath, [IO.FileMode]::Open, [IO.FileAccess]::Read, [IO.FileShare]::ReadWrite)
    $script:reader = [IO.StreamReader]::new($stream, [Text.Encoding]::UTF8, $true)
  }
  if (-not $script:reader) { return }
  while ($null -ne ($line = $script:reader.ReadLine())) {
    Write-FilteredRustLine -Line $line
  }
}

New-Item -ItemType Directory -Path $runRoot | Out-Null
try {
  $wrapper = @'
param([string]$TargetPath, [string]$ArgumentsBase64, [string]$OutputPath, [string]$ExitPath, [string]$ControlPrefix)
$ErrorActionPreference = 'Continue'
$exitCode = 1
$stream = $null
$writer = $null
$virtualLockCount = 0L
$opensslPdbCount = 0L
try {
  $argumentsJson = [Text.Encoding]::UTF8.GetString([Convert]::FromBase64String($ArgumentsBase64))
  $arguments = [string[]]($argumentsJson | ConvertFrom-Json)
  $stream = [IO.File]::Open($OutputPath, [IO.FileMode]::CreateNew, [IO.FileAccess]::Write, [IO.FileShare]::ReadWrite)
  $writer = [IO.StreamWriter]::new($stream, [Text.UTF8Encoding]::new($false))
  $writer.AutoFlush = $true
  & $TargetPath @arguments 2>&1 | ForEach-Object {
    $line = $_.ToString()
    if ($line -match 'sqlcipher_mlock: VirtualLock\(\) returned 0 LastError=1453') {
      $virtualLockCount += 1
      if ($virtualLockCount -eq 1) { $writer.WriteLine($line) }
    }
    elseif ($line -match "warning LNK4099: PDB 'ossl_static\.pdb' was not found") {
      $opensslPdbCount += 1
      if ($opensslPdbCount -eq 1) { $writer.WriteLine($line) }
    }
    else {
      $writer.WriteLine($line)
    }
  }
  $exitCode = $LASTEXITCODE
  if ($null -eq $exitCode) { $exitCode = if ($?) { 0 } else { 1 } }
}
catch {
  if ($writer) { $writer.WriteLine($_.Exception.ToString()) }
  $exitCode = 1
}
finally {
  if ($writer) {
    $writer.WriteLine("$ControlPrefix|$virtualLockCount|$opensslPdbCount")
    $writer.Dispose()
  }
  elseif ($stream) { $stream.Dispose() }
  Set-Content -LiteralPath $ExitPath -Value $exitCode -Encoding ASCII
}
exit $exitCode
'@
  Set-Content -LiteralPath $wrapperPath -Value $wrapper -Encoding UTF8
  $argumentsJson = ConvertTo-Json -InputObject @($CommandArguments) -Compress
  $argumentsBase64 = [Convert]::ToBase64String([Text.Encoding]::UTF8.GetBytes($argumentsJson))
  $hostExecutable = (Get-Process -Id $PID).Path
  if (-not $hostExecutable -or -not (Test-Path -LiteralPath $hostExecutable -PathType Leaf)) {
    throw 'Could not resolve the current PowerShell executable.'
  }
  $job = New-KillOnCloseJob
  $process = Start-ProcessInJob -Job $job -FilePath $hostExecutable -ArgumentList @(
    '-NoProfile', '-ExecutionPolicy', 'Bypass', '-File', $wrapperPath,
    $resolvedCommand.Path, $argumentsBase64, $outputPath, $exitPath, $controlPrefix
  )
  $deadline = [DateTime]::UtcNow.AddSeconds($TimeoutSeconds)
  while (-not $process.HasExited -and [DateTime]::UtcNow -lt $deadline) {
    Read-AvailableRustOutput
    Start-Sleep -Milliseconds $PollMilliseconds
    $process.Refresh()
  }
  if (-not $process.HasExited) {
    $timedOut = $true
    Close-KillOnCloseJob -Job $job
    [void]$process.WaitForExit(5000)
  }
  else {
    [void]$process.WaitForExit()
  }
  Read-AvailableRustOutput
  if ($timedOut) {
    $exitCode = 124
    Write-Output "Rust test command timed out after $TimeoutSeconds seconds; its owned process tree was terminated."
  }
  elseif (Test-Path -LiteralPath $exitPath -PathType Leaf) {
    $exitCode = [int](Get-Content -LiteralPath $exitPath -Raw)
  }
  else {
    $exitCode = $process.ExitCode
  }
}
finally {
  if ($reader) { $reader.Dispose() }
  if ($job) { Close-KillOnCloseJob -Job $job }
  if (Test-Path -LiteralPath $runRoot -PathType Container) {
    Remove-Item -LiteralPath $runRoot -Recurse -Force -ErrorAction SilentlyContinue
  }
}

if ($virtualLockCount -gt 0) {
  Write-Output "SQLCipher VirtualLock 1453 warnings: total=$virtualLockCount; shown=1; suppressed=$($virtualLockCount - 1)."
}
if ($opensslPdbCount -gt 0) {
  Write-Output "OpenSSL ossl_static.pdb LNK4099 warnings: total=$opensslPdbCount; shown=1; suppressed=$($opensslPdbCount - 1)."
}
Write-Output "Rust test command exit code: $exitCode."
exit $exitCode
