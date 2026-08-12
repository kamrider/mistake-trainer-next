# Installed Product Real-File Gates Implementation Plan

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:subagent-driven-development (recommended) or superpowers:executing-plans to implement this plan task-by-task. Steps use checkbox (`- [ ]`) syntax for tracking.

**Goal:** Make the packaged Windows product prove backup restore and DOCX generation in its isolated golden-path check, while documenting the remaining human-only PDF and Word compatibility matrix.

**Architecture:** Extend the existing `--windows-product-check` backend path because the x64 and ARM64 installer jobs already execute it after installation. Keep the check synthetic, offline, bounded, and independent of the user's profile; use the same domain APIs as production and assert generated artifacts before deleting the owned scratch workspace.

**Tech Stack:** Rust 1.97, SQLCipher/rusqlite, docx-rs, Windows installer smoke PowerShell, cargo tests.

## Global Constraints

- Never touch the real Windows credential store or production application data during product checks.
- Keep every fixture generated in the product-check-owned scratch directory and remove it afterward.
- Do not claim that XML inspection replaces opening representative files in desktop Word.

---

### Task 1: Restore and export golden path

**Files:**
- Modify: `src-tauri/src/modules/product_check.rs`
- Test: `src-tauri/src/modules/product_check.rs`

**Interfaces:**
- Consumes: `prepare_backup_restore`, `schedule_backup_restore`, `begin_pending_restore`, `create_export_snapshot`, and `generate_export`.
- Produces: `WindowsProductChecks.backup_restore` and `WindowsProductChecks.docx_export` booleans in the installed-product report.

- [x] **Step 1: Write failing report-contract tests**

```rust
assert_eq!(report["checks"]["backupRestore"], true);
assert_eq!(report["checks"]["docxExport"], true);
```

- [x] **Step 2: Run the focused tests and confirm the new fields are absent**

Run: `cargo test --manifest-path src-tauri/Cargo.toml modules::product_check`
Expected: FAIL because the product check report does not expose restore/export checks.

- [x] **Step 3: Add bounded restore and DOCX generation checks**

```rust
let candidate = prepare_backup_restore(&package, workspace, key, asset_key, account, now)?;
schedule_backup_restore(workspace, &candidate.id, key, asset_key, account, now + 1)?;
let swap = begin_pending_restore(workspace, library_root, key, asset_key, account, now + 2)?;
swap.commit()?;

let snapshot = create_export_snapshot(&mut connection, CreateExportSnapshot { /* owned ids */ })?;
let generated = generate_export(&connection, blob_root, asset_key, account, profile, &snapshot.id, export_root)?;
assert!(export_root.join(generated.output_name).is_file());
```

- [x] **Step 4: Run product-check and backup/export tests**

Run: `cargo test --manifest-path src-tauri/Cargo.toml modules::product_check backup_restore_startup exports_store`
Expected: PASS.

### Task 2: Acceptance evidence ledger

**Files:**
- Modify: `docs/windows-backup-restore-acceptance.md`
- Modify: `docs/windows-exam-acceptance.md`
- Modify: `docs/windows-release-runbook.md`

**Interfaces:**
- Consumes: CI product-check JSON fields.
- Produces: an exact split between automated evidence and manual Word/PDF/device evidence.

- [x] **Step 1: Record automated coverage and leave human-only boxes unchecked**

```markdown
- [x] Installed x64 and ARM64 product checks exercise encrypted create/read/review/backup/restore/DOCX generation.
- [ ] Open the representative DOCX in supported desktop Word and inspect pagination and image readability.
- [ ] Import the approved real-file corpus: multipage, password-protected, malformed, empty, and near-limit PDFs.
```

- [ ] **Step 2: Verify release source contracts still pass**

Run: `powershell -NoProfile -ExecutionPolicy Bypass -File scripts/windows-release-contract.ps1`
Expected: PASS.
