$ErrorActionPreference = 'Stop'
Set-StrictMode -Version Latest

$testRoot = Join-Path ([IO.Path]::GetTempPath()) "mistake-trainer-rust-runner-test-$([guid]::NewGuid().ToString('N'))"
$fixturePath = Join-Path $testRoot 'fixture.ps1'
$timeoutFixturePath = Join-Path $testRoot 'timeout-fixture.ps1'
$childPidPath = Join-Path $testRoot 'owned-child.pid'
$outputPath = Join-Path $testRoot 'runner.stdout.log'
$errorPath = Join-Path $testRoot 'runner.stderr.log'
$runnerPath = Join-Path $PSScriptRoot 'windows-rust-test.ps1'
$hostExecutable = (Get-Process -Id $PID).Path
$unrelated = $null

function Invoke-RunnerFixture {
  param([string]$Fixture, [int]$TimeoutSeconds, [string[]]$FixtureArguments = @())
  Remove-Item -LiteralPath $outputPath, $errorPath -Force -ErrorAction SilentlyContinue
  $targetArguments = @('-NoProfile', '-ExecutionPolicy', 'Bypass', '-File', $Fixture) + $FixtureArguments
  $targetArgumentsJson = ConvertTo-Json -InputObject $targetArguments -Compress
  $targetArgumentsBase64 = [Convert]::ToBase64String([Text.Encoding]::UTF8.GetBytes($targetArgumentsJson))
  $arguments = @(
    '-NoProfile', '-ExecutionPolicy', 'Bypass', '-File', $runnerPath,
    '-CommandPath', $hostExecutable,
    '-CommandArgumentsBase64', $targetArgumentsBase64,
    '-TimeoutSeconds', $TimeoutSeconds, '-PollMilliseconds', 20
  )
  $process = Start-Process -FilePath $hostExecutable -ArgumentList $arguments -WindowStyle Hidden `
    -RedirectStandardOutput $outputPath -RedirectStandardError $errorPath -PassThru -Wait
  $process.Refresh()
  return [int]$process.ExitCode
}

New-Item -ItemType Directory -Path $testRoot | Out-Null
try {
  @'
Write-Output 'normal-output-line'
1..3 | ForEach-Object { Write-Output 'WARN MEMORY sqlcipher_mlock: VirtualLock() returned 0 LastError=1453' }
1..2 | ForEach-Object { Write-Output "thing.obj : warning LNK4099: PDB 'ossl_static.pdb' was not found" }
exit 7
'@ | Set-Content -LiteralPath $fixturePath -Encoding UTF8

  $failed = Invoke-RunnerFixture -Fixture $fixturePath -TimeoutSeconds 10
  $output = Get-Content -LiteralPath $outputPath -Raw
  if ($failed -ne 7) {
    $errorOutput = if (Test-Path -LiteralPath $errorPath) { Get-Content -LiteralPath $errorPath -Raw } else { '' }
    throw "Runner changed target exit code 7 to $failed. stdout=$output stderr=$errorOutput"
  }
  if (($output | Select-String -Pattern 'VirtualLock\(\).*LastError=1453' -AllMatches).Matches.Count -ne 1) {
    throw 'VirtualLock warnings were not deduplicated to one displayed occurrence.'
  }
  if (($output | Select-String -Pattern "warning LNK4099: PDB 'ossl_static\.pdb'" -AllMatches).Matches.Count -ne 1) {
    throw 'OpenSSL PDB warnings were not deduplicated to one displayed occurrence.'
  }
  if ($output -notmatch 'SQLCipher VirtualLock 1453 warnings: total=3; shown=1; suppressed=2\.') {
    throw 'VirtualLock warning summary was missing or incorrect.'
  }
  if ($output -notmatch 'OpenSSL ossl_static\.pdb LNK4099 warnings: total=2; shown=1; suppressed=1\.') {
    throw 'OpenSSL PDB warning summary was missing or incorrect.'
  }
  if ($output -notmatch 'normal-output-line') { throw 'Normal child output was not streamed.' }

  $escapedHost = $hostExecutable.Replace("'", "''")
  $escapedPidPath = $childPidPath.Replace("'", "''")
  @"
`$child = Start-Process -FilePath '$escapedHost' -ArgumentList @('-NoProfile','-Command','Start-Sleep -Seconds 120') -WindowStyle Hidden -PassThru
Set-Content -LiteralPath '$escapedPidPath' -Value `$child.Id
Start-Sleep -Seconds 120
"@ | Set-Content -LiteralPath $timeoutFixturePath -Encoding UTF8
  $unrelated = Start-Process -FilePath $hostExecutable -ArgumentList @('-NoProfile', '-Command', 'Start-Sleep -Seconds 120') -WindowStyle Hidden -PassThru
  $timedOut = Invoke-RunnerFixture -Fixture $timeoutFixturePath -TimeoutSeconds 2
  if ($timedOut -ne 124) { throw "Runner timeout exit code was $timedOut, expected 124." }
  $timeoutOutput = Get-Content -LiteralPath $outputPath -Raw
  if ($timeoutOutput -notmatch 'owned process tree was terminated') { throw 'Timeout summary was not emitted.' }
  if (-not (Test-Path -LiteralPath $childPidPath -PathType Leaf)) { throw 'Timeout fixture did not record its owned child.' }
  $ownedChildPid = [int](Get-Content -LiteralPath $childPidPath -Raw)
  Start-Sleep -Milliseconds 300
  if (Get-Process -Id $ownedChildPid -ErrorAction SilentlyContinue) { throw 'Timeout left an owned child process running.' }
  if (-not (Get-Process -Id $unrelated.Id -ErrorAction SilentlyContinue)) { throw 'Timeout terminated an unrelated process.' }

  $runnerSource = Get-Content -LiteralPath $runnerPath -Raw
  if ($runnerSource -match '(?i)\b(taskkill|Stop-Process)\b') { throw 'Runner contains a broad process-termination command.' }
  Write-Output 'Windows Rust test runner contract tests passed.'
}
finally {
  if ($unrelated -and (Get-Process -Id $unrelated.Id -ErrorAction SilentlyContinue)) {
    Stop-Process -Id $unrelated.Id -Force -ErrorAction SilentlyContinue
  }
  Remove-Item -LiteralPath $testRoot -Recurse -Force -ErrorAction SilentlyContinue
}
