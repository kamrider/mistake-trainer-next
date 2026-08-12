[CmdletBinding()]
param(
  [string]$InstallerDirectory,
  [ValidateSet('x86_64', 'arm64')][string]$ExpectedArchitecture = 'x86_64'
)
$ErrorActionPreference = 'Stop'
if ([string]::IsNullOrWhiteSpace($InstallerDirectory)) {
  $InstallerDirectory = Join-Path $PSScriptRoot '..\src-tauri\target\release\bundle\nsis'
}
$resolvedInstallerDirectory = (Resolve-Path -LiteralPath $InstallerDirectory).Path
$tauriConfigurationPath = Join-Path $PSScriptRoot '..\src-tauri\tauri.conf.json'
$tauriConfiguration = Get-Content -LiteralPath $tauriConfigurationPath -Raw | ConvertFrom-Json
$installerArchitecture = if ($ExpectedArchitecture -eq 'arm64') { 'arm64' } else { 'x64' }
$expectedInstallerName = "$($tauriConfiguration.productName)_$($tauriConfiguration.version)_$installerArchitecture-setup.exe"
$installers = @(Get-ChildItem -LiteralPath $resolvedInstallerDirectory -File -Filter $expectedInstallerName)
if ($installers.Count -ne 1) { throw "Expected exactly one $expectedInstallerName; found $($installers.Count)." }
$runId = [guid]::NewGuid().ToString('N')

if ($env:CI -eq 'true') {
  if ($env:MISTAKE_TRAINER_EPHEMERAL_WINDOWS -ne '1') {
    throw 'CI installer smoke requires MISTAKE_TRAINER_EPHEMERAL_WINDOWS=1.'
  }
  $runnerTemp = if ($env:RUNNER_TEMP) { $env:RUNNER_TEMP } else { [IO.Path]::GetTempPath() }
  $runnerTemp = (Resolve-Path -LiteralPath $runnerTemp).Path.TrimEnd('\')
  $selectionRoot = Join-Path $runnerTemp "mistake-trainer-installer-selection-$runId"
  $resultRoot = Join-Path $runnerTemp "mistake-trainer-installer-result-$ExpectedArchitecture"
  if (Test-Path -LiteralPath $resultRoot) {
    $oldResult = Get-Item -LiteralPath $resultRoot -Force
    if (-not $oldResult.PSIsContainer -or ($oldResult.Attributes -band [IO.FileAttributes]::ReparsePoint) -ne 0 -or (Split-Path -Parent $oldResult.FullName) -cne $runnerTemp -or $oldResult.Name -cne "mistake-trainer-installer-result-$ExpectedArchitecture") {
      throw 'Refusing to replace an unsafe installer smoke result root.'
    }
    Remove-Item -LiteralPath $oldResult.FullName -Recurse -Force
  }
  New-Item -ItemType Directory -Path $selectionRoot, $resultRoot | Out-Null
  $succeeded = $false
  try {
    Copy-Item -LiteralPath $installers[0].FullName -Destination $selectionRoot
    & (Join-Path $PSScriptRoot 'windows-installer-smoke-inner.ps1') -InstallerDirectory $selectionRoot -ExpectedArchitecture $ExpectedArchitecture -RunId $runId -ResultDirectory $resultRoot
    if ($LASTEXITCODE -ne 0) { throw 'Ephemeral installer smoke failed.' }
    $succeeded = $true
  }
  finally {
    $selectionItem = Get-Item -LiteralPath $selectionRoot -Force -ErrorAction SilentlyContinue
    if ($selectionItem -and $selectionItem.PSIsContainer -and ($selectionItem.Attributes -band [IO.FileAttributes]::ReparsePoint) -eq 0 -and (Split-Path -Parent $selectionItem.FullName) -ceq $runnerTemp -and $selectionItem.Name -ceq "mistake-trainer-installer-selection-$runId") {
      Remove-Item -LiteralPath $selectionItem.FullName -Recurse -Force
    }
    if ($succeeded) {
      $resultItem = Get-Item -LiteralPath $resultRoot -Force -ErrorAction SilentlyContinue
      if ($resultItem -and $resultItem.PSIsContainer -and ($resultItem.Attributes -band [IO.FileAttributes]::ReparsePoint) -eq 0 -and (Split-Path -Parent $resultItem.FullName) -ceq $runnerTemp -and $resultItem.Name -ceq "mistake-trainer-installer-result-$ExpectedArchitecture") {
        Remove-Item -LiteralPath $resultItem.FullName -Recurse -Force
      }
    }
  }
  exit 0
}

& (Join-Path $PSScriptRoot 'windows-installer-smoke-sandbox.ps1') -InstallerPath $installers[0].FullName -ExpectedArchitecture $ExpectedArchitecture -RunId $runId
