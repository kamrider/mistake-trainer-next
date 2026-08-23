param(
  [string]$WorkflowDirectory = (Join-Path (Split-Path -Parent $PSScriptRoot) '.github\workflows')
)

$resolvedWorkflowDirectory = (Resolve-Path -LiteralPath $WorkflowDirectory -ErrorAction Stop).Path
$workflowFiles = @(Get-ChildItem -LiteralPath $resolvedWorkflowDirectory -File |
  Where-Object { $_.Extension -in @('.yml', '.yaml') })
if ($workflowFiles.Count -eq 0) {
  throw "No GitHub Actions workflow files were found under $resolvedWorkflowDirectory."
}

$violations = [System.Collections.Generic.List[string]]::new()
foreach ($workflowFile in $workflowFiles) {
  $lineNumber = 0
  foreach ($line in Get-Content -LiteralPath $workflowFile.FullName) {
    $lineNumber += 1
    if ($line -notmatch '^\s*(?:-\s*)?uses:\s*(?<reference>[^\s#]+)') {
      continue
    }
    $reference = $Matches.reference
    if ($reference.StartsWith('./')) {
      continue
    }
    if ($reference -notmatch '^[^@\s]+@[0-9a-fA-F]{40}$') {
      $violations.Add("$($workflowFile.Name):$lineNumber -> $reference")
    }
  }
}

if ($violations.Count -gt 0) {
  throw "GitHub Actions references must use full commit SHAs:`n$($violations -join "`n")"
}

$releaseWorkflowPath = Join-Path $resolvedWorkflowDirectory 'release-windows.yml'
if (-not (Test-Path -LiteralPath $releaseWorkflowPath -PathType Leaf)) {
  throw 'Signed Windows release workflow is missing.'
}
$releaseWorkflow = Get-Content -LiteralPath $releaseWorkflowPath -Raw
if ($releaseWorkflow -notmatch '(?m)^  verify-release-source:\s*$') {
  throw 'Signed Windows release workflow must verify the release source before signing.'
}
if ($releaseWorkflow -notmatch '(?m)^      source_commit: \$\{\{ steps\.verify\.outputs\.source_commit \}\}\s*$') {
  throw 'Release source verification must export the immutable source commit.'
}
$immutableCheckouts = [regex]::Matches(
  $releaseWorkflow,
  '(?m)^          ref: \$\{\{ needs\.verify-release-source\.outputs\.source_commit \}\}\s*$'
)
if ($immutableCheckouts.Count -ne 2) {
  throw "Signed and draft release jobs must both check out the verified immutable commit; found $($immutableCheckouts.Count) matching refs."
}
if ($releaseWorkflow -notmatch '(?m)^    needs: verify-release-source\s*$') {
  throw 'Signed Windows jobs must depend on release source verification.'
}
if ($releaseWorkflow -notmatch '(?m)^    needs: \[verify-release-source, signed-windows\]\s*$') {
  throw 'Draft release job must depend on source verification and both signed builds.'
}
$writePermissions = [regex]::Matches($releaseWorkflow, '(?m)^      contents: write\s*$')
if ($writePermissions.Count -ne 1) {
  throw "Only the draft release job may receive contents: write; found $($writePermissions.Count) grants."
}

$ciWorkflowPath = Join-Path $resolvedWorkflowDirectory 'ci.yml'
if (-not (Test-Path -LiteralPath $ciWorkflowPath -PathType Leaf)) {
  throw 'CI workflow is missing.'
}
$ciWorkflow = Get-Content -LiteralPath $ciWorkflowPath -Raw
$securityJobMatch = [regex]::Match(
  $ciWorkflow,
  '(?ms)^  security:\s*$.*?(?=^  [a-zA-Z0-9_-]+:\s*$)'
)
if (-not $securityJobMatch.Success) {
  throw 'CI must contain a dedicated security job.'
}
$securityJob = $securityJobMatch.Value
if ($securityJob -notmatch '(?m)^          fetch-depth: 0\s*$') {
  throw 'The security job must check out the complete Git history.'
}
if ($securityJob -notmatch '(?m)^        uses: gitleaks/gitleaks-action@[0-9a-fA-F]{40}\s+# v3\.0\.0\s*$') {
  throw 'The security job must pin Gitleaks action v3.0.0 to a full commit SHA.'
}
if ($securityJob -notmatch "(?m)^          GITLEAKS_VERSION: '8\.30\.1'\s*$") {
  throw 'The security job must pin the Gitleaks scanner version.'
}
if ($securityJob -notmatch '(?m)^        run: cargo install cargo-audit --version 0\.22\.2 --locked\s*$') {
  throw 'The security job must install the pinned RustSec auditor.'
}
if ($securityJob -notmatch '(?m)^        run: cargo audit --file src-tauri/Cargo\.lock\s*$') {
  throw 'The security job must audit the production Rust lockfile.'
}

Write-Output "GitHub Actions pin contract passed for $($workflowFiles.Count) workflow files."
