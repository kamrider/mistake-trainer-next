# Windows Install And Upgrade Hardening Implementation Plan

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:subagent-driven-development (recommended) or superpowers:executing-plans to implement this plan task-by-task. Steps use checkbox (`- [ ]`) syntax for tracking.

**Goal:** Finish the isolated Windows installer smoke path and prove fresh install, first launch, upgrade, uninstall, rollback safety, x64, and ARM64 behavior without touching a developer's production profile.

**Architecture:** Keep `windows-installer-smoke.ps1` as the only public entry point. Local production-identity smoke runs inside Windows Sandbox; explicitly ephemeral CI runners may invoke the inner runner. The inner runner owns only processes it creates, records signed stage evidence, and returns a non-zero exit code for every failed stage.

**Tech Stack:** PowerShell 7/Windows PowerShell, Windows Sandbox, NSIS, Tauri 2, Vitest contract tests, GitHub Actions.

## Global Constraints

- Never run a production-identity installer smoke directly in a normal developer Windows session.
- Never read credential values or delete paths that are not owned by the current smoke run.
- The tested executable must come from the real installer artifact, not a test-only build.
- Preserve the existing unsigned local/free updater release policy documented in `docs/windows-release-runbook.md`.

---

### Task 1: Lock the smoke process and result contract

**Files:**
- Modify: `scripts/windows-installer-smoke-inner.ps1`
- Modify: `scripts/windows-installer-smoke-sandbox.ps1`
- Test: `tests/windows-installer-smoke-isolation.test.ts`

**Interfaces:**
- Consumes: `scripts/windows-installer-smoke.ps1 -InstallerPath <absolute-path>`.
- Produces: a signed result JSON whose `status` is `passed` only after install, GUI launch, navigation, clean shutdown, and uninstall all complete.

- [ ] **Step 1: Keep the isolation tests red for any unsafe fallback**

```ts
expect(inner).toContain("MISTAKE_TRAINER_EPHEMERAL_WINDOWS")
expect(host).toContain("windows-installer-smoke-sandbox.ps1")
expect(inner).not.toMatch(/Stop-Process\s+-Name/)
```

- [ ] **Step 2: Run the focused contract**

Run: `pnpm exec vitest run tests/windows-installer-smoke-isolation.test.ts`

Expected: PASS; removing the environment gate, owned PID handling, or final success return makes the test fail.

- [ ] **Step 3: Verify the current branch diff contains no host-profile access**

Run: `git diff origin/main...HEAD -- scripts/windows-installer-smoke-inner.ps1 scripts/windows-installer-smoke-sandbox.ps1`

Expected: all writable roots are guest/run-owned paths and production credentials are represented only by non-secret fingerprints.

- [ ] **Step 4: Commit the contract checkpoint**

```bash
git add scripts/windows-installer-smoke-inner.ps1 scripts/windows-installer-smoke-sandbox.ps1 tests/windows-installer-smoke-isolation.test.ts
git commit -m "fix: complete isolated installer smoke contract"
```

### Task 2: Prove application and release orchestration compatibility

**Files:**
- Modify: `src/app/App.test.ts`
- Modify: `docs/windows-release-runbook.md`
- Modify: `CHANGELOG.md`
- Test: `tests/windows-installer-smoke-isolation.test.ts`

**Interfaces:**
- Consumes: the production first-launch lock and updater-signed installer artifacts.
- Produces: documented local Sandbox and ephemeral CI procedures with bounded cold-start timing.

- [ ] **Step 1: Run the first-launch and isolation tests together**

Run: `pnpm exec vitest run src/app/App.test.ts tests/windows-installer-smoke-isolation.test.ts`

Expected: PASS with first-launch locking preserved and the GUI smoke timeout bounded.

- [ ] **Step 2: Run repository static gates**

Run: `pnpm lint`

Expected: exit code 0 and zero warnings.

Run: `pnpm typecheck`

Expected: exit code 0.

- [ ] **Step 3: Build the production frontend**

Run: `pnpm build`

Expected: mobile vendor verification, Vue type checking, and Vite production build pass.

- [ ] **Step 4: Document the exact manual matrix**

The runbook table must include: clean Windows 11 x64 install, first launch, upgrade from the previous updater-signed build, rejected downgrade, uninstall preserving user-selected data policy, rollback artifact availability, ARM64 native install, 150% DPI, antivirus scan, and interrupted smoke cleanup.

- [ ] **Step 5: Commit the orchestration checkpoint**

```bash
git add src/app/App.test.ts docs/windows-release-runbook.md CHANGELOG.md
git commit -m "docs: finalize Windows upgrade acceptance"
```

### Task 3: Record installed-product evidence

**Files:**
- Modify: `docs/releases/0.1.1.md`

**Interfaces:**
- Consumes: x64 and ARM64 artifacts from the protected release workflow.
- Produces: hashes, workflow URLs, architecture, updater signature result, and every manual-matrix result.

- [ ] **Step 1: Run the x64 installed-product smoke on an ephemeral Windows worker**

Run: `powershell -NoProfile -ExecutionPolicy Bypass -File scripts/windows-installer-smoke.ps1 -InstallerPath <x64-installer>`

Expected: result JSON reports `passed`; the worker has no surviving application process.

- [ ] **Step 2: Repeat for ARM64**

Run the same command on native Windows ARM64 hardware with the ARM64 installer.

Expected: install, launch, navigation, shutdown, and uninstall all pass natively.

- [ ] **Step 3: Record evidence, not credentials**

Add artifact SHA-256, updater signature verification, OS build, architecture, timings, and PASS/FAIL rows. Do not record secrets, email addresses, credential contents, or private local paths.

- [ ] **Step 4: Commit the evidence ledger**

```bash
git add docs/releases/0.1.1.md
git commit -m "test: record Windows install and upgrade evidence"
```
