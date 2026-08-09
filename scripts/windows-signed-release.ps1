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
    throw "Windows signed release blocked: $Message"
  }
}

function Normalize-Thumbprint {
  param([string]$Value)
  return ($Value -replace '\s', '').ToUpperInvariant()
}

$requiredEnvironment = @(
  'WINDOWS_CERTIFICATE',
  'WINDOWS_CERTIFICATE_PASSWORD',
  'WINDOWS_CERTIFICATE_THUMBPRINT',
  'WINDOWS_TIMESTAMP_URL',
  'WINDOWS_UPDATE_ENDPOINT',
  'WINDOWS_UPDATE_ARTIFACT_BASE_URL',
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

$timestampUri = $null
Assert-Release ([uri]::TryCreate($env:WINDOWS_TIMESTAMP_URL, [System.UriKind]::Absolute, [ref]$timestampUri)) 'timestamp URL is invalid.'
Assert-Release ($timestampUri.Scheme -eq 'https') 'timestamp URL must use HTTPS.'

$updateEndpoint = $null
Assert-Release ([uri]::TryCreate($env:WINDOWS_UPDATE_ENDPOINT, [System.UriKind]::Absolute, [ref]$updateEndpoint)) 'update endpoint is invalid.'
Assert-Release ($updateEndpoint.Scheme -eq 'https') 'update endpoint must use HTTPS.'
Assert-Release ([string]::IsNullOrEmpty($updateEndpoint.UserInfo)) 'update endpoint must not contain credentials.'
Assert-Release ([string]::IsNullOrEmpty($updateEndpoint.Fragment)) 'update endpoint must not contain a fragment.'

$artifactBaseUri = $null
Assert-Release ([uri]::TryCreate($env:WINDOWS_UPDATE_ARTIFACT_BASE_URL, [System.UriKind]::Absolute, [ref]$artifactBaseUri)) 'update artifact base URL is invalid.'
Assert-Release ($artifactBaseUri.Scheme -eq 'https') 'update artifact base URL must use HTTPS.'
Assert-Release ([string]::IsNullOrEmpty($artifactBaseUri.UserInfo)) 'update artifact base URL must not contain credentials.'
Assert-Release ([string]::IsNullOrEmpty($artifactBaseUri.Query)) 'update artifact base URL must not contain a query.'
Assert-Release ([string]::IsNullOrEmpty($artifactBaseUri.Fragment)) 'update artifact base URL must not contain a fragment.'

$updaterPublicKey = $env:WINDOWS_UPDATER_PUBLIC_KEY.Trim()
Assert-Release ($updaterPublicKey.Length -ge 32) 'updater public key is unexpectedly short.'
Assert-Release ($updaterPublicKey.Length -le 16384) 'updater public key is unexpectedly large.'

$expectedThumbprint = Normalize-Thumbprint $env:WINDOWS_CERTIFICATE_THUMBPRINT
Assert-Release ($expectedThumbprint -match '^[0-9A-F]{40,64}$') 'certificate thumbprint has an invalid shape.'

$runnerTemp = if ($env:RUNNER_TEMP) { $env:RUNNER_TEMP } else { [System.IO.Path]::GetTempPath() }
$releaseTemp = Join-Path $runnerTemp "mistake-trainer-signing-$([guid]::NewGuid().ToString('N'))"
$pfxPath = Join-Path $releaseTemp 'windows-signing-certificate.pfx'
$overridePath = Join-Path $releaseTemp 'tauri.signing.conf.json'
$importedCertificatePaths = @()
$preExistingCertificatePaths = @(
  Get-ChildItem 'Cert:\CurrentUser\My' | ForEach-Object { $_.PSPath }
)
New-Item -ItemType Directory -Path $releaseTemp -Force | Out-Null

try {
  try {
    [System.IO.File]::WriteAllBytes(
      $pfxPath,
      [Convert]::FromBase64String($env:WINDOWS_CERTIFICATE)
    )
  }
  catch {
    throw 'Windows signed release blocked: WINDOWS_CERTIFICATE is not valid base64.'
  }

  $securePassword = ConvertTo-SecureString $env:WINDOWS_CERTIFICATE_PASSWORD -AsPlainText -Force
  $imported = @(Import-PfxCertificate -FilePath $pfxPath -CertStoreLocation 'Cert:\CurrentUser\My' -Password $securePassword)
  $importedCertificatePaths = @($imported | ForEach-Object { $_.PSPath })
  $signingCertificate = $imported |
    Where-Object { (Normalize-Thumbprint $_.Thumbprint) -eq $expectedThumbprint } |
    Select-Object -First 1
  Assert-Release ($null -ne $signingCertificate) 'imported PFX did not contain the expected signing certificate.'
  Assert-Release ($signingCertificate.HasPrivateKey) 'expected signing certificate has no private key.'
  $overrideJson = @{
    bundle = @{
      createUpdaterArtifacts = $true
      windows = @{
        certificateThumbprint = $expectedThumbprint
        digestAlgorithm = 'sha256'
        timestampUrl = $timestampUri.AbsoluteUri
        tsp = $true
      }
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

  foreach ($artifact in @((Get-Item -LiteralPath $applicationExecutable), $installers[0])) {
    $signature = Get-AuthenticodeSignature -LiteralPath $artifact.FullName
    Assert-Release ($signature.Status -eq 'Valid') "$($artifact.Name) signature status is '$($signature.Status)'."
    Assert-Release ($null -ne $signature.SignerCertificate) "$($artifact.Name) has no signer certificate."
    Assert-Release ((Normalize-Thumbprint $signature.SignerCertificate.Thumbprint) -eq $expectedThumbprint) "$($artifact.Name) was signed by an unexpected certificate."
  }

  & (Join-Path $repositoryRoot 'scripts\windows-installer-smoke.ps1') `
    -InstallerDirectory $installerDirectory `
    -ExpectedArchitecture $selfCheckArchitecture

  $hash = (Get-FileHash -LiteralPath $installers[0].FullName -Algorithm SHA256).Hash.ToLowerInvariant()
  $checksumPath = "$($installers[0].FullName).sha256"
  "$hash  $($installers[0].Name)" | Set-Content -LiteralPath $checksumPath -Encoding ascii
  Write-Output "Signed Windows release verified: $($installers[0].FullName)"
  Write-Output "Updater signature verified: $updaterSignaturePath"
  Write-Output "Checksum: $checksumPath"
}
finally {
  Remove-Item -LiteralPath $pfxPath -Force -ErrorAction SilentlyContinue
  foreach ($certificatePath in $importedCertificatePaths) {
    if ($preExistingCertificatePaths -notcontains $certificatePath) {
      Remove-Item -LiteralPath $certificatePath -Force -ErrorAction SilentlyContinue
    }
  }
  $resolvedReleaseTemp = [System.IO.Path]::GetFullPath($releaseTemp)
  $resolvedRunnerTemp = [System.IO.Path]::GetFullPath($runnerTemp).TrimEnd('\') + '\'
  if ($resolvedReleaseTemp.StartsWith($resolvedRunnerTemp, [System.StringComparison]::OrdinalIgnoreCase)) {
    Remove-Item -LiteralPath $resolvedReleaseTemp -Recurse -Force -ErrorAction SilentlyContinue
  }
}
