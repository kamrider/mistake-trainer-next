# Global Notice Stack Implementation Plan

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:subagent-driven-development (recommended) or superpowers:executing-plans to implement this plan task-by-task. Steps use checkbox (`- [ ]`) syntax for tracking.

**Goal:** Prevent the restore-result and Windows-compatibility notices from overlapping when both appear, including on narrow screens, while preserving their independent semantics and dismissal controls.

**Architecture:** Keep notice state and copy generation in `App.vue`, but give both existing transitions one shared fixed-position stack. The stack owns viewport positioning, width, spacing, and bounded scrolling; each notice owns only its card layout and live-region semantics. Cover runtime coexistence in the application orchestration test and cover the CSS ownership boundary in the existing application-chrome source contract.

**Tech Stack:** Vue 3 single-file components, TypeScript, CSS, Vitest, Testing Library Vue.

## Global Constraints

- Do not change backup restore, storage migration, account deletion, updater recovery, licensing, privacy, support operations, or SLA behavior.
- Preserve the existing restore and compatibility copy, roles, `aria-live` values, transitions, and 44 × 44 px dismiss targets.
- Preserve unrelated dirty-worktree changes and do not modify `src-tauri/src/infrastructure/recognition_visual_split.rs`.
- Do not add dependencies.

---

### Task 1: Lock the coexistence and responsive layout contract

**Files:**
- Modify: `src/app/App.profile.test.ts`
- Modify: `src/app/AppChromeReadability.test.ts`

**Interfaces:**
- Consumes: `commands.compatibilityStatus(): Promise<TypedResult<AppResult<WindowsCompatibilityStatus>>>` and `commands.backupRestoreStatus(): Promise<AppResult<BackupRestoreReceipt | null>>`.
- Produces: a DOM contract where both notice `<aside>` elements share one `.global-notice-stack`, and a CSS contract where only that stack owns fixed positioning and bounded viewport scrolling.

- [x] **Step 1: Add a failing coexistence behavior test**

Add `compatibilityStatus: vi.fn()` to `commandMocks`, give it a supported default in `beforeEach`, then add a test that returns an unsupported compatibility result and a successful restore receipt. Render `App`, await both notices, assert both closest `aside` elements have the same `.global-notice-stack` parent, dismiss the restore notice and verify compatibility remains, then dismiss compatibility and verify it disappears.

```ts
commandMocks.compatibilityStatus.mockResolvedValue({
  status: 'ok',
  data: {
    ok: true,
    data: {
      supportLevel: 'unsupported',
      supported: false,
      osName: 'Windows 10',
      displayVersion: '22H2',
      buildNumber: 19045,
      updateBuildRevision: 0,
      processArchitecture: 'x86_64',
      nativeArchitecture: 'x86_64',
      webview2Version: '138.0.3351.83',
      minimumWindowsBuild: 17763,
      summary: '当前系统已超出完整支持范围。',
    },
  },
})

const restoreNotice = (await screen.findByText('资料库恢复成功')).closest('aside')
const compatibilityNotice = (await screen.findByText('当前 Windows 环境不在支持范围')).closest('aside')
expect(restoreNotice?.parentElement).toBe(compatibilityNotice?.parentElement)
expect(restoreNotice?.parentElement).toHaveClass('global-notice-stack')
```

- [x] **Step 2: Add a failing source-level layout contract**

Extend `AppChromeReadability.test.ts` with a test that requires `.global-notice-stack` to contain fixed positioning, grid layout, spacing, bounded height, and automatic overflow, while `.restore-notice` no longer owns fixed positioning.

```ts
it('stacks simultaneous global notices inside one bounded viewport layer', () => {
  const stack = declarations('src/app/App.vue', '.global-notice-stack')
  expect(stack).toContain('position:fixed')
  expect(stack).toContain('display:grid')
  expect(stack).toContain('gap:10px')
  expect(stack).toContain('max-height:calc(100vh-40px)')
  expect(stack).toContain('overflow:auto')
  expect(declarations('src/app/App.vue', '.restore-notice')).not.toContain('position:fixed')
})
```

