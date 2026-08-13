[CmdletBinding()]
param()

$ErrorActionPreference = 'Stop'
Set-StrictMode -Version Latest

$repositoryRoot = (Resolve-Path (Join-Path $PSScriptRoot '..')).Path
$manifestScript = Join-Path $repositoryRoot 'scripts\windows-update-manifest.ps1'
$releaseScript = Join-Path $repositoryRoot 'scripts\windows-signed-release.ps1'
$signatureVerifierManifest = Join-Path $repositoryRoot 'scripts\updater-signature-verifier\Cargo.toml'
$workflowPath = Join-Path $repositoryRoot '.github\workflows\release-windows.yml'
$configurationPath = Join-Path $repositoryRoot 'src-tauri\tauri.conf.json'
$supportPolicyPath = Join-Path $repositoryRoot 'docs\windows-support-policy.md'
$changelogPath = Join-Path $repositoryRoot 'CHANGELOG.md'
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
    & cargo test --locked --quiet --manifest-path $signatureVerifierManifest
    Assert-Contract ($LASTEXITCODE -eq 0) 'updater signature verifier tests failed.'

    New-ArchitectureArtifacts -Architecture 'x64'
    New-ArchitectureArtifacts -Architecture 'arm64'

    $manifestPath = Join-Path $testRoot 'latest.json'
    & $manifestScript `
        -ArtifactDirectory $testRoot `
        -ReleaseTag 'v0.1.0' `
        -RepositorySlug 'kamrider/mistake-trainer-next' `
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
    Assert-Contract ($manifest.platforms.'windows-x86_64'.url -eq 'https://github.com/kamrider/mistake-trainer-next/releases/download/v0.1.0/Mistake.Trainer.Next_0.1.0_x64-setup.exe') 'x64 artifact URL did not match GitHub release asset filename normalization.'
    Assert-Contract ($manifest.platforms.'windows-aarch64'.url -eq 'https://github.com/kamrider/mistake-trainer-next/releases/download/v0.1.0/Mistake.Trainer.Next_0.1.0_arm64-setup.exe') 'ARM64 artifact URL did not match GitHub release asset filename normalization.'
    Assert-Contract ($manifest.platforms.'windows-x86_64'.url -notmatch '\.sig$') 'x64 URL pointed to a signature instead of an installer.'
    Assert-Contract ($manifest.platforms.'windows-aarch64'.url -notmatch '\.sig$') 'ARM64 URL pointed to a signature instead of an installer.'

    $configuration = Get-Content -LiteralPath $configurationPath -Raw | ConvertFrom-Json
    Assert-Contract ($configuration.bundle.createUpdaterArtifacts -eq $false) 'ordinary builds must disable updater artifacts.'
    $configurationText = Get-Content -LiteralPath $configurationPath -Raw
    Assert-Contract ($configurationText -notmatch '"pubkey"') 'ordinary config must not contain an updater public key.'
    Assert-Contract ($configurationText -notmatch '"endpoints"') 'ordinary config must not contain updater endpoints.'

    $releaseText = Get-Content -LiteralPath $releaseScript -Raw
    foreach ($requiredName in @(
        'WINDOWS_AUTHENTICODE_MODE',
        'GITHUB_REPOSITORY',
        'WINDOWS_UPDATER_PUBLIC_KEY',
        'TAURI_SIGNING_PRIVATE_KEY',
        'TAURI_SIGNING_PRIVATE_KEY_PASSWORD'
    )) {
        Assert-Contract ($releaseText.Contains("'$requiredName'")) "release script did not require $requiredName."
    }
    Assert-Contract ($releaseText -match 'createUpdaterArtifacts\s*=\s*\$true') 'release override did not enable updater artifacts.'
    Assert-Contract ($releaseText -match "installMode\s*=\s*'passive'") 'release override did not use passive update installation.'
    Assert-Contract ($releaseText -match "WINDOWS_AUTHENTICODE_MODE\s+-ceq\s+'disabled'") 'release script did not require the explicit free unsigned mode.'
    Assert-Contract ($releaseText -match "Status\s+-eq\s+'NotSigned'") 'release script did not verify that Authenticode is absent.'
    Assert-Contract ($releaseText -match 'updater-signature-verifier\\Cargo\.toml') 'release script did not invoke the updater signature verifier.'
    Assert-Contract ($releaseText -match 'signature did not match the installer and configured public key') 'release script did not fail closed on a mismatched updater signature.'
    foreach ($forbiddenName in @(
        'WINDOWS_CERTIFICATE',
        'WINDOWS_CERTIFICATE_PASSWORD',
        'WINDOWS_CERTIFICATE_THUMBPRINT',
        'WINDOWS_TIMESTAMP_URL'
    )) {
        Assert-Contract (-not $releaseText.Contains($forbiddenName)) "free release script still referenced $forbiddenName."
    }

    $workflowText = Get-Content -LiteralPath $workflowPath -Raw
    Assert-Contract ($workflowText -match '\*-setup\.exe\.sig') 'workflow did not upload updater signatures.'
    Assert-Contract ($workflowText -match 'windows-update-manifest\.ps1') 'workflow did not generate latest.json.'
    Assert-Contract ($workflowText -match 'latest\.json') 'workflow did not publish latest.json.'
    Assert-Contract ($workflowText -notmatch 'vars\.WINDOWS_UPDATE_ENDPOINT') 'workflow still required a manually configured update endpoint.'
    Assert-Contract ($workflowText -notmatch 'vars\.WINDOWS_UPDATE_ARTIFACT_BASE_URL') 'workflow still required a manually configured artifact base URL.'
    Assert-Contract ($workflowText -match 'WINDOWS_AUTHENTICODE_MODE:\s*disabled') 'workflow did not explicitly select the free unsigned mode.'
    Assert-Contract ($workflowText -match "CI:\s*'true'") 'release workflow did not identify the installer smoke worker as CI.'
    Assert-Contract ($workflowText -match "MISTAKE_TRAINER_EPHEMERAL_WINDOWS:\s*'1'") 'release workflow did not explicitly authorize its ephemeral Windows smoke worker.'
    Assert-Contract ($workflowText -match 'Unknown publisher or Microsoft Defender SmartScreen warning') 'draft release did not disclose the unsigned installer warning.'
    Assert-Contract ($workflowText -match 'target_triple:\s*x86_64-pc-windows-msvc') 'workflow did not define the exact x64 target artifact root.'
    Assert-Contract ($workflowText -match 'target_triple:\s*aarch64-pc-windows-msvc') 'workflow did not define the exact ARM64 target artifact root.'
    Assert-Contract ($workflowText -match 'src-tauri/target/\$\{\{\s*matrix\.target_triple\s*\}\}/release/bundle/nsis/\*-setup\.exe') 'workflow upload paths do not flatten installers into the artifact root.'
    Assert-Contract ($workflowText -notmatch 'src-tauri/target/\*-pc-windows-msvc') 'workflow upload paths still preserve architecture directories inside artifacts.'
    foreach ($forbiddenName in @(
        'secrets.WINDOWS_CERTIFICATE',
        'secrets.WINDOWS_CERTIFICATE_PASSWORD',
        'secrets.WINDOWS_CERTIFICATE_THUMBPRINT',
        'vars.WINDOWS_TIMESTAMP_URL'
    )) {
        Assert-Contract (-not $workflowText.Contains($forbiddenName)) "free workflow still referenced $forbiddenName."
    }

    $supportPolicyText = Get-Content -LiteralPath $supportPolicyPath -Raw -Encoding UTF8
    $unsignedStatusPhrase = (-join @([char]0x72B6, [char]0x6001, [char]0x5747, [char]0x4E3A)) + ' `NotSigned`'
    $updaterSigningPhrase = 'Tauri updater ' + (-join @([char]0x79C1, [char]0x94A5, [char]0x7B7E, [char]0x540D))
    $trustedPublisherPhrase = -join @(
        [char]0x5FC5, [char]0x987B, [char]0x4F7F, [char]0x7528, [char]0x540C,
        [char]0x4E00, [char]0x4E2A, [char]0x53D7, [char]0x4FE1, [char]0x4EFB,
        [char]0x53D1, [char]0x5E03, [char]0x8005, [char]0x8EAB, [char]0x4EFD
    )
    Assert-Contract ($supportPolicyText.Contains($unsignedStatusPhrase)) 'support policy did not document the unsigned executable contract.'
    Assert-Contract ($supportPolicyText.Contains($updaterSigningPhrase)) 'support policy did not preserve updater signing.'
    Assert-Contract (-not $supportPolicyText.Contains($trustedPublisherPhrase)) 'support policy still required trusted Authenticode for the free channel.'

    $changelogText = Get-Content -LiteralPath $changelogPath -Raw -Encoding UTF8
    Assert-Contract ($changelogText -match '## 0\.1\.0 \u2014 2026-08-10') 'stable 0.1.0 changelog entry was missing.'
    Assert-Contract ($changelogText -match 'intentionally not Authenticode-signed') 'stable changelog did not disclose the unsigned installer risk.'
    Assert-Contract ($changelogText -notmatch 'before a paid production release') 'changelog still required a paid production release.'

    $failureCases = @(
        @{
            Name = 'repository slug with URL syntax'
            Arguments = @('-RepositorySlug', 'https://github.com/kamrider/mistake-trainer-next')
        },
        @{
            Name = 'repository slug traversal'
            Arguments = @('-RepositorySlug', 'kamrider/../mistake-trainer-next')
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
            '-RepositorySlug', 'kamrider/mistake-trainer-next',
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
        '-RepositorySlug', 'kamrider/mistake-trainer-next',
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
