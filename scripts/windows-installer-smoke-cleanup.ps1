function Convert-OwnedSmokeTimestampToUtc {
  param([Parameter(Mandatory)]$Value)
  if ($Value -is [DateTime]) {
    if ($Value.Kind -eq [DateTimeKind]::Unspecified) { throw 'Smoke timestamp has no timezone.' }
    return $Value.ToUniversalTime()
  }
  if ($Value -is [DateTimeOffset]) { return $Value.UtcDateTime }
  $text = [string]$Value
  if ($text -notmatch '(?:Z|[+-]\d{2}:\d{2})$') { throw 'Smoke timestamp has no timezone.' }
  return [DateTimeOffset]::Parse(
    $text,
    [Globalization.CultureInfo]::InvariantCulture,
    [Globalization.DateTimeStyles]::AllowWhiteSpaces
  ).UtcDateTime
}

function Test-OwnedSmokeProcessPresent {
  param([Parameter(Mandatory)][string]$Root)
  $canonicalRoot = [IO.Path]::GetFullPath($Root).TrimEnd('\')
  $prefix = "$canonicalRoot\"
  foreach ($process in @(Get-Process -ErrorAction Stop)) {
    try { $canonicalPath = [IO.Path]::GetFullPath([string]$process.Path) } catch { continue }
    if ($canonicalPath.StartsWith($prefix, [StringComparison]::OrdinalIgnoreCase)) { return $true }
  }
  return $false
}

function Remove-OwnedStaleSmokeRoot {
  param([Parameter(Mandatory)][string]$RunnerTemp, [DateTime]$NowUtc = [DateTime]::UtcNow)
  $canonicalRunner = (Resolve-Path -LiteralPath $RunnerTemp).Path.TrimEnd('\')
  foreach ($candidate in @(Get-ChildItem -LiteralPath $canonicalRunner -Directory -Force -ErrorAction SilentlyContinue)) {
    if ($candidate.Name -cnotmatch '^mistake-trainer-installer-smoke-([0-9a-f]{32})$') { continue }
    if (($candidate.Attributes -band [IO.FileAttributes]::ReparsePoint) -ne 0) { continue }
    $runId = $Matches[1]
    $canonicalCandidate = (Resolve-Path -LiteralPath $candidate.FullName).Path.TrimEnd('\')
    if ((Split-Path -Parent $canonicalCandidate) -ne $canonicalRunner) { continue }
    $markerPath = Join-Path $canonicalCandidate '.mistake-trainer-installer-smoke.json'
    $markerItem = Get-Item -LiteralPath $markerPath -Force -ErrorAction SilentlyContinue
    if (-not $markerItem -or $markerItem.PSIsContainer -or $markerItem.Length -gt 4096 -or ($markerItem.Attributes -band [IO.FileAttributes]::ReparsePoint) -ne 0) { continue }
    try { $marker = Get-Content -LiteralPath $markerPath -Raw | ConvertFrom-Json } catch { continue }
    if ($marker.schemaVersion -ne 1 -or $marker.runId -ne $runId) { continue }
    try { $created = Convert-OwnedSmokeTimestampToUtc $marker.createdAtUtc } catch { continue }
    if (($NowUtc.ToUniversalTime() - $created).TotalHours -le 24) { continue }
    $ownerPid = 0
    if (-not [int]::TryParse([string]$marker.ownerPid, [ref]$ownerPid) -or $ownerPid -le 0) { continue }
    try { $recordedOwnerStart = Convert-OwnedSmokeTimestampToUtc $marker.ownerStartedAtUtc } catch { continue }
    $ownerAlive = $false
    $owner = Get-Process -Id $ownerPid -ErrorAction SilentlyContinue
    if ($owner) {
      try { $ownerAlive = $owner.StartTime.ToUniversalTime() -eq $recordedOwnerStart } catch { $ownerAlive = $true }
    }
    if ($ownerAlive) { continue }
    if (Test-OwnedSmokeProcessPresent -Root $canonicalCandidate) { continue }
    Remove-Item -LiteralPath $canonicalCandidate -Recurse -Force
  }
}

function Remove-OwnedCurrentSmokeRoot {
  param(
    [Parameter(Mandatory)][string]$RunnerTemp,
    [Parameter(Mandatory)][string]$SmokeRoot,
    [Parameter(Mandatory)][string]$RunId
  )
  if ($RunId -notmatch '^[0-9a-f]{32}$') { return $false }
  $canonicalRunner = (Resolve-Path -LiteralPath $RunnerTemp).Path.TrimEnd('\')
  $candidate = Get-Item -LiteralPath $SmokeRoot -Force -ErrorAction SilentlyContinue
  if (-not $candidate -or -not $candidate.PSIsContainer -or ($candidate.Attributes -band [IO.FileAttributes]::ReparsePoint) -ne 0) { return $false }
  if ($candidate.Name -cne "mistake-trainer-installer-smoke-$RunId") { return $false }
  $canonicalCandidate = (Resolve-Path -LiteralPath $candidate.FullName).Path.TrimEnd('\')
  if ((Split-Path -Parent $canonicalCandidate) -cne $canonicalRunner) { return $false }

  $markerPath = Join-Path $canonicalCandidate '.mistake-trainer-installer-smoke.json'
  $markerItem = Get-Item -LiteralPath $markerPath -Force -ErrorAction SilentlyContinue
  if (-not $markerItem -or $markerItem.PSIsContainer -or $markerItem.Length -gt 4096 -or ($markerItem.Attributes -band [IO.FileAttributes]::ReparsePoint) -ne 0) { return $false }
  try { $marker = Get-Content -LiteralPath $markerPath -Raw | ConvertFrom-Json } catch { return $false }
  if ($marker.schemaVersion -ne 1 -or $marker.runId -cne $RunId) { return $false }
  if (Test-OwnedSmokeProcessPresent -Root $canonicalCandidate) { return $false }

  Remove-Item -LiteralPath $canonicalCandidate -Recurse -Force
  return $true
}
