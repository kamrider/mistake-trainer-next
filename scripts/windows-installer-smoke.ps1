[CmdletBinding()]
param(
  [string]$InstallerDirectory = (Join-Path $PSScriptRoot '..\src-tauri\target\release\bundle\nsis'),
  [ValidateSet('x86_64', 'arm64')]
  [string]$ExpectedArchitecture = 'x86_64'
)

$ErrorActionPreference = 'Stop'

function Assert-Smoke {
  param([bool]$Condition, [string]$Message)
  if (-not $Condition) {
    throw "Windows installer smoke failed: $Message"
  }
}

$resolvedInstallerDirectory = (Resolve-Path -LiteralPath $InstallerDirectory).Path
$installers = @(Get-ChildItem -LiteralPath $resolvedInstallerDirectory -File -Filter '*-setup.exe')
Assert-Smoke ($installers.Count -eq 1) "expected exactly one *-setup.exe in $resolvedInstallerDirectory; found $($installers.Count)."

$runnerTemp = if ($env:RUNNER_TEMP) { $env:RUNNER_TEMP } else { [System.IO.Path]::GetTempPath() }
$smokeRoot = Join-Path $runnerTemp "mistake-trainer-installer-smoke-$([guid]::NewGuid().ToString('N'))"
$installRoot = Join-Path $smokeRoot 'installed'
$selfCheckPath = Join-Path $smokeRoot 'windows-self-check.json'
New-Item -ItemType Directory -Path $installRoot -Force | Out-Null

$installedExecutable = $null
$uninstaller = $null
try {
  $installProcess = Start-Process `
    -FilePath $installers[0].FullName `
    -ArgumentList @('/S', "/D=$installRoot") `
    -Wait `
    -PassThru
  Assert-Smoke ($installProcess.ExitCode -eq 0) "installer exited with code $($installProcess.ExitCode)."

  $applicationCandidates = @(
    Get-ChildItem -LiteralPath $installRoot -Recurse -File -Filter '*.exe' |
      Where-Object { $_.Name -notmatch '^(unins|uninstall)' }
  )
  Assert-Smoke ($applicationCandidates.Count -eq 1) "expected exactly one installed application executable; found $($applicationCandidates.Count)."
  $installedExecutable = $applicationCandidates[0]

  $selfCheckProcess = Start-Process `
    -FilePath $installedExecutable.FullName `
    -ArgumentList @('--windows-self-check', $selfCheckPath) `
    -Wait `
    -PassThru
  Assert-Smoke ($selfCheckProcess.ExitCode -eq 0) "installed self-check exited with code $($selfCheckProcess.ExitCode)."
  Assert-Smoke (Test-Path -LiteralPath $selfCheckPath -PathType Leaf) 'self-check JSON was not created.'

  $selfCheck = Get-Content -LiteralPath $selfCheckPath -Raw | ConvertFrom-Json
  Assert-Smoke ($selfCheck.schemaVersion -eq 1) 'unexpected self-check schema version.'
  Assert-Smoke ($selfCheck.windows.processArchitecture -eq $ExpectedArchitecture) "installed process architecture was $($selfCheck.windows.processArchitecture), expected $ExpectedArchitecture."
  Assert-Smoke ($selfCheck.windows.buildNumber -ge 17763) "Windows build $($selfCheck.windows.buildNumber) is below 17763."
  Assert-Smoke (@('supported', 'extended') -contains $selfCheck.windows.supportLevel) "support level was $($selfCheck.windows.supportLevel)."

  $uninstallers = @(
    Get-ChildItem -LiteralPath $installRoot -Recurse -File -Filter '*.exe' |
      Where-Object { $_.Name -match '^(unins|uninstall)' }
  )
  Assert-Smoke ($uninstallers.Count -eq 1) "expected exactly one uninstaller; found $($uninstallers.Count)."
  $uninstaller = $uninstallers[0]
  $uninstallProcess = Start-Process `
    -FilePath $uninstaller.FullName `
    -ArgumentList @('/S') `
    -Wait `
    -PassThru
  Assert-Smoke ($uninstallProcess.ExitCode -eq 0) "uninstaller exited with code $($uninstallProcess.ExitCode)."

  Start-Sleep -Milliseconds 500
  Assert-Smoke (-not (Test-Path -LiteralPath $installedExecutable.FullName -PathType Leaf)) 'application executable remained after uninstall.'
  Write-Output "Windows installer smoke passed: $($installers[0].Name)"
}
finally {
  if ($uninstaller -and (Test-Path -LiteralPath $uninstaller.FullName -PathType Leaf)) {
    try {
      Start-Process -FilePath $uninstaller.FullName -ArgumentList @('/S') -Wait | Out-Null
    }
    catch {
      Write-Warning 'Cleanup uninstaller did not complete.'
    }
  }
  $resolvedSmokeRoot = [System.IO.Path]::GetFullPath($smokeRoot)
  $resolvedRunnerTemp = [System.IO.Path]::GetFullPath($runnerTemp).TrimEnd('\') + '\'
  if ($resolvedSmokeRoot.StartsWith($resolvedRunnerTemp, [System.StringComparison]::OrdinalIgnoreCase)) {
    Remove-Item -LiteralPath $resolvedSmokeRoot -Recurse -Force -ErrorAction SilentlyContinue
  }
}
