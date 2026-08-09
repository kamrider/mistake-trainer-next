# Commercial Quality Gates Implementation Plan

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:subagent-driven-development (recommended) or superpowers:executing-plans to implement this plan task-by-task. Steps use checkbox (`- [ ]`) syntax for tracking.

**Goal:** Turn the current frontend coverage baseline and dependency update practices into reproducible commercial CI gates.

**Architecture:** Keep Vitest as the single frontend test runner and add conservative global thresholds below the measured baseline. Run the coverage gate in the existing Windows CI job, pin the Supabase CLI to the repository's declared version, and let Dependabot propose reviewed dependency updates without changing release behavior.

**Tech Stack:** Vitest 4 with V8 coverage, GitHub Actions, Dependabot v2, pnpm, Cargo.

## Global Constraints

- Do not modify application runtime behavior or the current uncommitted recognition work.
- Use the measured baseline from 2026-07-28: statements 71.43%, branches 72.08%, functions 67.61%, lines 74.29%.
- Initial thresholds must detect material regression while leaving at least two percentage points of headroom.
- Dependency updates remain pull requests reviewed through the existing CI and release process.
- Do not introduce floating `latest` tool versions.

---

### Task 1: Enforce frontend coverage floors

**Files:**
- Modify: `vite.config.ts`
- Modify: `.github/workflows/ci.yml:36`

**Interfaces:**
- Consumes: the existing `pnpm test:coverage` script and `@vitest/coverage-v8`.
- Produces: global coverage floors of 70% statements, 70% branches, 65% functions, and 70% lines; CI runs the coverage command instead of the non-coverage duplicate.

- [x] **Step 1: Configure V8 coverage thresholds**

Add inside the existing `test` object:

```ts
coverage: {
  provider: 'v8',
  reporter: ['text', 'json-summary'],
  thresholds: {
    statements: 70,
    branches: 70,
    functions: 65,
    lines: 70,
  },
},
```

- [x] **Step 2: Run the coverage gate**

Run:

```powershell
pnpm test:coverage -- --reporter=dot
```

Expected: 56 test files and 292 tests pass; all four measured totals remain above the configured floors.

- [x] **Step 3: Make CI run the coverage gate**

Replace:

```yaml
- run: pnpm test
```

with:

```yaml
- run: pnpm test:coverage
```

The workflow must not run both commands because coverage already executes the complete suite.

### Task 2: Reproducible dependency update inputs

**Files:**
- Modify: `.github/workflows/ci.yml:111-113`
- Create: `.github/dependabot.yml`

**Interfaces:**
- Consumes: Supabase CLI `2.109.1` already declared in `package.json`.
- Produces: an exact Supabase test CLI version and weekly reviewed update pull requests for pnpm, Cargo, and GitHub Actions.

- [x] **Step 1: Pin the Supabase CLI action input**

Change:

```yaml
version: latest
```

to:

```yaml
version: 2.109.1
```

- [x] **Step 2: Add Dependabot configuration**

Create `.github/dependabot.yml`:

```yaml
version: 2
updates:
  - package-ecosystem: npm
    directory: /
    schedule:
      interval: weekly
    open-pull-requests-limit: 5
  - package-ecosystem: cargo
    directory: /src-tauri
    schedule:
      interval: weekly
    open-pull-requests-limit: 5
  - package-ecosystem: github-actions
    directory: /
    schedule:
      interval: weekly
    open-pull-requests-limit: 5
```

- [x] **Step 3: Verify no floating tool version remains**

Run:

```powershell
rg -n "version:\s*latest|@latest" .github package.json pnpm-lock.yaml
```

Expected: no matches.

### Task 3: Verify the commercial quality-gate change set

**Files:**
- Verify: `vite.config.ts`
- Verify: `.github/workflows/ci.yml`
- Verify: `.github/dependabot.yml`

**Interfaces:**
- Consumes: Tasks 1–2.
- Produces: a linted, typed, tested, buildable, and whitespace-clean gate configuration.

- [x] **Step 1: Run frontend gates**

Run:

```powershell
pnpm lint
pnpm typecheck
pnpm test:coverage -- --reporter=dot
pnpm build
```

Expected: all commands exit with code `0`.

- [x] **Step 2: Review configuration and diff**

Run:

```powershell
git diff --check
git diff -- vite.config.ts .github/workflows/ci.yml .github/dependabot.yml
```

Expected: no whitespace errors; the diff contains only coverage enforcement, the exact Supabase version, and Dependabot schedules.