- [x] **Step 3: Run the focused tests and verify red**

Run: `npm test -- src/app/App.profile.test.ts src/app/AppChromeReadability.test.ts`

Expected: FAIL because `App.vue` does not contain `.global-notice-stack`, and the two notices are still separate fixed-position elements.

### Task 2: Move viewport layout responsibility into one notice stack

**Files:**
- Modify: `src/app/App.vue:417-455`
- Modify: `src/app/App.vue:540-545`

**Interfaces:**
- Consumes: existing `restoreNoticeCopy`, `compatibilityNoticeCopy`, dismiss state, roles, live regions, and `restore-notice` transitions.
- Produces: `.global-notice-stack`, an always-mounted presentation wrapper containing both transitions in document order.

- [x] **Step 1: Wrap both notice transitions**

Wrap the two existing `<Transition name="restore-notice">` nodes without changing either notice's contents or conditions.

```vue
<div class="global-notice-stack">
  <Transition name="restore-notice">
    <!-- existing restore-result aside -->
  </Transition>
  <Transition name="restore-notice">
    <!-- existing Windows-compatibility aside -->
  </Transition>
</div>
```

Keep the wrapper always mounted so leave transitions can complete when the final notice is dismissed.

- [x] **Step 2: Make the stack own viewport positioning**

Add the stack declaration and remove viewport positioning from `.restore-notice` and `.compatibility-notice`.

```css
.global-notice-stack {
  position: fixed;
  z-index: 60;
  top: 20px;
  right: 24px;
  display: grid;
  gap: 10px;
  width: min(440px, calc(100vw - 48px));
  max-height: calc(100vh - 40px);
  overflow: auto;
  overscroll-behavior: contain;
}
.restore-notice {
  position: relative;
  display: grid;
  grid-template-columns: auto minmax(0, 1fr) auto;
  /* retain existing card declarations */
}
@media (max-width: 760px) {
  .global-notice-stack {
    top: 12px;
    right: 12px;
    width: calc(100vw - 24px);
    max-height: calc(100vh - 24px);
  }
}
```

Delete the old `.compatibility-notice` left/right positioning rule and the old mobile notice positioning overrides.

- [x] **Step 3: Run the focused tests and verify green**

Run: `npm test -- src/app/App.profile.test.ts src/app/AppChromeReadability.test.ts`

Expected: both test files PASS; both notices share the stack, remain independently dismissible, and the CSS contract identifies one fixed viewport layer.

### Task 3: Verify the commercial-quality regression boundary

**Files:**
- Modify: `docs/superpowers/plans/2026-08-08-global-notice-stack.md`

**Interfaces:**
- Consumes: the completed application and tests from Tasks 1–2.
- Produces: recorded verification results and a completed checklist.

- [x] **Step 1: Run static validation**

Run: `npm run lint`

Expected: PASS with zero warnings.

Run: `npm run typecheck`

Expected: PASS with zero TypeScript errors.

- [x] **Step 2: Run the production build**

Run: `npm run build`

Expected: PASS through mobile asset verification, Vue type checking, and Vite production bundling.

- [x] **Step 3: Run the full frontend suite**

Run: `npm test`

Expected: all Vitest files and tests PASS.

- [x] **Step 4: Review scope and record completion**

Inspect `git diff -- src/app/App.vue src/app/App.profile.test.ts src/app/AppChromeReadability.test.ts docs/superpowers/plans/2026-08-08-global-notice-stack.md`, confirm no excluded functionality changed, then replace every plan checkbox with `[x]` and append the exact verification counts.

## Verification Record

- Red phase: focused Vitest failed in 2 expected assertions because the shared stack did not exist and notice cards still owned fixed positioning.
- Focused green phase: 2 test files, 21 tests passed.
- `npm run lint`: passed with zero warnings.
- `npm run typecheck`: passed with zero errors.
- `npm run build`: passed; Vite transformed 2054 modules.
- `npm test`: 138 test files, 772 tests passed.
- Local code review: no Critical or Important findings; `git diff --check` passed. No excluded launch-only implementation was edited in this batch.
