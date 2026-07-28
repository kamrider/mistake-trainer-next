# Library Tags Implementation Plan

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:subagent-driven-development (recommended) or superpowers:executing-plans to implement this plan task-by-task. Steps use checkbox (`- [ ]`) syntax for tracking.

**Goal:** Preserve the tags captured with each mistake as a complete library workflow: visible on cards and detail, searchable, safely editable, and included in canonical sync payloads.

**Architecture:** Keep `problems.tags_json` as the encrypted local source of truth and expose normalized `string[]` values through existing Rust DTOs. Validation remains in the Rust use case; Vue provides a keyboard-accessible chip editor but cannot bypass the 20-tag/30-character contract. Existing outbox canonicalization already reloads `tags_json`, so an atomic problem update automatically produces the correct cloud aggregate.

**Tech Stack:** Rust, rusqlite/SQLCipher, serde, Tauri Specta bindings, Vue 3, TypeScript strict, Vitest, Testing Library.

**Status (2026-07-22):** Implemented and verified as one cohesive local commit. Focused Rust tests, 167 Vue tests, lint, strict typecheck, production build, and Rust `--all-targets` all pass.

## Global Constraints

- One problem may contain at most 20 tags; each trimmed tag may contain at most 30 Unicode characters.
- Empty tags are removed and duplicate tags are removed in first-seen order.
- Search input remains bounded to 100 characters and treats `%`, `_`, and `\` as literals.
- Vue receives no account ID, profile ID, database handle, file path, or raw encrypted asset metadata.
- Tag chip motion uses existing 120/180 ms tokens and disappears under `prefers-reduced-motion: reduce`.
- No new runtime dependency is introduced.

---

### Task 1: Rust tag query and mutation contract

**Files:**
- Modify: `src-tauri/src/modules/problems.rs`
- Modify: `src-tauri/src/commands/library.rs`
- Test: `src-tauri/tests/problem_query.rs`
- Test: `src-tauri/tests/problem_detail.rs`
- Test: `src-tauri/tests/problem_lifecycle.rs`

**Interfaces:**
- Produces: `ProblemSummary.tags: Vec<String>` and `ProblemDetail.tags: Vec<String>`.
- Consumes: `UpdateProblem.tags: Vec<String>` and `ProblemUpdateInput.tags: Vec<String>`.
- Produces: `ProblemUseCaseError::InvalidTags` and public code `problem_tags_invalid`.

- [ ] **Step 1: Write failing query and detail tests**

  Add fixtures with `tags_json = '["函数","粗心"]'`. Assert summaries and details return both tags and a search for `粗心` returns the problem while `%` remains literal.

- [ ] **Step 2: Run focused query/detail tests and verify failure**

  Run: `.\scripts\cargo-msvc.cmd test --manifest-path src-tauri\Cargo.toml --test problem_query --test problem_detail`

  Expected: compilation fails because `ProblemSummary` and `ProblemDetail` do not expose `tags`.

- [ ] **Step 3: Implement query DTOs and tag search**

  Select `p.tags_json` with summaries and details, deserialize it into `Vec<String>`, and extend the bounded search predicate with:

  ```sql
  OR EXISTS (
    SELECT 1 FROM json_each(p.tags_json) tag
    WHERE CAST(tag.value AS TEXT) LIKE '%' || ?4 || '%' ESCAPE '\'
  )
  ```

  Treat malformed stored JSON as a use-case error rather than returning an empty value.

- [ ] **Step 4: Write failing atomic update tests**

  Update a problem with whitespace, duplicates, and two valid tags. Assert the row stores normalized JSON, revision increments exactly once, and the newest outbox operation canonicalizes to the new tag list. Add invalid cases for 21 tags and a 31-character tag; assert row and outbox counts remain unchanged.

- [ ] **Step 5: Implement normalized atomic updates**

  Normalize before opening the transaction. Store `tags_json` in the same optimistic `UPDATE` as subject, note, time limit, timestamp, and revision. Add `tags` to the lightweight diagnostic payload; the push lease continues to rebuild the canonical aggregate from the database.

- [ ] **Step 6: Map the typed command input and public error**

  Extend `ProblemUpdateInput` and `problem_update_for`; map `InvalidTags` to a non-retryable Chinese message explaining the 20/30 limits.

- [ ] **Step 7: Run focused Rust tests**

  Run: `.\scripts\cargo-msvc.cmd test --manifest-path src-tauri\Cargo.toml --test problem_query --test problem_detail --test problem_lifecycle --test library_command`

  Expected: PASS.

- [ ] **Step 8: Commit the Rust slice**

  ```powershell
  git add src-tauri/src/modules/problems.rs src-tauri/src/commands/library.rs src-tauri/tests/problem_query.rs src-tauri/tests/problem_detail.rs src-tauri/tests/problem_lifecycle.rs
  git commit -m "feat: preserve tags across the mistake library"
  ```

---

### Task 2: Keyboard-friendly tag chip editor

**Files:**
- Create: `src/modules/library/components/ProblemTagEditor.vue`
- Create: `src/modules/library/components/ProblemTagEditor.test.ts`

**Interfaces:**
- Consumes: `modelValue: string[]`, `disabled?: boolean`.
- Produces: `update:modelValue` with normalized tags.

- [ ] **Step 1: Write failing component tests**

  Assert Enter, comma, and Chinese comma add a trimmed tag; duplicate and empty input do not add; Backspace on an empty input removes the last tag; every tag has a labelled remove button; and the 20/30 limits produce an inline alert without emitting invalid data.

- [ ] **Step 2: Run the focused Vue test and verify failure**

  Run: `pnpm test -- src/modules/library/components/ProblemTagEditor.test.ts`

  Expected: FAIL because the component does not exist.

- [ ] **Step 3: Implement the editor**

  Use a `TransitionGroup` named `tag-chip`, a visible `标签` label, an input with `aria-describedby`, and removable chips. Keep focus in the input after additions/removals. The component must not trim or reorder the parent array outside explicit user actions.

- [ ] **Step 4: Add restrained motion and reduced-motion fallback**

  New chips enter with opacity plus `scale(.92)` over `var(--motion-standard)`; removed chips fade over `var(--motion-feedback)`. Under reduced motion, remove transition and transform.

- [ ] **Step 5: Run focused tests**

  Run: `pnpm test -- src/modules/library/components/ProblemTagEditor.test.ts`

  Expected: PASS.

- [ ] **Step 6: Commit the editor slice**

  ```powershell
  git add src/modules/library/components/ProblemTagEditor.vue src/modules/library/components/ProblemTagEditor.test.ts
  git commit -m "feat: add accessible problem tag editor"
  ```

---

### Task 3: Library cards, detail wiring, and release gates

**Files:**
- Modify: `src/modules/library/components/LibraryWorkspace.vue`
- Modify: `src/modules/library/components/LibraryWorkspace.test.ts`
- Modify: `src/modules/library/components/ProblemDetailDrawer.vue`
- Modify: `src/modules/library/components/ProblemDetailDrawer.test.ts`
- Modify: `src/app/views/LibraryView.vue`
- Modify: `src/app/views/LibraryView.test.ts`
- Modify: `src/shared/api/bindings.ts` (generated)
- Modify: `docs/plans/library.md`

**Interfaces:**
- Consumes: generated `ProblemSummary.tags`, `ProblemDetail.tags`, and `ProblemUpdateInput.tags`.
- Produces: tag chips on every library card, tag display/edit in the drawer, and tag-aware save orchestration.

- [ ] **Step 1: Write failing card and drawer tests**

  Assert cards show the first three tags plus `+N`; the detail read view shows every tag; edit mode initializes the tag editor; dirty-state protection includes tag changes; and saving emits the normalized tag array.

- [ ] **Step 2: Regenerate bindings**

  Run: `pnpm bindings:generate`

  Expected: generated DTOs contain the new tag properties and update input.

- [ ] **Step 3: Implement card and detail presentation**

  Render compact paper chips below the note and use a `TransitionGroup` for card list changes. In the drawer, show tags beside the review note and mount `ProblemTagEditor` in edit mode. Change search copy to `搜索科目、标签或复盘笔记`.

- [ ] **Step 4: Wire update orchestration and preview fixtures**

  Pass `tags` through `LibraryView.updateProblem`; update browser preview fixtures and all typed test fixtures so development preview and tests use the real contract.

- [ ] **Step 5: Run focused frontend tests**

  Run: `pnpm test -- src/modules/library/components/ProblemTagEditor.test.ts src/modules/library/components/LibraryWorkspace.test.ts src/modules/library/components/ProblemDetailDrawer.test.ts src/app/views/LibraryView.test.ts`

  Expected: PASS.

- [ ] **Step 6: Run full quality gates**

  Run: `pnpm lint`, `pnpm typecheck`, `pnpm test`, `pnpm bindings:check`, `.\scripts\cargo-msvc.cmd test --manifest-path src-tauri\Cargo.toml --all-targets`, and `pnpm build`.

  Expected: PASS with only the documented SQLCipher `VirtualLock` and OpenSSL PDB warnings.

- [ ] **Step 7: Update library plan and commit**

  Record visible/searchable/editable tags and the 20/30 validation boundary in `docs/plans/library.md`.

  ```powershell
  git add src docs/plans/library.md
  git commit -m "feat: complete the library tag workflow"
  ```
