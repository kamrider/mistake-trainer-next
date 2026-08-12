$ErrorActionPreference = 'Stop'
$testRoot = Join-Path ([IO.Path]::GetTempPath()) "mistake-trainer-job-test-$([guid]::NewGuid().ToString('N'))"
$ownerScript = Join-Path $testRoot 'owner.ps1'
$childPidPath = Join-Path $testRoot 'child.pid'
$ownerStdoutPath = Join-Path $testRoot 'owner.stdout.log'
$ownerStderrPath = Join-Path $testRoot 'owner.stderr.log'
$owner = $null
New-Item -ItemType Directory -Path $testRoot | Out-Null
try {
  $hostExecutable = (Get-Process -Id $PID).Path
  if (-not $hostExecutable -or -not (Test-Path -LiteralPath $hostExecutable -PathType Leaf)) {
    throw 'Could not resolve the current PowerShell executable.'
  }
  $escapedHostExecutable = $hostExecutable.Replace("'", "''")
  $helper = (Resolve-Path -LiteralPath (Join-Path $PSScriptRoot 'windows-job-object.ps1')).Path.Replace("'", "''")
  @"
`$ErrorActionPreference = 'Stop'
. '$helper'
`$job = New-KillOnCloseJob
`$child = Start-ProcessInJob -Job `$job -FilePath '$escapedHostExecutable' -ArgumentList @('-NoProfile','-Command','Start-Sleep -Seconds 120')
Set-Content -LiteralPath '$($childPidPath.Replace("'", "''"))' -Value `$child.Id
Start-Sleep -Seconds 120
"@ | Set-Content -LiteralPath $ownerScript -Encoding UTF8
  $owner = Start-Process -FilePath $hostExecutable -ArgumentList @('-NoProfile', '-ExecutionPolicy', 'Bypass', '-File', $ownerScript) -RedirectStandardOutput $ownerStdoutPath -RedirectStandardError $ownerStderrPath -PassThru
  $deadline = [DateTime]::UtcNow.AddSeconds(10)
  while (-not (Test-Path -LiteralPath $childPidPath -PathType Leaf) -and [DateTime]::UtcNow -lt $deadline) { Start-Sleep -Milliseconds 100 }
  if (-not (Test-Path -LiteralPath $childPidPath -PathType Leaf)) {
    $diagnostic = if (Test-Path -LiteralPath $ownerStderrPath -PathType Leaf) { (Get-Content -LiteralPath $ownerStderrPath -Raw).Trim() } else { '' }
    throw "Job owner did not create a child process. $diagnostic"
  }
  $childPid = [int](Get-Content -LiteralPath $childPidPath -Raw)
  Stop-Process -Id $owner.Id -Force
  $deadline = [DateTime]::UtcNow.AddSeconds(5)
  while (Get-Process -Id $childPid -ErrorAction SilentlyContinue) {
    if ([DateTime]::UtcNow -ge $deadline) { throw 'Kill-on-close Job Object left an orphan child.' }
    Start-Sleep -Milliseconds 100
  }
  Write-Output 'Windows Job Object interruption test passed'
}
finally {
  if ($owner -and (Get-Process -Id $owner.Id -ErrorAction SilentlyContinue)) { Stop-Process -Id $owner.Id -Force -ErrorAction SilentlyContinue }
  Remove-Item -LiteralPath $testRoot -Recurse -Force -ErrorAction SilentlyContinue
}
