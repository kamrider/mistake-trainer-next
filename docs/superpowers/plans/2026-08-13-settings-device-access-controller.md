# Settings Device Access Controller Completion Plan

**Goal:** Complete `useSettingsCloudSession` as the owner of the coupled cloud-session/local-device-lock workflow by moving device access status and mode-aware focus restoration out of `SettingsView.vue`.

**Architecture:** Sign-out-and-lock is one ordered application transaction, so it remains in the existing controller. The controller gains a normalized `loadDeviceAccess` operation and `restoreLockFocus(mode)` capability, readonly device state, and an async close action. The view only adapts commands and supplies component-focus callbacks.

### Task 1: Tests and implementation

- [x] Extend the harness with device status and focus capabilities.
- [x] Test successful/backend/transport device-status loads.
- [x] Test lock/sign-out close modes restore the correct focus and busy close is rejected.
- [x] Export explicit options interfaces and expose readonly device status/error.
- [x] Run focused controller tests.

### Task 2: Compose and verify

- [x] Remove device refs, load function, and close wrapper from SettingsView.
- [x] Preserve page-load task and dialog bindings through controller destructuring.
- [x] Add architecture ownership assertions and documentation.
- [x] Run Settings tests, typecheck, full architecture/lint/test/diff gates.
- [x] Recount SettingsView and move to CaptureView orchestration.
