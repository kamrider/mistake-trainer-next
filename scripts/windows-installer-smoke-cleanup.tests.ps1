$ErrorActionPreference = 'Stop'
. (Join-Path $PSScriptRoot 'windows-installer-smoke-cleanup.ps1')
$root = Join-Path ([IO.Path]::GetTempPath()) "mistake-trainer-cleanup-test-$([guid]::NewGuid().ToString('N'))"
$outside = Join-Path ([IO.Path]::GetTempPath()) "mistake-trainer-installer-smoke-99999999999999999999999999999999"
New-Item -ItemType Directory -Path $root, $outside | Out-Null

function New-SmokeFixture {
  param([string]$Name, [string]$RunId, [int]$OwnerPid = 2147483647, [string]$OwnerStartedAtUtc = '2000-01-01T00:00:00.0000000Z', [string]$CreatedAtUtc = '2000-01-01T00:00:00Z', [string]$MarkerRunId = $RunId)
  $path = Join-Path $root $Name
  New-Item -ItemType Directory -Path $path | Out-Null
  @{ schemaVersion = 1; runId = $MarkerRunId; ownerPid = $OwnerPid; ownerStartedAtUtc = $OwnerStartedAtUtc; createdAtUtc = $CreatedAtUtc } | ConvertTo-Json | Set-Content -LiteralPath (Join-Path $path '.mistake-trainer-installer-smoke.json')
  return $path
}

try {
  $validId = '11111111111111111111111111111111'
  $reusedId = '22222222222222222222222222222222'
  $liveId = '33333333333333333333333333333333'
  $youngId = '44444444444444444444444444444444'
  $mismatchId = '55555555555555555555555555555555'
  $missingId = '66666666666666666666666666666666'
  $reparseId = '77777777777777777777777777777777'
  $valid = New-SmokeFixture "mistake-trainer-installer-smoke-$validId" $validId
  $reused = New-SmokeFixture "mistake-trainer-installer-smoke-$reusedId" $reusedId -OwnerPid $PID -OwnerStartedAtUtc '2000-01-01T00:00:00.0000000Z'
  $owner = Get-Process -Id $PID
  $live = New-SmokeFixture "mistake-trainer-installer-smoke-$liveId" $liveId -OwnerPid $PID -OwnerStartedAtUtc $owner.StartTime.ToUniversalTime().ToString('o')
  $young = New-SmokeFixture "mistake-trainer-installer-smoke-$youngId" $youngId -CreatedAtUtc '2025-12-31T12:00:00Z'
  $mismatch = New-SmokeFixture "mistake-trainer-installer-smoke-$mismatchId" $mismatchId -MarkerRunId 'wrong'
  $missing = Join-Path $root "mistake-trainer-installer-smoke-$missingId"
  New-Item -ItemType Directory -Path $missing | Out-Null
  $uppercase = New-SmokeFixture 'mistake-trainer-installer-smoke-AAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAA' 'aaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaa'
  $timezoneLessId = 'abababababababababababababababab'
  $timezoneLess = New-SmokeFixture "mistake-trainer-installer-smoke-$timezoneLessId" $timezoneLessId -OwnerStartedAtUtc '2000-01-01T00:00:00' -CreatedAtUtc '2000-01-01T00:00:00'
  $unrelated = Join-Path $root 'unrelated-sibling'
  New-Item -ItemType Directory -Path $unrelated | Out-Null
  @{ schemaVersion = 1; runId = '99999999999999999999999999999999'; ownerPid = 2147483647; ownerStartedAtUtc = '2000-01-01T00:00:00.0000000Z'; createdAtUtc = '2000-01-01T00:00:00Z' } | ConvertTo-Json | Set-Content -LiteralPath (Join-Path $outside '.mistake-trainer-installer-smoke.json')

  $reparseTarget = Join-Path $root 'reparse-target'
  New-Item -ItemType Directory -Path $reparseTarget | Out-Null
  $reparse = Join-Path $root "mistake-trainer-installer-smoke-$reparseId"
  New-Item -ItemType Junction -Path $reparse -Target $reparseTarget | Out-Null

  Remove-OwnedStaleSmokeRoot -RunnerTemp $root -NowUtc ([DateTime]'2026-01-01T00:00:00Z')
  if (Test-Path -LiteralPath $valid) { throw 'Valid owned stale root was not removed.' }
  if (Test-Path -LiteralPath $reused) { throw 'Reused-PID stale root was not removed.' }
  foreach ($preserved in @($live, $young, $mismatch, $missing, $uppercase, $timezoneLess, $unrelated, $reparse, $outside)) {
    if (-not (Test-Path -LiteralPath $preserved)) { throw "Rejected fixture was removed: $preserved" }
  }
  $currentId = '88888888888888888888888888888888'
  $current = New-SmokeFixture "mistake-trainer-installer-smoke-$currentId" $currentId -CreatedAtUtc '2026-01-01T00:00:00Z'
  if (-not (Remove-OwnedCurrentSmokeRoot -RunnerTemp $root -SmokeRoot $current -RunId $currentId)) { throw 'Valid current owned root was not removed.' }
  if (Test-Path -LiteralPath $current) { throw 'Valid current owned root remained after cleanup.' }
  if (Remove-OwnedCurrentSmokeRoot -RunnerTemp $root -SmokeRoot $mismatch -RunId $mismatchId) { throw 'Current cleanup accepted a mismatched marker.' }
  if (Remove-OwnedCurrentSmokeRoot -RunnerTemp $root -SmokeRoot $reparse -RunId $reparseId) { throw 'Current cleanup accepted a reparse root.' }
  if (Remove-OwnedCurrentSmokeRoot -RunnerTemp $root -SmokeRoot $outside -RunId '99999999999999999999999999999999') { throw 'Current cleanup accepted a root outside runner temp.' }
  Write-Output 'Owned stale smoke cleanup rejection matrix passed'
}
finally {
  Remove-Item -LiteralPath $root -Recurse -Force -ErrorAction SilentlyContinue
  Remove-Item -LiteralPath $outside -Recurse -Force -ErrorAction SilentlyContinue
}
