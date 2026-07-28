[CmdletBinding()]
param(
  [string]$InstallerDirectory,
  [ValidateSet('x86_64', 'arm64')]
  [string]$ExpectedArchitecture = 'x86_64'
)

$ErrorActionPreference = 'Stop'

if ([string]::IsNullOrWhiteSpace($InstallerDirectory)) {
  $InstallerDirectory = Join-Path $PSScriptRoot '..\src-tauri\target\release\bundle\nsis'
}

function Assert-Smoke {
  param([bool]$Condition, [string]$Message)
  if (-not $Condition) {
    throw "Windows installer smoke failed: $Message"
  }
}

function Wait-ForProcessExit {
  param(
    [System.Diagnostics.Process]$Process,
    [int]$TimeoutSeconds
  )
  $deadline = [DateTime]::UtcNow.AddSeconds($TimeoutSeconds)
  do {
    $Process.Refresh()
    if ($Process.HasExited) {
      return $true
    }
    Start-Sleep -Milliseconds 250
  } while ([DateTime]::UtcNow -lt $deadline)
  $Process.Refresh()
  return $Process.HasExited
}

function Wait-ForMainWindow {
  param(
    [System.Diagnostics.Process]$Process,
    [int]$TimeoutSeconds
  )
  $deadline = [DateTime]::UtcNow.AddSeconds($TimeoutSeconds)
  do {
    $Process.Refresh()
    if ($Process.HasExited) {
      return $false
    }
    if ($Process.MainWindowHandle -ne [IntPtr]::Zero) {
      return $true
    }
    Start-Sleep -Milliseconds 250
  } while ([DateTime]::UtcNow -lt $deadline)
  return $false
}

$resolvedInstallerDirectory = (Resolve-Path -LiteralPath $InstallerDirectory).Path
$installers = @(Get-ChildItem -LiteralPath $resolvedInstallerDirectory -File -Filter '*-setup.exe')
Assert-Smoke ($installers.Count -eq 1) "expected exactly one *-setup.exe in $resolvedInstallerDirectory; found $($installers.Count)."

$runnerTemp = if ($env:RUNNER_TEMP) { $env:RUNNER_TEMP } else { [System.IO.Path]::GetTempPath() }
$smokeRoot = Join-Path $runnerTemp "mistake-trainer-installer-smoke-$([guid]::NewGuid().ToString('N'))"
$installRoot = Join-Path $smokeRoot 'installed'
$selfCheckPath = Join-Path $smokeRoot 'windows-self-check.json'
$isolatedAppData = Join-Path $smokeRoot 'appdata'
$isolatedLocalAppData = Join-Path $smokeRoot 'localappdata'
$startupFailurePath = Join-Path $isolatedAppData 'com.mistaketrainer.next\startup-failure.json'
New-Item -ItemType Directory -Path $installRoot -Force | Out-Null
New-Item -ItemType Directory -Path $isolatedAppData -Force | Out-Null
New-Item -ItemType Directory -Path $isolatedLocalAppData -Force | Out-Null

$installedExecutable = $null
$uninstaller = $null
$firstProcess = $null
$secondProcess = $null
$originalAppData = $env:APPDATA
$originalLocalAppData = $env:LOCALAPPDATA
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
  Assert-Smoke ($selfCheck.schemaVersion -eq 2) 'unexpected self-check schema version.'
  Assert-Smoke ($selfCheck.ready -eq $true) 'installed runtime did not report ready.'
  Assert-Smoke (@($selfCheck.failureCodes).Count -eq 0) "self-check reported failures: $($selfCheck.failureCodes -join ', ')."
  Assert-Smoke ($selfCheck.windows.processArchitecture -eq $ExpectedArchitecture) "installed process architecture was $($selfCheck.windows.processArchitecture), expected $ExpectedArchitecture."
  Assert-Smoke ($selfCheck.windows.buildNumber -ge 17763) "Windows build $($selfCheck.windows.buildNumber) is below 17763."
  Assert-Smoke (@('supported', 'extended') -contains $selfCheck.windows.supportLevel) "support level was $($selfCheck.windows.supportLevel)."
  Assert-Smoke (-not [string]::IsNullOrWhiteSpace($selfCheck.windows.webview2Version)) 'WebView2 runtime version was not detected.'

  $env:APPDATA = $isolatedAppData
  $env:LOCALAPPDATA = $isolatedLocalAppData

  $firstProcess = Start-Process -FilePath $installedExecutable.FullName -PassThru
  Assert-Smoke (Wait-ForMainWindow -Process $firstProcess -TimeoutSeconds 20) 'installed GUI did not create a main window within 20 seconds.'
  Start-Sleep -Seconds 10
  $firstProcess.Refresh()
  Assert-Smoke (-not $firstProcess.HasExited) 'installed GUI exited during the 10-second stability window.'

  $secondProcess = Start-Process -FilePath $installedExecutable.FullName -PassThru
  Assert-Smoke (Wait-ForProcessExit -Process $secondProcess -TimeoutSeconds 10) 'second launch did not hand off to the existing instance within 10 seconds.'
  Assert-Smoke ($secondProcess.ExitCode -eq 0) "second launch exited with code $($secondProcess.ExitCode)."
  $firstProcess.Refresh()
  Assert-Smoke (-not $firstProcess.HasExited) 'first instance exited after the second-launch handoff.'

  Assert-Smoke ($firstProcess.CloseMainWindow()) 'main window did not accept a normal close request.'
  Assert-Smoke (Wait-ForProcessExit -Process $firstProcess -TimeoutSeconds 10) 'installed GUI did not exit after its main window closed.'
  Assert-Smoke ($firstProcess.ExitCode -eq 0) "installed GUI exited with code $($firstProcess.ExitCode)."
  Assert-Smoke (-not (Test-Path -LiteralPath $startupFailurePath -PathType Leaf)) 'healthy GUI launch created startup-failure.json.'

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
  foreach ($process in @($secondProcess, $firstProcess)) {
    if ($process) {
      try {
        $process.Refresh()
        if (-not $process.HasExited) {
          Stop-Process -Id $process.Id -Force -ErrorAction SilentlyContinue
        }
      }
      catch {
        Write-Warning 'Smoke process cleanup did not complete.'
      }
    }
  }
  $env:APPDATA = $originalAppData
  $env:LOCALAPPDATA = $originalLocalAppData
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
