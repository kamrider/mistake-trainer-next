[CmdletBinding()]
param(
    [Parameter(Mandatory)]
    [string]$ArtifactDirectory,
    [Parameter(Mandatory)]
    [string]$ReleaseTag,
    [Parameter(Mandatory)]
    [string]$ArtifactBaseUrl,
    [Parameter(Mandatory)]
    [string]$OutputPath,
    [string]$PublicationDateUtc = [DateTimeOffset]::UtcNow.ToString('yyyy-MM-ddTHH:mm:ssZ')
)

$ErrorActionPreference = 'Stop'
Set-StrictMode -Version Latest

function Assert-Manifest {
    param(
        [Parameter(Mandatory)]
        [bool]$Condition,
        [Parameter(Mandatory)]
        [string]$Message
    )

    if (-not $Condition) {
        throw "Windows update manifest blocked: $Message"
    }
}

function Read-VerifiedArtifact {
    param(
        [Parameter(Mandatory)]
        [string]$Architecture
    )

    $installers = @(Get-ChildItem -LiteralPath $resolvedArtifactDirectory -File -Filter "*_$Architecture-setup.exe")
    Assert-Manifest ($installers.Count -eq 1) "expected one $Architecture installer; found $($installers.Count)."
    $installer = $installers[0]
    $signaturePath = "$($installer.FullName).sig"
    $checksumPath = "$($installer.FullName).sha256"
    Assert-Manifest (Test-Path -LiteralPath $signaturePath -PathType Leaf) "$Architecture updater signature is missing."
    Assert-Manifest (Test-Path -LiteralPath $checksumPath -PathType Leaf) "$Architecture checksum is missing."

    $signature = (Get-Content -LiteralPath $signaturePath -Raw).Trim()
    Assert-Manifest (-not [string]::IsNullOrWhiteSpace($signature)) "$Architecture updater signature is empty."
    Assert-Manifest ($signature.Length -le 16384) "$Architecture updater signature is unexpectedly large."

    $checksumLine = (Get-Content -LiteralPath $checksumPath -Raw).Trim()
    $checksumMatch = [regex]::Match($checksumLine, '^([0-9a-fA-F]{64})\s{2}(.+)$')
    Assert-Manifest $checksumMatch.Success "$Architecture checksum format is invalid."
    Assert-Manifest ($checksumMatch.Groups[2].Value -eq $installer.Name) "$Architecture checksum names a different installer."
    $actualHash = (Get-FileHash -LiteralPath $installer.FullName -Algorithm SHA256).Hash
    Assert-Manifest ($actualHash -eq $checksumMatch.Groups[1].Value) "$Architecture installer checksum does not match."

    $versionPattern = [regex]::Escape($releaseVersion)
    Assert-Manifest ($installer.Name -match "_${versionPattern}_$Architecture-setup\.exe$") "$Architecture installer version does not match the release tag."

    $escapedName = [Uri]::EscapeDataString($installer.Name)
    return [ordered]@{
        signature = $signature
        url = "$normalizedArtifactBaseUrl/$escapedName"
    }
}

Assert-Manifest (Test-Path -LiteralPath $ArtifactDirectory -PathType Container) 'artifact directory does not exist.'
$resolvedArtifactDirectory = (Resolve-Path -LiteralPath $ArtifactDirectory).Path

Assert-Manifest (-not [string]::IsNullOrWhiteSpace($ReleaseTag)) 'release tag is missing.'
$releaseVersion = $ReleaseTag.TrimStart('v')
Assert-Manifest ($releaseVersion -match '^\d+\.\d+\.\d+([+-][0-9A-Za-z.-]+)?$') "release tag '$ReleaseTag' is not a supported semantic version."

$artifactBaseUri = $null
Assert-Manifest ([Uri]::TryCreate($ArtifactBaseUrl, [UriKind]::Absolute, [ref]$artifactBaseUri)) 'artifact base URL is invalid.'
Assert-Manifest ($artifactBaseUri.Scheme -eq 'https') 'artifact base URL must use HTTPS.'
Assert-Manifest ([string]::IsNullOrEmpty($artifactBaseUri.UserInfo)) 'artifact base URL must not contain credentials.'
Assert-Manifest ([string]::IsNullOrEmpty($artifactBaseUri.Query)) 'artifact base URL must not contain a query.'
Assert-Manifest ([string]::IsNullOrEmpty($artifactBaseUri.Fragment)) 'artifact base URL must not contain a fragment.'
$normalizedArtifactBaseUrl = $artifactBaseUri.AbsoluteUri.TrimEnd('/')

$parsedPublicationDate = [DateTimeOffset]::MinValue
Assert-Manifest ([DateTimeOffset]::TryParse(
    $PublicationDateUtc,
    [Globalization.CultureInfo]::InvariantCulture,
    [Globalization.DateTimeStyles]::AssumeUniversal,
    [ref]$parsedPublicationDate
)) 'publication date must be RFC 3339 compatible.'
Assert-Manifest ($PublicationDateUtc -match '^\d{4}-\d{2}-\d{2}T\d{2}:\d{2}:\d{2}(?:\.\d+)?Z$') 'publication date must be an explicit UTC timestamp.'

$allInstallers = @(Get-ChildItem -LiteralPath $resolvedArtifactDirectory -File -Filter '*-setup.exe')
Assert-Manifest ($allInstallers.Count -eq 2) "expected exactly two installers; found $($allInstallers.Count)."
Assert-Manifest (@($allInstallers.Name | Select-Object -Unique).Count -eq 2) 'installer names must be unique.'

$x64 = Read-VerifiedArtifact -Architecture 'x64'
$arm64 = Read-VerifiedArtifact -Architecture 'arm64'
$manifest = [ordered]@{
    version = $releaseVersion
    pub_date = $PublicationDateUtc
    platforms = [ordered]@{
        'windows-x86_64' = $x64
        'windows-aarch64' = $arm64
    }
}

$resolvedOutputPath = [System.IO.Path]::GetFullPath($OutputPath)
$outputDirectory = Split-Path -Parent $resolvedOutputPath
Assert-Manifest (-not [string]::IsNullOrWhiteSpace($outputDirectory)) 'output directory is invalid.'
New-Item -ItemType Directory -Path $outputDirectory -Force | Out-Null
$temporaryOutput = "$resolvedOutputPath.$([guid]::NewGuid().ToString('N')).tmp"
try {
    $json = $manifest | ConvertTo-Json -Depth 6
    [System.IO.File]::WriteAllText($temporaryOutput, "$json`r`n", [System.Text.UTF8Encoding]::new($false))
    Move-Item -LiteralPath $temporaryOutput -Destination $resolvedOutputPath -Force
}
finally {
    Remove-Item -LiteralPath $temporaryOutput -Force -ErrorAction SilentlyContinue
}

Write-Output "Windows update manifest verified: $resolvedOutputPath"
