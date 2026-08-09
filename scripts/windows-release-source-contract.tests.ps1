$ErrorActionPreference = 'Stop'
Set-StrictMode -Version Latest

$contractPath = Join-Path $PSScriptRoot 'windows-release-source-contract.ps1'
$sourceCommit = '0123456789abcdef0123456789abcdef01234567'
$otherCommit = '89abcdef0123456789abcdef0123456789abcdef'
$successfulRuns = @(
  @{
    conclusion = 'success'
    event = 'push'
    headSha = $sourceCommit
  }
) | ConvertTo-Json -Compress

& $contractPath `
  -ReleaseTag 'v0.1.0-rc.1' `
  -TagObjectType 'tag' `
  -SourceCommit $sourceCommit `
  -MainCommit $sourceCommit `
  -CiRunsJson $successfulRuns | Out-Null

function Assert-Blocked {
  param(
    [string]$Name,
    [hashtable]$Arguments
  )
  $blocked = $false
  try {
    & $contractPath @Arguments | Out-Null
  }
  catch {
    $blocked = $true
  }
  if (-not $blocked) {
    throw "Release source contract test failed: '$Name' was accepted."
  }
}

$validArguments = @{
  ReleaseTag = 'v0.1.0-rc.1'
  TagObjectType = 'tag'
  SourceCommit = $sourceCommit
  MainCommit = $sourceCommit
  CiRunsJson = $successfulRuns
}

foreach ($case in @(
  @{
    Name = 'repeated v prefix'
    Arguments = $validArguments.Clone()
    Mutation = { param($arguments) $arguments.ReleaseTag = 'vv0.1.0-rc.1' }
  },
  @{
    Name = 'empty prerelease identifier'
    Arguments = $validArguments.Clone()
    Mutation = { param($arguments) $arguments.ReleaseTag = 'v1.2.3-.' }
  },
  @{
    Name = 'numeric version with a leading zero'
    Arguments = $validArguments.Clone()
    Mutation = { param($arguments) $arguments.ReleaseTag = 'v01.2.3' }
  },
  @{
    Name = 'lightweight tag'
    Arguments = $validArguments.Clone()
    Mutation = { param($arguments) $arguments.TagObjectType = 'commit' }
  },
  @{
    Name = 'tag not at origin main'
    Arguments = $validArguments.Clone()
    Mutation = { param($arguments) $arguments.MainCommit = $otherCommit }
  },
  @{
    Name = 'failed CI run'
    Arguments = $validArguments.Clone()
    Mutation = {
      param($arguments)
      $arguments.CiRunsJson = @(
        @{ conclusion = 'failure'; event = 'push'; headSha = $sourceCommit }
      ) | ConvertTo-Json -Compress
    }
  },
  @{
    Name = 'workflow dispatch instead of push CI'
    Arguments = $validArguments.Clone()
    Mutation = {
      param($arguments)
      $arguments.CiRunsJson = @(
        @{ conclusion = 'success'; event = 'workflow_dispatch'; headSha = $sourceCommit }
      ) | ConvertTo-Json -Compress
    }
  }
)) {
  & $case.Mutation $case.Arguments
  Assert-Blocked -Name $case.Name -Arguments $case.Arguments
}

Write-Output 'Windows release source contract tests passed.'
