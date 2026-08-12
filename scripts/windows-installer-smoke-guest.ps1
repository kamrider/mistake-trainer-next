$ErrorActionPreference = 'Stop'
$env:CI = 'true'
$env:MISTAKE_TRAINER_EPHEMERAL_WINDOWS = '1'
$env:RUNNER_TEMP = 'C:\SmokeTemp'
New-Item -ItemType Directory -Path $env:RUNNER_TEMP -Force | Out-Null
$configuration = Get-Content -LiteralPath 'C:\SmokeInput\guest-config.json' -Raw | ConvertFrom-Json
try {
  & 'C:\SmokeInput\windows-installer-smoke-inner.ps1' `
    -InstallerDirectory 'C:\SmokeInput\installer' `
    -ExpectedArchitecture $configuration.architecture `
    -RunId $configuration.runId `
    -ResultDirectory 'C:\SmokeResults'
}
finally {
  Start-Sleep -Seconds 2
  & "$env:WINDIR\System32\shutdown.exe" /s /t 0
}
