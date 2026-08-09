[CmdletBinding()]
param()

$ErrorActionPreference = 'Stop'
Set-StrictMode -Version Latest

$repositoryRoot = (Resolve-Path (Join-Path $PSScriptRoot '..')).Path
$manifestScript = Join-Path $repositoryRoot 'scripts\windows-update-manifest.ps1'
$releaseScript = Join-Path $repositoryRoot 'scripts\windows-signed-release.ps1'
$workflowPath = Join-Path $repositoryRoot '.github\workflows\release-windows.yml'
$configurationPath = Join-Path $repositoryRoot 'src-tauri\tauri.conf.json'
$testRoot = Join-Path ([System.IO.Path]::GetTempPath()) "mistake-trainer-update-contract-$([guid]::NewGuid().ToString('N'))"

function Assert-Contract {
    param(
        [Parameter(Mandatory)]
        [bool]$Condition,
        [Parameter(Mandatory)]
        [string]$Message
    )

    if (-not $Condition) {
        throw "Windows update contract test failed: $Message"
    }
}

function Write-Utf8NoBom {
    param(
        [Parameter(Mandatory)]
        [string]$Path,
        [Parameter(Mandatory)]
        [AllowEmptyString()]
        [string]$Content
    )

    [System.IO.File]::WriteAllText($Path, $Content, [System.Text.UTF8Encoding]::new($false))
}

function New-ArchitectureArtifacts {
    param(
        [Parameter(Mandatory)]
        [string]$Architecture
    )

    $installerName = "Mistake Trainer Next_0.1.0_$Architecture-setup.exe"
    $installerPath = Join-Path $testRoot $installerName
    Write-Utf8NoBom -Path $installerPath -Content "verified-$Architecture-installer"
    Write-Utf8NoBom -Path "$installerPath.sig" -Content "trusted-$Architecture-updater-signature"
    $hash = (Get-FileHash -LiteralPath $installerPath -Algorithm SHA256).Hash.ToLowerInvariant()
    Write-Utf8NoBom -Path "$installerPath.sha256" -Content "$hash  $installerName`r`n"
}

New-Item -ItemType Directory -Path $testRoot -Force | Out-Null

