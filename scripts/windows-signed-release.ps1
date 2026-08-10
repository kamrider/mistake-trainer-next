[CmdletBinding()]
param(
  [string]$ReleaseTag = $env:GITHUB_REF_NAME,
  [ValidateSet('x64', 'arm64')]
  [string]$Architecture = 'x64'
)

$ErrorActionPreference = 'Stop'

function Assert-Release {
  param([bool]$Condition, [string]$Message)
  if (-not $Condition) {
    throw "Windows updater release blocked: $Message"
  }
}

$requiredEnvironment = @(
  'WINDOWS_AUTHENTICODE_MODE',
  'GITHUB_REPOSITORY',
  'WINDOWS_UPDATER_PUBLIC_KEY',
  'TAURI_SIGNING_PRIVATE_KEY',
  'TAURI_SIGNING_PRIVATE_KEY_PASSWORD'
)
foreach ($name in $requiredEnvironment) {
  Assert-Release (-not [string]::IsNullOrWhiteSpace([Environment]::GetEnvironmentVariable($name))) "missing $name."
}

Assert-Release (-not [string]::IsNullOrWhiteSpace($ReleaseTag)) 'release tag is missing.'
Assert-Release ($ReleaseTag -match '^v(?:0|[1-9]\d*)\.(?:0|[1-9]\d*)\.(?:0|[1-9]\d*)(?:-(?:(?:0|[1-9]\d*)|(?:\d*[A-Za-z-][0-9A-Za-z-]*))(?:\.(?:(?:0|[1-9]\d*)|(?:\d*[A-Za-z-][0-9A-Za-z-]*)))*)?(?:\+[0-9A-Za-z-]+(?:\.[0-9A-Za-z-]+)*)?$') "tag '$ReleaseTag' must use exact vX.Y.Z semantic-version syntax."
$releaseVersion = $ReleaseTag.Substring(1)
$targetTriple = if ($Architecture -eq 'arm64') { 'aarch64-pc-windows-msvc' } else { 'x86_64-pc-windows-msvc' }
$selfCheckArchitecture = if ($Architecture -eq 'arm64') { 'arm64' } else { 'x86_64' }

$repositoryRoot = (Resolve-Path (Join-Path $PSScriptRoot '..')).Path
$packageJson = Get-Content -LiteralPath (Join-Path $repositoryRoot 'package.json') -Raw | ConvertFrom-Json
$tauriConfiguration = Get-Content -LiteralPath (Join-Path $repositoryRoot 'src-tauri\tauri.conf.json') -Raw | ConvertFrom-Json
$cargoManifest = Get-Content -LiteralPath (Join-Path $repositoryRoot 'src-tauri\Cargo.toml') -Raw
$cargoVersionMatch = [regex]::Match($cargoManifest, '(?ms)^\[package\].*?^version\s*=\s*"([^"]+)"')
Assert-Release $cargoVersionMatch.Success 'Cargo package version could not be read.'

Assert-Release ($packageJson.version -eq $releaseVersion) "package.json version '$($packageJson.version)' does not match '$releaseVersion'."
Assert-Release ($tauriConfiguration.version -eq $releaseVersion) "tauri.conf.json version '$($tauriConfiguration.version)' does not match '$releaseVersion'."
Assert-Release ($cargoVersionMatch.Groups[1].Value -eq $releaseVersion) "Cargo.toml version '$($cargoVersionMatch.Groups[1].Value)' does not match '$releaseVersion'."

$repositorySlug = $env:GITHUB_REPOSITORY.Trim()
Assert-Release ($repositorySlug -eq $env:GITHUB_REPOSITORY) 'GITHUB_REPOSITORY must not contain surrounding whitespace.'
Assert-Release ($repositorySlug -match '^[A-Za-z0-9_.-]+/[A-Za-z0-9_.-]+$') 'GITHUB_REPOSITORY must use owner/repository syntax.'
$updateEndpointValue = "https://github.com/$repositorySlug/releases/latest/download/latest.json"
$updateEndpoint = $null
Assert-Release ([uri]::TryCreate($updateEndpointValue, [System.UriKind]::Absolute, [ref]$updateEndpoint)) 'derived update endpoint is invalid.'
Assert-Release ($updateEndpoint.Scheme -eq 'https') 'update endpoint must use HTTPS.'
Assert-Release ([string]::IsNullOrEmpty($updateEndpoint.UserInfo)) 'update endpoint must not contain credentials.'
Assert-Release ([string]::IsNullOrEmpty($updateEndpoint.Fragment)) 'update endpoint must not contain a fragment.'

$updaterPublicKey = $env:WINDOWS_UPDATER_PUBLIC_KEY.Trim()
Assert-Release ($updaterPublicKey.Length -ge 32) 'updater public key is unexpectedly short.'
Assert-Release ($updaterPublicKey.Length -le 16384) 'updater public key is unexpectedly large.'
Assert-Release ($env:WINDOWS_AUTHENTICODE_MODE -ceq 'disabled') 'WINDOWS_AUTHENTICODE_MODE must be exactly disabled for the free release channel.'

