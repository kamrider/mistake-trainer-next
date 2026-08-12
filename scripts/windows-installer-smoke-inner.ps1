[CmdletBinding()]
param(
  [Parameter(Mandatory)][string]$InstallerDirectory,
  [ValidateSet('x86_64', 'arm64')][string]$ExpectedArchitecture,
  [Parameter(Mandatory)][string]$RunId,
  [Parameter(Mandatory)][string]$ResultDirectory
)

$ErrorActionPreference = 'Stop'
Set-StrictMode -Version Latest
if ($env:CI -ne 'true' -or $env:MISTAKE_TRAINER_EPHEMERAL_WINDOWS -ne '1') {
  throw 'Production-identity installer smoke is allowed only on an explicitly ephemeral Windows runner.'
}
if ($RunId -notmatch '^[0-9a-f]{32}$') { throw 'Invalid smoke RunId.' }
. (Join-Path $PSScriptRoot 'windows-job-object.ps1')
. (Join-Path $PSScriptRoot 'windows-installer-smoke-cleanup.ps1')

function Assert-Smoke([bool]$Condition, [string]$Message) {
  if (-not $Condition) { throw "Windows installer smoke failed: $Message" }
}
function Wait-MainWindow([System.Diagnostics.Process]$Process, [int]$Seconds) {
  $deadline = [DateTime]::UtcNow.AddSeconds($Seconds)
  while ([DateTime]::UtcNow -lt $deadline) {
    $Process.Refresh(); if ($Process.HasExited) { return $false }
    if ($Process.MainWindowHandle -ne [IntPtr]::Zero) { return $true }
    Start-Sleep -Milliseconds 200
  }
  return $false
}
function Start-SmokeProcess {
  param([Parameter(Mandatory)]$Job, [Parameter(Mandatory)][string]$FilePath, [string[]]$ArgumentList = @())
  $process = Start-ProcessInJob -Job $Job -FilePath $FilePath -ArgumentList $ArgumentList
  $script:launchedProcesses.Add($process)
  return $process
}
function Start-InstallerProcess {
  param([Parameter(Mandatory)][string]$FilePath, [string[]]$ArgumentList = @())
  $process = Start-Process -FilePath $FilePath -ArgumentList $ArgumentList -PassThru
  $script:launchedProcesses.Add($process)
  return $process
}
function Get-SmokeTreeFingerprint([string]$Root) {
  if (-not (Test-Path -LiteralPath $Root -PathType Container)) { return '' }
  $entries = @(Get-ChildItem -LiteralPath $Root -Recurse -File -Force | Sort-Object FullName | ForEach-Object {
    @{ relativePath = $_.FullName.Substring($Root.TrimEnd('\').Length + 1); length = $_.Length; sha256 = (Get-FileHash -LiteralPath $_.FullName -Algorithm SHA256).Hash.ToLowerInvariant() }
  })
  return ($entries | ConvertTo-Json -Depth 4 -Compress)
}
function Resolve-OwnedRegularExecutable([string]$Path, [string]$Root) {
  $rootItem = Get-Item -LiteralPath $Root -Force
  if (-not $rootItem.PSIsContainer -or ($rootItem.Attributes -band [IO.FileAttributes]::ReparsePoint) -ne 0) { throw 'owned executable root is unsafe' }
  $canonicalRoot = (Resolve-Path -LiteralPath $rootItem.FullName).Path.TrimEnd('\')
  $item = Get-Item -LiteralPath $Path -Force
  if ($item.PSIsContainer -or ($item.Attributes -band [IO.FileAttributes]::ReparsePoint) -ne 0) { throw 'owned executable is unsafe' }
  $ancestor = $item.Directory
  $insideRoot = $false
  while ($ancestor) {
    if (($ancestor.Attributes -band [IO.FileAttributes]::ReparsePoint) -ne 0) { throw 'owned executable has a reparse ancestor' }
    if ($ancestor.FullName.TrimEnd('\') -ceq $canonicalRoot) { $insideRoot = $true; break }
    $ancestor = $ancestor.Parent
  }
  if (-not $insideRoot) { throw 'owned executable escaped its root' }
  return (Resolve-Path -LiteralPath $item.FullName).Path
}

$runnerTemp = if ($env:RUNNER_TEMP) { $env:RUNNER_TEMP } else { [System.IO.Path]::GetTempPath() }
$runnerTemp = [System.IO.Path]::GetFullPath($runnerTemp).TrimEnd('\')
Remove-OwnedStaleSmokeRoot -RunnerTemp $runnerTemp
$smokeRoot = Join-Path $runnerTemp "mistake-trainer-installer-smoke-$RunId"
$markerPath = Join-Path $smokeRoot '.mistake-trainer-installer-smoke.json'
$resultPath = Join-Path $ResultDirectory 'result.json'
$job = $null
$firstProcess = $null
$failureCodes = @()
$failureStage = 'selection'
$allowedFailureStages = @(
  'selection', 'install_start', 'install_wait', 'install_exit', 'installed_layout', 'self_check', 'product_check',
  'gui_launch', 'single_instance', 'gui_shutdown', 'first_run_data',
  'reinstall', 'reinstall_preservation'
)
$status = 'failed'
$checksPassed = $false
$installer = $null
$uninstallerPath = $null
$sentinelPath = $null
$sentinelHash = $null
$libraryPath = $null
$libraryFingerprint = $null
$script:launchedProcesses = [Collections.Generic.List[Diagnostics.Process]]::new()
New-Item -ItemType Directory -Path $ResultDirectory -Force | Out-Null

try {
  $installers = @(Get-ChildItem -LiteralPath (Resolve-Path -LiteralPath $InstallerDirectory).Path -File -Filter '*-setup.exe')
  Assert-Smoke ($installers.Count -eq 1) 'expected exactly one selected installer in the ephemeral input.'
  $installer = $installers[0]
  New-Item -ItemType Directory -Path $smokeRoot | Out-Null
  $owner = Get-Process -Id $PID
  @{
    schemaVersion = 1; runId = $RunId; ownerPid = $PID
    ownerStartedAtUtc = $owner.StartTime.ToUniversalTime().ToString('o')
    createdAtUtc = [DateTime]::UtcNow.ToString('o')
  } | ConvertTo-Json | Set-Content -LiteralPath $markerPath -Encoding UTF8

  $installRoot = Join-Path $smokeRoot 'installed'
  $isolatedAppData = Join-Path $smokeRoot 'appdata'
  $isolatedLocalAppData = Join-Path $smokeRoot 'localappdata'
  $scratch = Join-Path $smokeRoot 'product-check-scratch'
  foreach ($directory in @($installRoot, $isolatedAppData, $isolatedLocalAppData, $scratch)) {
    New-Item -ItemType Directory -Path $directory -Force | Out-Null
  }
  $env:APPDATA = $isolatedAppData
  $env:LOCALAPPDATA = $isolatedLocalAppData
  $job = New-KillOnCloseJob
  # NSIS installers may manage their own child/job topology and have failed when
  # forced into this test's Job Object. They run only inside an ephemeral CI
  # worker or Windows Sandbox; product processes remain in the strict job above.

  $failureStage = 'install_start'
  $install = Start-InstallerProcess -FilePath $installer.FullName -ArgumentList @('/S', "/D=$installRoot")
  $failureStage = 'install_wait'
  Assert-Smoke (Wait-JobProcessExit $install 90) 'installer timed out.'
  $failureStage = 'install_exit'
  Assert-Smoke ($install.ExitCode -eq 0) "installer exit code $($install.ExitCode)."

  $failureStage = 'installed_layout'
  $apps = @(Get-ChildItem -LiteralPath $installRoot -Recurse -File -Filter '*.exe' | Where-Object { $_.Name -notmatch '^(unins|uninstall)' })
  $uninstallers = @(Get-ChildItem -LiteralPath $installRoot -Recurse -File -Filter '*.exe' | Where-Object { $_.Name -match '^(unins|uninstall)' })
  Assert-Smoke ($apps.Count -eq 1) 'expected exactly one installed application executable.'
  Assert-Smoke ($uninstallers.Count -eq 1) 'expected exactly one installed uninstaller.'
  $application = $apps[0]
  $applicationPath = Resolve-OwnedRegularExecutable -Path $application.FullName -Root $installRoot
  $uninstallerPath = Resolve-OwnedRegularExecutable -Path $uninstallers[0].FullName -Root $installRoot

  $failureStage = 'self_check'
  $selfPath = Join-Path $smokeRoot 'windows-self-check.json'
  $selfCheck = Start-SmokeProcess -Job $job -FilePath $applicationPath -ArgumentList @('--windows-self-check', $selfPath)
  Assert-Smoke (Wait-JobProcessExit $selfCheck 60) 'self-check timed out.'
  Assert-Smoke ($selfCheck.ExitCode -eq 0 -and (Test-Path -LiteralPath $selfPath -PathType Leaf)) 'self-check failed.'
  $self = Get-Content -LiteralPath $selfPath -Raw | ConvertFrom-Json
  Assert-Smoke ($self.ready -eq $true -and @($self.failureCodes).Count -eq 0) 'self-check reported a failure.'
  Assert-Smoke ($self.windows.processArchitecture -eq $ExpectedArchitecture) 'installed architecture mismatch.'

  $failureStage = 'product_check'
  $productPath = Join-Path $smokeRoot 'windows-product-check.json'
  $productCheck = Start-SmokeProcess -Job $job -FilePath $applicationPath -ArgumentList @('--windows-product-check', $productPath, $scratch)
  Assert-Smoke (Wait-JobProcessExit $productCheck 90) 'product check timed out.'
  Assert-Smoke ($productCheck.ExitCode -eq 0 -and (Test-Path -LiteralPath $productPath -PathType Leaf)) 'product check failed.'
  $product = Get-Content -LiteralPath $productPath -Raw | ConvertFrom-Json
  Assert-Smoke ($product.ready -eq $true -and @($product.failureCodes).Count -eq 0) 'product lifecycle check reported a failure.'

  $failureStage = 'gui_launch'
  $firstProcess = Start-SmokeProcess -Job $job -FilePath $applicationPath
  Assert-Smoke (Wait-MainWindow $firstProcess 25) 'installed GUI did not create a main window.'
  $failureStage = 'single_instance'
  $second = Start-SmokeProcess -Job $job -FilePath $applicationPath
  Assert-Smoke (Wait-JobProcessExit $second 15) 'second launch did not hand off.'
  Assert-Smoke ($second.ExitCode -eq 0) 'second launch handoff failed.'
  Start-Sleep -Seconds 10
  $failureStage = 'gui_shutdown'
  Assert-Smoke ($firstProcess.CloseMainWindow()) 'main window rejected normal close.'
  Assert-Smoke (Wait-JobProcessExit $firstProcess 15) 'main window did not exit.'

  $failureStage = 'first_run_data'
  $controlRoot = Join-Path $isolatedAppData 'com.mistaketrainer.next'
  $libraryPath = Join-Path $controlRoot 'library'
  Assert-Smoke (Test-Path -LiteralPath (Join-Path $libraryPath 'library.db') -PathType Leaf) 'first run did not create the isolated encrypted library.'
  $sentinelPath = Join-Path $controlRoot 'installer-preservation-sentinel.bin'
  $sentinelBytes = New-Object byte[] 32
  $random = [Security.Cryptography.RandomNumberGenerator]::Create()
  try { $random.GetBytes($sentinelBytes) } finally { $random.Dispose() }
  [IO.File]::WriteAllBytes($sentinelPath, $sentinelBytes)
  $sentinelHash = (Get-FileHash -LiteralPath $sentinelPath -Algorithm SHA256).Hash.ToLowerInvariant()
  $libraryFingerprint = Get-SmokeTreeFingerprint $libraryPath

  $failureStage = 'reinstall'
  $reinstall = Start-InstallerProcess -FilePath $installer.FullName -ArgumentList @('/S', "/D=$installRoot")
  Assert-Smoke (Wait-JobProcessExit $reinstall 90) 'same-version reinstall timed out.'
  Assert-Smoke ($reinstall.ExitCode -eq 0) 'same-version reinstall failed.'
  $failureStage = 'reinstall_preservation'
  Assert-Smoke ((Get-FileHash -LiteralPath $sentinelPath -Algorithm SHA256).Hash.ToLowerInvariant() -eq $sentinelHash) 'same-version reinstall changed the sentinel.'
  Assert-Smoke ((Get-SmokeTreeFingerprint $libraryPath) -ceq $libraryFingerprint) 'same-version reinstall changed the encrypted library.'
  $checksPassed = $true
}
catch {
  $boundedStage = if ($allowedFailureStages -ccontains $failureStage) { $failureStage } else { 'unknown' }
  $failureCodes += "installer_smoke_$boundedStage"
  Write-Warning 'Windows installer smoke checks failed; cleanup and bounded result reporting will continue.'
}
finally {
  if ($firstProcess -and -not $firstProcess.HasExited) { try { [void]$firstProcess.CloseMainWindow() } catch {} }
  $closeDeadline = [DateTime]::UtcNow.AddSeconds(10)
  while ($firstProcess -and -not $firstProcess.HasExited -and [DateTime]::UtcNow -lt $closeDeadline) { Start-Sleep -Milliseconds 200; $firstProcess.Refresh() }
  foreach ($recordedProcess in @($script:launchedProcesses)) {
    try {
      if (-not $recordedProcess.HasExited) { Stop-Process -Id $recordedProcess.Id -Force -ErrorAction SilentlyContinue }
    } catch {}
  }
  if ($job) { Close-KillOnCloseJob $job }

  if ($uninstallerPath -and (Test-Path -LiteralPath $uninstallerPath -PathType Leaf)) {
    try {
      $validatedUninstaller = Resolve-OwnedRegularExecutable -Path $uninstallerPath -Root $installRoot
      $uninstall = Start-InstallerProcess -FilePath $validatedUninstaller -ArgumentList @('/S')
      if (-not (Wait-JobProcessExit $uninstall 90) -or $uninstall.ExitCode -ne 0) { throw 'uninstaller failed' }
    }
    catch { $failureCodes += 'uninstaller_cleanup_failed' }
  } elseif ($uninstallerPath) {
    $failureCodes += 'uninstaller_missing'
  }

  if ($sentinelPath -and $libraryPath) {
    try {
      if (-not (Test-Path -LiteralPath $sentinelPath -PathType Leaf)) { throw 'sentinel missing after uninstall' }
      if ((Get-FileHash -LiteralPath $sentinelPath -Algorithm SHA256).Hash.ToLowerInvariant() -cne $sentinelHash) { throw 'sentinel changed after uninstall' }
      if ((Get-SmokeTreeFingerprint $libraryPath) -cne $libraryFingerprint) { throw 'encrypted library changed after uninstall' }
    }
    catch { $failureCodes += 'uninstall_data_preservation_failed' }
  }
  if ($checksPassed -and $failureCodes.Count -eq 0) { $status = 'passed' }

  if (Test-Path -LiteralPath $smokeRoot) {
    try {
      if (-not (Remove-OwnedCurrentSmokeRoot -RunnerTemp $runnerTemp -SmokeRoot $smokeRoot -RunId $RunId)) { $failureCodes += 'owned_cleanup_refused'; $status = 'failed' }
    }
    catch {
      $failureCodes += 'owned_cleanup_failed'
      $status = 'failed'
    }
  }
  $installerHash = if ($installer) { (Get-FileHash -LiteralPath $installer.FullName -Algorithm SHA256).Hash.ToLowerInvariant() } else { '' }
  $result = @{ runId = $RunId; architecture = $ExpectedArchitecture; installerSha256 = $installerHash; status = $status; failureCodes = $failureCodes }
  $temporaryResult = Join-Path $ResultDirectory "result.$RunId.tmp"
  $result | ConvertTo-Json | Set-Content -LiteralPath $temporaryResult -Encoding UTF8
  Move-Item -LiteralPath $temporaryResult -Destination $resultPath -Force
}

if ($status -ne 'passed') { exit 1 }
Write-Output 'Windows installer smoke passed'
