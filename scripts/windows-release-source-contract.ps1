[CmdletBinding()]
param(
  [Parameter(Mandatory)]
  [string]$ReleaseTag,
  [Parameter(Mandatory)]
  [string]$TagObjectType,
  [Parameter(Mandatory)]
  [string]$SourceCommit,
  [Parameter(Mandatory)]
  [string]$MainCommit,
  [Parameter(Mandatory)]
  [string]$CiRunsJson
)

$ErrorActionPreference = 'Stop'
Set-StrictMode -Version Latest

function Assert-ReleaseSource {
  param([bool]$Condition, [string]$Message)
  if (-not $Condition) {
    throw "Windows release source contract failed: $Message"
  }
}

Assert-ReleaseSource `
  ($ReleaseTag -match '^v(?:0|[1-9]\d*)\.(?:0|[1-9]\d*)\.(?:0|[1-9]\d*)(?:-(?:(?:0|[1-9]\d*)|(?:\d*[A-Za-z-][0-9A-Za-z-]*))(?:\.(?:(?:0|[1-9]\d*)|(?:\d*[A-Za-z-][0-9A-Za-z-]*)))*)?(?:\+[0-9A-Za-z-]+(?:\.[0-9A-Za-z-]+)*)?$') `
  "release tag '$ReleaseTag' must use exact vX.Y.Z semantic-version syntax."
Assert-ReleaseSource ($TagObjectType.Trim() -eq 'tag') 'release ref must be an annotated tag object.'

$normalizedSourceCommit = $SourceCommit.Trim().ToLowerInvariant()
$normalizedMainCommit = $MainCommit.Trim().ToLowerInvariant()
Assert-ReleaseSource ($normalizedSourceCommit -match '^[0-9a-f]{40}$') 'source commit must be a full Git SHA.'
Assert-ReleaseSource ($normalizedMainCommit -match '^[0-9a-f]{40}$') 'main commit must be a full Git SHA.'
Assert-ReleaseSource `
  ($normalizedSourceCommit -eq $normalizedMainCommit) `
  "tag commit '$normalizedSourceCommit' is not the reviewed origin/main commit '$normalizedMainCommit'."

try {
  $parsedRuns = $CiRunsJson | ConvertFrom-Json
}
catch {
  throw 'Windows release source contract failed: CI run evidence is not valid JSON.'
}
$ciRuns = if ($null -eq $parsedRuns) { @() } else { @($parsedRuns) }
$successfulRuns = @(
  $ciRuns | Where-Object {
    "$($_.headSha)".ToLowerInvariant() -eq $normalizedSourceCommit -and
      $_.event -eq 'push' -and
      $_.conclusion -eq 'success'
  }
)
Assert-ReleaseSource `
  ($successfulRuns.Count -ge 1) `
  "reviewed commit '$normalizedSourceCommit' has no successful completed CI push run."

Write-Output "Windows release source contract passed for $ReleaseTag at $normalizedSourceCommit."