$runnerTemp = if ($env:RUNNER_TEMP) { $env:RUNNER_TEMP } else { [System.IO.Path]::GetTempPath() }
$releaseTemp = Join-Path $runnerTemp "mistake-trainer-signing-$([guid]::NewGuid().ToString('N'))"
$overridePath = Join-Path $releaseTemp 'tauri.signing.conf.json'
New-Item -ItemType Directory -Path $releaseTemp -Force | Out-Null

try {
  $overrideJson = @{
    bundle = @{
      createUpdaterArtifacts = $true
    }
    plugins = @{
      updater = @{
        pubkey = $updaterPublicKey
        endpoints = @($updateEndpoint.AbsoluteUri)
        windows = @{
          installMode = 'passive'
        }
      }
    }
  } | ConvertTo-Json -Depth 7
  [System.IO.File]::WriteAllText(
    $overridePath,
    $overrideJson,
    [System.Text.UTF8Encoding]::new($false)
  )

  & (Join-Path $repositoryRoot 'scripts\windows-release-contract.ps1')

  Push-Location $repositoryRoot
  try {
    corepack pnpm tauri build --target $targetTriple --config $overridePath
    if ($LASTEXITCODE -ne 0) { throw "Tauri build exited with code $LASTEXITCODE." }
  }
  finally {
    Pop-Location
  }

  $targetReleaseRoot = Join-Path $repositoryRoot "src-tauri\target\$targetTriple\release"
  $applicationExecutable = Join-Path $targetReleaseRoot 'mistake-trainer-next.exe'
  $installerDirectory = Join-Path $targetReleaseRoot 'bundle\nsis'
  $installers = @(Get-ChildItem -LiteralPath $installerDirectory -File -Filter '*-setup.exe')
  Assert-Release (Test-Path -LiteralPath $applicationExecutable -PathType Leaf) 'release application executable is missing.'
  Assert-Release ($installers.Count -eq 1) "expected one NSIS installer; found $($installers.Count)."
  Assert-Release ($installers[0].Name -match "_$Architecture-setup\.exe$") "installer '$($installers[0].Name)' does not match requested architecture '$Architecture'."
  $updaterSignaturePath = "$($installers[0].FullName).sig"
  Assert-Release (Test-Path -LiteralPath $updaterSignaturePath -PathType Leaf) 'Tauri updater signature is missing.'
  $updaterSignature = (Get-Content -LiteralPath $updaterSignaturePath -Raw).Trim()
  Assert-Release (-not [string]::IsNullOrWhiteSpace($updaterSignature)) 'Tauri updater signature is empty.'
  Assert-Release ($updaterSignature.Length -le 16384) 'Tauri updater signature is unexpectedly large.'

  $verifierManifest = Join-Path $repositoryRoot 'scripts\updater-signature-verifier\Cargo.toml'
  & cargo run `
    --release `
    --locked `
    --quiet `
    --manifest-path $verifierManifest `
    -- `
    $installers[0].FullName `
    $updaterSignaturePath
  Assert-Release ($LASTEXITCODE -eq 0) 'Tauri updater signature did not match the installer and configured public key.'

  foreach ($artifact in @((Get-Item -LiteralPath $applicationExecutable), $installers[0])) {
    $signature = Get-AuthenticodeSignature -LiteralPath $artifact.FullName
    Assert-Release ($signature.Status -eq 'NotSigned') "$($artifact.Name) unexpectedly has Authenticode status '$($signature.Status)'."
    Assert-Release ($null -eq $signature.SignerCertificate) "$($artifact.Name) unexpectedly has an Authenticode signer certificate."
  }

  & (Join-Path $repositoryRoot 'scripts\windows-installer-smoke.ps1') `
    -InstallerDirectory $installerDirectory `
    -ExpectedArchitecture $selfCheckArchitecture

  $hash = (Get-FileHash -LiteralPath $installers[0].FullName -Algorithm SHA256).Hash.ToLowerInvariant()
  $checksumPath = "$($installers[0].FullName).sha256"
  "$hash  $($installers[0].Name)" | Set-Content -LiteralPath $checksumPath -Encoding ascii
  Write-Warning 'Installer is intentionally not Authenticode-signed; Windows may show Unknown publisher or SmartScreen warnings.'
  Write-Output "Unsigned Windows installer verified: $($installers[0].FullName)"
  Write-Output "Updater signature cryptographically verified: $updaterSignaturePath"
  Write-Output "Checksum: $checksumPath"
}
finally {
  $resolvedReleaseTemp = [System.IO.Path]::GetFullPath($releaseTemp)
  $resolvedRunnerTemp = [System.IO.Path]::GetFullPath($runnerTemp).TrimEnd('\') + '\'
  if ($resolvedReleaseTemp.StartsWith($resolvedRunnerTemp, [System.StringComparison]::OrdinalIgnoreCase)) {
    Remove-Item -LiteralPath $resolvedReleaseTemp -Recurse -Force -ErrorAction SilentlyContinue
  }
}
