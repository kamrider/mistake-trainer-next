[CmdletBinding()]
param(
  [Parameter(Mandatory)][string]$InstallerPath,
  [ValidateSet('x86_64', 'arm64')][string]$ExpectedArchitecture,
  [Parameter(Mandatory)][string]$RunId
)
$ErrorActionPreference = 'Stop'
if ($RunId -notmatch '^[0-9a-f]{32}$') { throw 'Invalid smoke RunId.' }
$feature = Get-WindowsOptionalFeature -Online -FeatureName 'Containers-DisposableClientVM' -ErrorAction SilentlyContinue
$sandboxExecutable = Join-Path $env:WINDIR 'System32\WindowsSandbox.exe'
if (-not $feature -or $feature.State -ne 'Enabled' -or -not (Test-Path -LiteralPath $sandboxExecutable -PathType Leaf)) {
  throw [Text.Encoding]::UTF8.GetString([Convert]::FromBase64String('5pys5py65pyq5ZCv55SoIFdpbmRvd3MgU2FuZGJveO+8m+ivt+WcqCBDSSDnmoTkuLTml7YgV2luZG93cyBydW5uZXIg5Lit5omn6KGM5a6J6KOF5Zmo5YaS54Of5rWL6K+V44CC'))
}

function Get-ProductionFingerprint {
  $controlRoot = Join-Path $env:APPDATA 'com.mistaketrainer.next'
  $files = @('storage-location.json', 'storage-migration-pending.json', 'restore-pending.json', 'library-reset-pending.json') | ForEach-Object {
    $path = Join-Path $controlRoot $_
    if (Test-Path -LiteralPath $path -PathType Leaf) {
      $item = Get-Item -LiteralPath $path
      @{ name = $_; length = $item.Length; modified = $item.LastWriteTimeUtc.ToString('o'); hash = (Get-FileHash -LiteralPath $path -Algorithm SHA256).Hash }
    } else { @{ name = $_; missing = $true } }
  }
  $processes = @(Get-Process -Name 'mistake-trainer-next' -ErrorAction SilentlyContinue)
  $pids = @($processes | Select-Object -ExpandProperty Id | Sort-Object)
  $credentialTargets = @(& (Join-Path $env:WINDIR 'System32\cmdkey.exe') /list 2>$null | Where-Object { $_ -match 'com\.mistaketrainer\.next' } | ForEach-Object { $_.Trim() } | Sort-Object -Unique)
  if ($LASTEXITCODE -ne 0) { throw 'Could not enumerate production credential target names.' }

  $executableCandidates = [Collections.Generic.HashSet[string]]::new([StringComparer]::OrdinalIgnoreCase)
  foreach ($process in $processes) { try { if ($process.Path) { [void]$executableCandidates.Add($process.Path) } } catch {} }
  foreach ($registryPath in @('HKCU:\Software\Microsoft\Windows\CurrentVersion\Uninstall\*', 'HKLM:\Software\Microsoft\Windows\CurrentVersion\Uninstall\*')) {
    foreach ($entry in @(Get-ItemProperty -Path $registryPath -ErrorAction SilentlyContinue | Where-Object { $_.DisplayName -eq 'Mistake Trainer Next' })) {
      if ($entry.InstallLocation) {
        $candidate = Join-Path ([string]$entry.InstallLocation) 'mistake-trainer-next.exe'
        if (Test-Path -LiteralPath $candidate -PathType Leaf) { [void]$executableCandidates.Add((Resolve-Path -LiteralPath $candidate).Path) }
      }
      if ($entry.DisplayIcon) {
        $candidate = ([string]$entry.DisplayIcon).Trim('"') -replace ',\d+$', ''
        if (Test-Path -LiteralPath $candidate -PathType Leaf) { [void]$executableCandidates.Add((Resolve-Path -LiteralPath $candidate).Path) }
      }
    }
  }
  $executables = @($executableCandidates | Sort-Object | ForEach-Object {
    $item = Get-Item -LiteralPath $_ -Force
    if (($item.Attributes -band [IO.FileAttributes]::ReparsePoint) -ne 0) { throw 'Production executable is a reparse point.' }
    @{ path = $item.FullName; length = $item.Length; modified = $item.LastWriteTimeUtc.ToString('o'); hash = (Get-FileHash -LiteralPath $item.FullName -Algorithm SHA256).Hash }
  })
  return (@{ files = $files; credentialTargets = $credentialTargets; executables = $executables; pids = $pids } | ConvertTo-Json -Depth 6 -Compress)
}

