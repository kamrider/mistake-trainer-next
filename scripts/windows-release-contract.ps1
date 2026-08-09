[CmdletBinding()]
param(
    [switch]$ConfigOnly
)

$ErrorActionPreference = 'Stop'
Set-StrictMode -Version Latest

$repositoryRoot = (Resolve-Path (Join-Path $PSScriptRoot '..')).Path
$configurationPath = Join-Path $repositoryRoot 'src-tauri\tauri.conf.json'
$configuration = Get-Content -LiteralPath $configurationPath -Raw | ConvertFrom-Json

function Assert-Contract {
    param(
        [Parameter(Mandatory)]
        [bool]$Condition,
        [Parameter(Mandatory)]
        [string]$Message
    )

    if (-not $Condition) {
        throw "Windows release contract failed: $Message"
    }
}

$targets = @($configuration.bundle.targets)
$languages = @($configuration.bundle.windows.nsis.languages)

Assert-Contract ($targets -contains 'nsis') 'bundle.targets must include nsis.'
Assert-Contract ($configuration.bundle.windows.allowDowngrades -eq $false) 'installer downgrades must be disabled.'
Assert-Contract ($configuration.bundle.windows.webviewInstallMode.type -eq 'offlineInstaller') 'WebView2 must use offlineInstaller delivery.'
Assert-Contract ($configuration.bundle.windows.webviewInstallMode.silent -eq $true) 'WebView2 prerequisite installation must be silent.'
Assert-Contract ($configuration.bundle.windows.nsis.installMode -eq 'currentUser') 'the consumer installer must remain per-user.'
Assert-Contract ($languages -contains 'SimpChinese') 'the installer must include Simplified Chinese.'
Assert-Contract ($languages -contains 'English') 'the installer must include English.'
Assert-Contract ($configuration.bundle.windows.nsis.displayLanguageSelector -eq $false) 'installer language must follow the Windows locale without an extra prompt.'
Assert-Contract ($configuration.bundle.windows.nsis.compression -eq 'lzma') 'the offline installer must use LZMA compression.'
Assert-Contract ($configuration.bundle.createUpdaterArtifacts -eq $false) 'ordinary builds must not create updater artifacts.'
$configurationText = Get-Content -LiteralPath $configurationPath -Raw
Assert-Contract ($configurationText -notmatch '"pubkey"') 'ordinary config must not contain an updater public key.'
Assert-Contract ($configurationText -notmatch '"endpoints"') 'ordinary config must not contain updater endpoints.'

if (-not $ConfigOnly) {
    $cargoManifest = Get-Content -LiteralPath (Join-Path $repositoryRoot 'src-tauri\Cargo.toml') -Raw
    $cargoVersionMatch = [regex]::Match($cargoManifest, '(?ms)^\[package\].*?^version\s*=\s*"([^"]+)"')
    Assert-Contract $cargoVersionMatch.Success 'Cargo package version could not be read.'
    $versions = @(
        $configuration.version
        (Get-Content -LiteralPath (Join-Path $repositoryRoot 'package.json') -Raw | ConvertFrom-Json).version
        $cargoVersionMatch.Groups[1].Value
    )
    Assert-Contract (@($versions | Select-Object -Unique).Count -eq 1) 'tauri.conf.json, package.json, and Cargo.toml versions must match.'
}

Write-Output 'Windows release config contract passed.'
