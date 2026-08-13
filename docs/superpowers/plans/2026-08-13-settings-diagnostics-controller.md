# Settings Diagnostics Export Controller Plan

**Goal:** Move privacy-safe diagnostic export state, native cancellation, retry behavior, single-flight, stale-receipt clearing, and focus restoration out of `SettingsView.vue`.

**Architecture:** `useSettingsDiagnosticsExport` consumes one normalized export operation plus desktop-availability and focus capabilities. It exposes readonly receipt/busy/message state and one `exportDiagnostics` action. The view only adapts the generated command and formats the accepted receipt timestamp.

### Task 1: Tests red

- [x] Create `useSettingsDiagnosticsExport.test.ts` covering success, cancellation, backend failure, transport failure, duplicate clicks, stale receipt clearing, desktop-disabled behavior, and focus restoration.
- [x] Run focused test and verify missing-module failure.

### Task 2: Implement

- [x] Create explicit options/operations interfaces.
- [x] Implement readonly state and one shared task/promise for single-flight.
- [x] Preserve exact generic failure copy and never expose thrown/native diagnostics.
- [x] Run focused tests green.

### Task 3: Compose and lock ownership

- [x] Adapt `diagnosticsExport` in `SettingsView.vue`; delete local refs/function.
- [x] Run controller and SettingsView tests plus typecheck.
- [x] Add architecture source assertions and document ownership.
- [x] Run architecture, type, lint, all tests, and diff checks.
- [x] Recount SettingsView and continue with device access/lock.
