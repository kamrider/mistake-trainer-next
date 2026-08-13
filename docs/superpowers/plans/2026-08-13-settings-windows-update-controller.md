# Settings Windows Update Controller Extraction Plan

> Execute in order. Preserve the distinct product semantics of silent throttled startup checks and explicit settings checks.

**Goal:** Move Windows compatibility/status probes, explicit update check/install state, exact-version installation policy, privacy-safe errors, publication formatting, and focus restoration out of `SettingsView.vue`.

**Architecture:** `useSettingsWindowsUpdate` consumes normalized compatibility, status, check, and exact-version install operations plus a focus callback. It owns only the explicit Settings experience. `useStartupUpdate` remains the silent/throttled startup policy; both controllers use the same generated DTO vocabulary but do not share UI state or throttling rules.

**Why next:** `SettingsView.vue` is now 887 lines. The update cluster is the highest-risk remaining slice because it makes network requests, installs only a verified version, clears stale metadata after errors, and must never render private updater diagnostics. It is already protected by four end-to-end settings tests.

## Required behavior

- Compatibility and update-status probes are supplementary and silent on failure.
- Check/install are mutually exclusive and duplicate clicks are rejected.
- Check clears stale report/message, maps backend messages exactly, and restores focus.
- Install sends only the currently displayed exact version.
- Any install application/transport failure invalidates the stale report and remains retryable.
- Publication labels are derived only from accepted metadata and malformed dates render empty.

### Task 1: Write controller tests red

**Files:**
- Create: `src/app/composables/useSettingsWindowsUpdate.test.ts`

- [x] Test silent compatibility/status loads and disabled builds.
- [x] Test latest-version and available-version check copy.
- [x] Test check single-flight, backend/transport failures, and focus restoration.
- [x] Test exact-version install, stale-report invalidation, install single-flight, and success copy.
- [x] Test publication formatting for valid, absent, and malformed timestamps.
- [x] Run focused test and verify missing-module failure.

### Task 2: Implement controller

**Files:**
- Create: `src/app/composables/useSettingsWindowsUpdate.ts`
- Modify: focused test.

- [x] Export explicit normalized operations/options interfaces.
- [x] Own readonly compatibility/status/report/checking/installing/message/publication state.
- [x] Implement supplementary loads and explicit check/install workflows with exact existing copy.
- [x] Restore focus after every explicit check/install completion.
- [x] Run focused tests green.

### Task 3: Compose in SettingsView

**Files:**
- Modify: `src/app/views/SettingsView.vue`
- Verify: `src/app/views/SettingsView.test.ts`

- [x] Adapt four generated commands and inject the update-panel focus callback.
- [x] Delete update/compatibility refs, three load/action functions, install function, and publication formatter from the view.
- [x] Keep supplementary load registration and template bindings unchanged through destructuring.
- [x] Run focused controller and full SettingsView tests plus typecheck.

### Task 4: Lock ownership and verify

**Files:**
- Modify: `tests/architecture-dependency-direction.test.ts`
- Modify: `docs/architecture.md`
- Modify: this plan checkboxes.

- [x] Add source ownership assertions preventing update workflow return to SettingsView.
- [x] Document explicit settings-update ownership versus silent startup-update ownership.
- [x] Run architecture, type, lint, all tests, and diff checks.
- [x] Recount SettingsView and choose diagnostics or device-lock next.
