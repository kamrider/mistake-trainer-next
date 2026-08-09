# GitHub Actions SHA Pinning Implementation Plan

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:subagent-driven-development (recommended) or superpowers:executing-plans to implement this plan task-by-task. Steps use checkbox (`- [ ]`) syntax for tracking.

**Goal:** Make every third-party GitHub Action used by CI and Windows release workflows resolve to an immutable official commit and prevent tag-based references from returning.

**Architecture:** Add a repository contract script that scans every workflow `uses:` entry and rejects non-local references unless they end in a 40-character commit SHA. First prove the contract fails against the existing tag-based workflows, then pin all official actions to commits resolved from their upstream Git repositories and run the contract in CI.

**Tech Stack:** GitHub Actions YAML, PowerShell 7/Windows PowerShell, Git remote references.

## Global Constraints

- Do not change any Action inputs, job permissions, runner labels, build commands, release secrets, or artifact paths.
- Pin only to commits resolved from each Action's official `github.com/<owner>/<repository>` Git repository on 2026-07-28.
- Keep a human-readable version comment after every SHA.
- Dereference annotated Git tags to their executable commit before pinning.
- Local actions beginning with `./` are permitted without a SHA; Docker references and tag/branch references are rejected.
- Do not modify the current uncommitted recognition work.

## Verified upstream mapping

| Action | Commit SHA | Version comment |
| --- | --- | --- |
| `actions/checkout` | `11d5960a326750d5838078e36cf38b85af677262` | `v4.4.0` |
| `pnpm/action-setup` | `b906affcce14559ad1aafd4ab0e942779e9f58b1` | `v4.3.0` |
| `actions/setup-node` | `49933ea5288caeca8642d1e84afbd3f7d6820020` | `v4.4.0` |
| `dtolnay/rust-toolchain` | `4cda84d5c5c54efe2404f9d843567869ab1699d4` | `stable` |
| `actions/upload-artifact` | `ea165f8d65b6e75b540449e92b4886f43607fa02` | `v4.6.2` |
| `actions/download-artifact` | `d3f86a106a0bac45b974a628896c90dbdf5c8093` | `v4.3.0` |
| `supabase/setup-cli` | `ab058987d8d6c725971f6cf9d0b5c98467e30bd1` | `v1.7.1` |

---

### Task 1: Workflow pinning contract

**Files:**
- Create: `scripts/github-actions-pin-contract.ps1`

**Interfaces:**
- Consumes: optional `-WorkflowDirectory`; defaults to `<repository>/.github/workflows`.
- Produces: exit code `0` and a count for fully pinned workflows; throws with `file:line -> reference` entries for violations.

- [x] **Step 1: Create the scanner**

Create:

```powershell
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

Write-Output "GitHub Actions pin contract passed for $($workflowFiles.Count) workflow files."
```

- [x] **Step 2: Run the contract and verify it fails**

Run:

```powershell
.\scripts\github-actions-pin-contract.ps1
```

Expected: FAIL and list the current `@v4`, `@stable`, and `@v1` references in `ci.yml` and `release-windows.yml`.

### Task 2: Pin CI and release Actions

**Files:**
- Modify: `.github/workflows/ci.yml`
- Modify: `.github/workflows/release-windows.yml`

**Interfaces:**
- Consumes: the verified upstream mapping in this plan.
- Produces: identical workflow behavior with every external Action reference pinned to an immutable commit.

- [x] **Step 1: Replace every external reference**

Use these exact forms wherever the matching Action occurs:

```yaml
uses: actions/checkout@11d5960a326750d5838078e36cf38b85af677262 # v4.4.0
uses: pnpm/action-setup@b906affcce14559ad1aafd4ab0e942779e9f58b1 # v4.3.0
uses: actions/setup-node@49933ea5288caeca8642d1e84afbd3f7d6820020 # v4.4.0
uses: dtolnay/rust-toolchain@4cda84d5c5c54efe2404f9d843567869ab1699d4 # stable
uses: actions/upload-artifact@ea165f8d65b6e75b540449e92b4886f43607fa02 # v4.6.2
uses: actions/download-artifact@d3f86a106a0bac45b974a628896c90dbdf5c8093 # v4.3.0
uses: supabase/setup-cli@ab058987d8d6c725971f6cf9d0b5c98467e30bd1 # v1.7.1
```

- [x] **Step 2: Run the contract and verify it passes**

Run:

```powershell
.\scripts\github-actions-pin-contract.ps1
```

Expected: `GitHub Actions pin contract passed for 2 workflow files.`

- [x] **Step 3: Confirm no tag or branch reference remains**

Run:

```powershell
rg -n "uses:\s*[^#\s]+@(v\d+|stable|main|master)\b" .github/workflows
```

Expected: no matches.

### Task 3: Enforce the contract in CI

**Files:**
- Modify: `.github/workflows/ci.yml`

**Interfaces:**
- Consumes: `scripts/github-actions-pin-contract.ps1`.
- Produces: a required CI step named `Verify GitHub Actions pin contract`.

- [x] **Step 1: Add the contract step**

Immediately after the first checkout in the `app` job, add:

```yaml
- name: Verify GitHub Actions pin contract
  shell: powershell
  run: .\scripts\github-actions-pin-contract.ps1
```

- [x] **Step 2: Run repository verification**

Run:

```powershell
.\scripts\github-actions-pin-contract.ps1
pnpm lint
pnpm typecheck
pnpm test:coverage -- --reporter=dot
pnpm build
git diff --check
```

Expected: all commands exit with code `0`; coverage remains above the configured floors.

- [x] **Step 3: Review the final workflow diff**

Run:

```powershell
git diff -- .github/workflows/ci.yml .github/workflows/release-windows.yml scripts/github-actions-pin-contract.ps1
```

Expected: only immutable Action refs, readable version comments, and the new enforcement step/script.
