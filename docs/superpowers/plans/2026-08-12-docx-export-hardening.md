# DOCX Export Hardening Implementation Plan

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:subagent-driven-development (recommended) or superpowers:executing-plans to implement this plan task-by-task. Steps use checkbox (`- [ ]`) syntax for tracking.

**Goal:** Produce readable, atomic Word documents for representative and 500-problem exports, with deterministic headings, page boundaries, missing-role copy, and truthful disk/corruption failures.

**Architecture:** Keep all decryption and DOCX packaging in Rust. Build into a unique temporary file, validate inputs before writing, fsync, and atomically rename. Add Word-oriented paragraph styles and explicit page breaks without leaking local asset paths.

**Tech Stack:** Rust, `docx-rs`, `image`, ZIP inspection tests, Microsoft Word manual acceptance.

## Global Constraints

- Never write decrypted source images outside the user-selected export destination.
- Never leave a partial `.docx` after generation fails.
- Keep a maximum of 500 selected problems, 2,000 assets, and the existing byte/pixel safety caps.
- Output filenames must remain unique even when titles are identical.

---

### Task 1: Add readable Word structure

**Files:**
- Modify: `src-tauri/src/modules/exports_generation.rs`
- Test: `src-tauri/tests/export_generation.rs`

**Interfaces:**
- Consumes: `ExportLayout::{QuestionAnswerAlternating, QuestionsThenAnswers}`.
- Produces: `generate_docx(...) -> Result<String, ExportError>` with title, section headings, problem headings, role labels, and page breaks encoded in `word/document.xml`.

- [ ] **Step 1: Write XML-level failing assertions**

```rust
let xml = document_xml(&path);
assert!(xml.contains("w:val=\"Title\""));
assert!(xml.contains("w:val=\"Heading1\""));
assert!(xml.contains("w:type=\"page\""));
assert!(!xml.contains(directory.path().to_string_lossy().as_ref()));
```

- [ ] **Step 2: Run the focused test and observe the missing style/page-break failure**

Run: `.\scripts\cargo-msvc.cmd test --manifest-path src-tauri\Cargo.toml --test export_generation`

Expected: FAIL before the DOCX structure is added.

- [ ] **Step 3: Introduce focused paragraph helpers**

Add helpers with these signatures:

```rust
fn title_paragraph(title: &str) -> Paragraph;
fn section_heading(label: &str) -> Paragraph;
fn problem_heading(index: usize, problem: &ExportProblem, section: &str) -> Paragraph;
fn page_break_paragraph() -> Paragraph;
```

Use Word built-in styles `Title`, `Heading1`, and `Heading2`. Insert a page break between the questions and answers sections, and between alternating problems when another problem follows.

- [ ] **Step 4: Run the focused Rust test**

Run: `.\scripts\cargo-msvc.cmd test --manifest-path src-tauri\Cargo.toml --test export_generation`

Expected: PASS.

- [ ] **Step 5: Commit**

```bash
git add src-tauri/src/modules/exports_generation.rs src-tauri/tests/export_generation.rs
git commit -m "feat: improve DOCX document structure"
```

### Task 2: Cover corruption, disk pressure, and large exports

**Files:**
- Modify: `src-tauri/tests/export_generation.rs`
- Modify: `src-tauri/src/modules/exports_generation.rs`
- Modify: `src-tauri/src/commands/exports.rs`

**Interfaces:**
- Consumes: encrypted assets and an absolute existing destination directory.
- Produces: stable `ExportError::{InvalidImage, File, ExportTooLarge}` mappings and no partial output.

- [ ] **Step 1: Add failure fixtures**

Create tests that truncate an encrypted asset, remove an asset, generate two snapshots with the same title, select exactly 500 one-image problems, and force the destination write to fail by passing a read-only/non-directory target.

- [ ] **Step 2: Assert atomic cleanup**

```rust
assert!(destination.path().read_dir().unwrap().all(|entry| {
    let name = entry.unwrap().file_name().to_string_lossy().into_owned();
    !name.starts_with('.') || !name.ends_with(".tmp")
}));
```

- [ ] **Step 3: Run export store and generation tests**

Run: `.\scripts\cargo-msvc.cmd test --manifest-path src-tauri\Cargo.toml --test export_store --test export_generation`

Expected: all export tests pass and failures retain truthful error codes.

- [ ] **Step 4: Commit**

```bash
git add src-tauri/src/modules/exports_generation.rs src-tauri/src/commands/exports.rs src-tauri/tests/export_generation.rs
git commit -m "test: harden DOCX export failure handling"
```

### Task 3: Approve representative documents in Word

**Files:**
- Modify: `docs/windows-export-acceptance.md`

**Interfaces:**
- Consumes: generated alternating and grouped DOCX fixtures.
- Produces: Word version, OS build, sample size, layout, page-break, image clarity, and failure-path evidence.

- [ ] **Step 1: Generate representative exports**

Generate documents containing portrait photos, landscape geometry, tall answer images, missing-answer problems, Chinese notes, duplicate titles, and a 500-problem snapshot.

- [ ] **Step 2: Inspect with supported Microsoft Word**

Confirm title/section hierarchy, problem order, page breaks, no clipped images, readable notes, correct question-answer association, and no repair warning on open.

- [ ] **Step 3: Record exact evidence**

Write PASS/FAIL rows with Word version and document SHA-256. Failed rows stay open until a replacement artifact passes.

- [ ] **Step 4: Commit**

```bash
git add docs/windows-export-acceptance.md
git commit -m "test: approve DOCX export in Word"
```