try {
    New-ArchitectureArtifacts -Architecture 'x64'
    New-ArchitectureArtifacts -Architecture 'arm64'

    $manifestPath = Join-Path $testRoot 'latest.json'
    & $manifestScript `
        -ArtifactDirectory $testRoot `
        -ReleaseTag 'v0.1.0' `
        -ArtifactBaseUrl 'https://downloads.mistake-trainer.invalid/releases/v0.1.0/' `
        -PublicationDateUtc '2026-07-28T00:00:00Z' `
        -OutputPath $manifestPath

    $manifestText = Get-Content -LiteralPath $manifestPath -Raw
    $manifest = $manifestText | ConvertFrom-Json
    Assert-Contract ($manifest.version -eq '0.1.0') 'manifest version did not match the release tag.'
    Assert-Contract ($manifestText -match '"pub_date"\s*:\s*"2026-07-28T00:00:00Z"') 'publication date was not preserved.'
    $platformNames = @($manifest.platforms.PSObject.Properties.Name | Sort-Object)
    Assert-Contract (($platformNames -join ',') -eq 'windows-aarch64,windows-x86_64') 'manifest platform set was not exact.'
    Assert-Contract ($manifest.platforms.'windows-x86_64'.signature -eq 'trusted-x64-updater-signature') 'x64 signature content was not embedded.'
    Assert-Contract ($manifest.platforms.'windows-aarch64'.signature -eq 'trusted-arm64-updater-signature') 'ARM64 signature content was not embedded.'
    Assert-Contract ($manifest.platforms.'windows-x86_64'.url -match '^https://') 'x64 artifact URL was not HTTPS.'
    Assert-Contract ($manifest.platforms.'windows-aarch64'.url -match '^https://') 'ARM64 artifact URL was not HTTPS.'
    Assert-Contract ($manifest.platforms.'windows-x86_64'.url -notmatch '\.sig$') 'x64 URL pointed to a signature instead of an installer.'
    Assert-Contract ($manifest.platforms.'windows-aarch64'.url -notmatch '\.sig$') 'ARM64 URL pointed to a signature instead of an installer.'

    $configuration = Get-Content -LiteralPath $configurationPath -Raw | ConvertFrom-Json
    Assert-Contract ($configuration.bundle.createUpdaterArtifacts -eq $false) 'ordinary builds must disable updater artifacts.'
    $configurationText = Get-Content -LiteralPath $configurationPath -Raw
    Assert-Contract ($configurationText -notmatch '"pubkey"') 'ordinary config must not contain an updater public key.'
    Assert-Contract ($configurationText -notmatch '"endpoints"') 'ordinary config must not contain updater endpoints.'

    $releaseText = Get-Content -LiteralPath $releaseScript -Raw
    foreach ($requiredName in @(
        'WINDOWS_UPDATE_ENDPOINT',
        'WINDOWS_UPDATE_ARTIFACT_BASE_URL',
        'WINDOWS_UPDATER_PUBLIC_KEY',
        'TAURI_SIGNING_PRIVATE_KEY',
        'TAURI_SIGNING_PRIVATE_KEY_PASSWORD'
    )) {
        Assert-Contract ($releaseText.Contains("'$requiredName'")) "release script did not require $requiredName."
    }
    Assert-Contract ($releaseText -match 'createUpdaterArtifacts\s*=\s*\$true') 'release override did not enable updater artifacts.'
    Assert-Contract ($releaseText -match "installMode\s*=\s*'passive'") 'release override did not use passive update installation.'

    $workflowText = Get-Content -LiteralPath $workflowPath -Raw
    Assert-Contract ($workflowText -match '\*-setup\.exe\.sig') 'workflow did not upload updater signatures.'
    Assert-Contract ($workflowText -match 'windows-update-manifest\.ps1') 'workflow did not generate latest.json.'
    Assert-Contract ($workflowText -match 'latest\.json') 'workflow did not publish latest.json.'

    $failureCases = @(
        @{
            Name = 'HTTP artifact base URL'
            Arguments = @('-ArtifactBaseUrl', 'http://downloads.mistake-trainer.invalid/releases/v0.1.0/')
        },
        @{
            Name = 'artifact base URL credentials'
            Arguments = @('-ArtifactBaseUrl', 'https://user:password@downloads.mistake-trainer.invalid/releases/v0.1.0/')
        },
        @{
            Name = 'version mismatch'
            Arguments = @('-ReleaseTag', 'v0.2.0')
        }
    )

    foreach ($failureCase in $failureCases) {
        $failureOutput = Join-Path $testRoot "$($failureCase.Name -replace '[^A-Za-z0-9]', '-').json"
        $arguments = @(
            '-NoProfile',
            '-ExecutionPolicy', 'Bypass',
            '-File', $manifestScript,
            '-ArtifactDirectory', $testRoot,
            '-ReleaseTag', 'v0.1.0',
            '-ArtifactBaseUrl', 'https://downloads.mistake-trainer.invalid/releases/v0.1.0/',
            '-PublicationDateUtc', '2026-07-28T00:00:00Z',
            '-OutputPath', $failureOutput
        )
        for ($index = 0; $index -lt $failureCase.Arguments.Count; $index += 2) {
            $argumentName = $failureCase.Arguments[$index]
            $existingIndex = [Array]::IndexOf($arguments, $argumentName)
            if ($existingIndex -ge 0) {
                $arguments[$existingIndex + 1] = $failureCase.Arguments[$index + 1]
            }
        }
        $process = Start-Process powershell.exe -ArgumentList $arguments -Wait -PassThru -WindowStyle Hidden
        Assert-Contract ($process.ExitCode -ne 0) "$($failureCase.Name) was accepted."
        Assert-Contract (-not (Test-Path -LiteralPath $failureOutput)) "$($failureCase.Name) left a manifest behind."
    }

    $x64Signature = Get-ChildItem -LiteralPath $testRoot -Filter '*_x64-setup.exe.sig' | Select-Object -First 1
    Write-Utf8NoBom -Path $x64Signature.FullName -Content ''
    $emptySignatureOutput = Join-Path $testRoot 'empty-signature.json'
    $process = Start-Process powershell.exe -ArgumentList @(
        '-NoProfile',
        '-ExecutionPolicy', 'Bypass',
        '-File', $manifestScript,
        '-ArtifactDirectory', $testRoot,
        '-ReleaseTag', 'v0.1.0',
        '-ArtifactBaseUrl', 'https://downloads.mistake-trainer.invalid/releases/v0.1.0/',
        '-PublicationDateUtc', '2026-07-28T00:00:00Z',
        '-OutputPath', $emptySignatureOutput
    ) -Wait -PassThru -WindowStyle Hidden
    Assert-Contract ($process.ExitCode -ne 0) 'empty updater signature was accepted.'
    Assert-Contract (-not (Test-Path -LiteralPath $emptySignatureOutput)) 'empty signature left a manifest behind.'

    Write-Output 'Windows update contract tests passed.'
}
finally {
    $resolvedTestRoot = [System.IO.Path]::GetFullPath($testRoot)
    $resolvedTempRoot = [System.IO.Path]::GetFullPath([System.IO.Path]::GetTempPath()).TrimEnd('\') + '\'
    if ($resolvedTestRoot.StartsWith($resolvedTempRoot, [System.StringComparison]::OrdinalIgnoreCase)) {
        Remove-Item -LiteralPath $resolvedTestRoot -Recurse -Force -ErrorAction SilentlyContinue
    }
}
