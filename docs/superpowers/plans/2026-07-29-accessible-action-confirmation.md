# Accessible Action Confirmation Implementation Plan

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:executing-plans to implement this plan task-by-task. Steps use checkbox (`- [ ]`) syntax for tracking.

**Goal:** Replace every browser-native confirmation in the Vue product with one reusable, accessible in-app confirmation flow for destructive actions and unsaved-edit dismissal.

**Architecture:** A small composable owns the single pending confirmation promise and guarantees cancellation on component disposal. A shared teleported dialog owns presentation, safe initial focus, keyboard trapping, Escape/backdrop cancellation, and focus restoration. Feature components provide only product-specific copy and continue owning their mutations.

**Tech Stack:** Vue 3, TypeScript, Testing Library, Vitest, repository source contracts.

## Global Constraints

- Preserve every pre-existing worktree change.
- Do not edit recognition/OCR, Rust, migrations, installer/release, licensing, privacy, support, account deletion, device migration, update recovery, or SLA behavior.
- Keep public events, Tauri command signatures, and generated bindings unchanged.
- Preserve the existing mutation behavior and Chinese meaning after confirmation.
- Default focus must be the safe cancel action; Escape and backdrop click must cancel.
- Trap Tab focus inside the confirmation while it is open and restore the previously focused control after cancellation.
- Do not stage or commit.

---

### Task 1: Build the shared confirmation primitive

**Files:**

- Create: `src/app/composables/useActionConfirmation.ts`
- Create: `src/app/composables/useActionConfirmation.test.ts`
- Create: `src/app/components/ActionConfirmDialog.vue`
- Create: `src/app/components/ActionConfirmDialog.test.ts`

**Interfaces:**

```ts
export interface ActionConfirmationRequest {
  eyebrow?: string
  title: string
  description: string
  confirmLabel: string
  cancelLabel?: string
  tone?: 'danger' | 'warning'
}

export function useActionConfirmation(): {
  current: Readonly<Ref<ActionConfirmationRequest | undefined>>
  ask: (request: ActionConfirmationRequest) => Promise<boolean>
  confirm: () => void
  cancel: () => void
}
```

- [x] **Step 1: Write failing controller tests**

Prove `ask()` stays pending until `confirm()` or `cancel()`, refuses to replace an active request, clears the current request after settlement, and resolves `false` when its owner unmounts.

- [x] **Step 2: Run the controller test and verify failure**

```powershell
npm run test -- --run src/app/composables/useActionConfirmation.test.ts
```

Expected: FAIL because the composable does not exist.

- [x] **Step 3: Implement the single-pending-request controller**

Use one stored resolver. A second `ask()` while a request is pending must resolve `false` immediately instead of replacing the visible decision. Register an unmount cleanup that settles the active request as `false`.

- [x] **Step 4: Write failing dialog interaction tests**

Assert:

- `role="alertdialog"`, `aria-modal`, labelled title, and described risk copy.
- cancel receives initial focus.
- Tab and Shift+Tab remain inside the dialog.
- Escape and backdrop clicks emit `cancel`.
- the confirm button emits `confirm` and carries the destructive visual class for `tone="danger"`.
- unmount restores focus to the launcher that was active before the dialog opened.

- [x] **Step 5: Implement and verify the shared dialog**

Render through `<Teleport to="body">`, stop keyboard propagation, use stable title/description IDs, and add responsive/reduced-motion styles. Run:

```powershell
npm run test -- --run src/app/composables/useActionConfirmation.test.ts src/app/components/ActionConfirmDialog.test.ts
```

Expected: both focused files PASS.

### Task 2: Replace destructive browser confirmations

**Files:**

- Modify: `src/modules/capture/components/CaptureWorkspace.vue`
- Modify: `src/modules/capture/components/CaptureWorkspace.test.ts`
- Modify: `src/app/views/CaptureView.vue`
- Modify: `src/app/views/CaptureView.test.ts`
- Modify: `src/app/views/ReportView.vue`
- Modify: `src/app/views/ReportView.test.ts`

- [x] **Step 1: Convert existing tests to in-app decisions**

Add or update regressions proving:

- deleting a capture batch does nothing after “保留批次” and emits the original `discardBatch` event only after the destructive action;
- removing one capture image and reverting a crop do not invoke their commands before confirmation;
- deleting an export snapshot does not invoke `exportDelete` before confirmation and still refreshes the recycle area after confirmation.

- [x] **Step 2: Verify the consumer tests fail against native confirmation**

```powershell
npm run test -- --run src/modules/capture/components/CaptureWorkspace.test.ts src/app/views/CaptureView.test.ts src/app/views/ReportView.test.ts
```

Expected: new dialog queries fail while the old `window.confirm` paths remain.

- [x] **Step 3: Wire the shared controller and dialog**

Keep feature-specific request copy next to each mutation. Await `ask()` before setting busy state or issuing commands. Use distinct labels:

- batch: `保留批次` / `删除批次`
- image: `保留图片` / `删除图片`
- crop: `保留裁剪结果` / `恢复原图`
- snapshot: `保留快照` / `移入回收区`

- [x] **Step 4: Run the focused consumer suite**

Expected: all three consumer test files PASS with their original command/event assertions intact.

### Task 3: Replace unsaved-edit confirmation and prevent regression

**Files:**

- Modify: `src/modules/library/components/ProblemDetailDrawer.vue`
- Modify: `src/modules/library/components/ProblemDetailDrawer.test.ts`
- Modify: `tests/repository-contract.test.ts`

- [x] **Step 1: Replace the native-confirm test with dialog behavior**

Enter edit mode, make the form dirty, request close, cancel with `继续编辑`, and assert the drawer remains open with the typed value intact. Request close again, choose `放弃修改`, and assert the original `close` event fires.

- [x] **Step 2: Wire asynchronous dismissal decisions**

Make `requestClose()`, `requestStatus()`, and adjacent-problem navigation await a shared `confirmDiscard()` helper. Clean drawers continue immediately; dirty drawers use the warning-toned confirmation and retain all local fields after cancellation.

- [x] **Step 3: Add a repository source contract**

Scan `src/**/*.vue` and fail if `window.confirm(`, `window.alert(`, or `window.prompt(` appears. This makes non-blocking, accessible app dialogs a maintained product invariant.

- [x] **Step 4: Run focused and complete frontend gates**

```powershell
npm run lint
npm run typecheck
npm run test:coverage
npm run build
```

- [x] **Step 5: Review the scoped diff**

Confirm safe initial focus, focus trap/restoration, cancellation on Escape/backdrop/unmount, no command before consent, unchanged mutation results, no native blocking dialogs, and no excluded subsystem edits.