function Read-BoundedSmokeResult([string]$Path) {
  $item = Get-Item -LiteralPath $Path -Force -ErrorAction SilentlyContinue
  if (-not $item -or $item.PSIsContainer -or $item.Length -gt 4096 -or ($item.Attributes -band [IO.FileAttributes]::ReparsePoint) -ne 0) {
    throw 'Windows Sandbox did not return a bounded regular smoke result.'
  }
  return (Get-Content -LiteralPath $item.FullName -Raw | ConvertFrom-Json)
}

$hostBefore = Get-ProductionFingerprint
$runnerTemp = if ($env:RUNNER_TEMP) { $env:RUNNER_TEMP } else { [IO.Path]::GetTempPath() }
$hostRoot = Join-Path $runnerTemp "mistake-trainer-installer-sandbox-$RunId"
$inputRoot = Join-Path $hostRoot 'input'
$installerRoot = Join-Path $inputRoot 'installer'
$resultRoot = Join-Path $hostRoot 'results'
New-Item -ItemType Directory -Path $installerRoot, $resultRoot | Out-Null
try {
  Copy-Item -LiteralPath $InstallerPath -Destination $installerRoot
  foreach ($script in @('windows-installer-smoke-cleanup.ps1', 'windows-installer-smoke-inner.ps1', 'windows-installer-smoke-guest.ps1')) {
    Copy-Item -LiteralPath (Join-Path $PSScriptRoot $script) -Destination $inputRoot
  }
  @{ runId = $RunId; architecture = $ExpectedArchitecture } | ConvertTo-Json | Set-Content -LiteralPath (Join-Path $inputRoot 'guest-config.json') -Encoding UTF8
  $inputXml = [Security.SecurityElement]::Escape((Resolve-Path -LiteralPath $inputRoot).Path)
  $resultXml = [Security.SecurityElement]::Escape((Resolve-Path -LiteralPath $resultRoot).Path)
  $wsbPath = Join-Path $hostRoot 'installer-smoke.wsb'
  @"
<Configuration>
  <Networking>Disable</Networking><ClipboardRedirection>Disable</ClipboardRedirection><PrinterRedirection>Disable</PrinterRedirection>
  <MappedFolders>
    <MappedFolder><HostFolder>$inputXml</HostFolder><SandboxFolder>C:\SmokeInput</SandboxFolder><ReadOnly>true</ReadOnly></MappedFolder>
    <MappedFolder><HostFolder>$resultXml</HostFolder><SandboxFolder>C:\SmokeResults</SandboxFolder><ReadOnly>false</ReadOnly></MappedFolder>
  </MappedFolders>
  <LogonCommand><Command>powershell.exe -NoProfile -ExecutionPolicy Bypass -File C:\SmokeInput\windows-installer-smoke-guest.ps1</Command></LogonCommand>
</Configuration>
"@ | Set-Content -LiteralPath $wsbPath -Encoding UTF8
  $sandbox = Start-Process -FilePath $sandboxExecutable -ArgumentList @($wsbPath) -PassThru -Wait
  $resultPath = Join-Path $resultRoot 'result.json'
  $result = Read-BoundedSmokeResult $resultPath
  $expectedHash = (Get-FileHash -LiteralPath $InstallerPath -Algorithm SHA256).Hash.ToLowerInvariant()
  if ($result.runId -ne $RunId -or $result.architecture -ne $ExpectedArchitecture -or $result.installerSha256 -ne $expectedHash -or $result.status -ne 'passed' -or @($result.failureCodes).Count -ne 0) {
    throw 'Windows Sandbox returned an invalid or failed smoke result.'
  }
  Write-Output 'Windows installer smoke passed in Windows Sandbox'
}
finally {
  $hostUnchanged = $false
  try { $hostUnchanged = (Get-ProductionFingerprint) -ceq $hostBefore } catch { $hostUnchanged = $false }
  $resolvedRunner = (Resolve-Path -LiteralPath $runnerTemp).Path.TrimEnd('\')
  $hostItem = Get-Item -LiteralPath $hostRoot -Force -ErrorAction SilentlyContinue
  if ($hostItem -and $hostItem.PSIsContainer -and ($hostItem.Attributes -band [IO.FileAttributes]::ReparsePoint) -eq 0) {
    $resolved = (Resolve-Path -LiteralPath $hostItem.FullName).Path.TrimEnd('\')
    if ((Split-Path -Leaf $resolved) -ceq "mistake-trainer-installer-sandbox-$RunId" -and (Split-Path -Parent $resolved) -ceq $resolvedRunner) {
      Remove-Item -LiteralPath $resolved -Recurse -Force -ErrorAction SilentlyContinue
    }
  }
  if (-not $hostUnchanged) { throw 'Host production state changed or could not be re-verified during isolated installer smoke.' }
}
