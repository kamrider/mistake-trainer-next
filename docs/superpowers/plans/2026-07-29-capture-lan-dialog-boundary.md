# Capture LAN Dialog Boundary Implementation Plan

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:subagent-driven-development (recommended) or superpowers:executing-plans to implement this plan task-by-task. Steps use checkbox (`- [ ]`) syntax for tracking.

**Goal:** Move the complete phone/LAN capture dialog out of the oversized capture workspace while raising its readable-type, touch-target, viewport, and keyboard-focus floors to commercial desktop quality.

**Architecture:** `CaptureWorkspace.vue` continues to decide when the dialog opens, remembers the launcher for focus return, and forwards domain events upward. A new presentation-only `CaptureLanDialog.vue` owns network selection, all preflight/session states, byte/time formatting, modal focus containment, and responsive dialog styling. Rust commands, `CaptureView.vue`, generated bindings, and LAN security behavior stay unchanged.

**Tech Stack:** Vue 3, TypeScript, Testing Library, Vitest, scoped CSS.

## Global Constraints

- Preserve all `CaptureWorkspace` public props and emitted event names.
- Preserve the `mobileCapture(selectedAddress: string | null)`, `refreshLanAddresses`, `refreshLanPreflight`, and `stopMobileCapture` payloads.
- The first toolbar click must still open the dialog and immediately begin the existing preflight/start flow.
- Keep initial focus on Close, trap Tab, close on Escape/backdrop, recover focus after conditional state replacement, and return focus to the toolbar launcher after closing.
- Use 12px minimum for dialog instructions/status and 44px minimum for every dialog button.
- Use `100dvh` with a `100vh` fallback and stack the session/actions at narrow widths.
- Preserve reduced-motion behavior for the LAN progress indicator.
- Do not edit OCR, recognition, Rust, generated bindings, migration, installer, release, or excluded pre-launch features.
- Do not stage or commit the dirty worktree.

---

### Task 1: Specify and build the standalone LAN dialog

**Files:**
- Create: `src/modules/capture/components/CaptureLanDialog.vue`
- Create: `src/modules/capture/components/CaptureLanDialog.test.ts`

**Interfaces:**

```ts
defineProps<{
  addresses: CaptureLanAddress[]
  preflight: CaptureLanPreflight | undefined
  preflightBusy: boolean
  session: CaptureLanSession | undefined
  busy: boolean
}>()

defineEmits<{
  close: []
  start: [selectedAddress: string | null]
  refreshAddresses: []
  refreshPreflight: []
  stop: []
}>()
```

- [x] **Step 1: Write failing component tests**

Create fixtures for a ready preflight, a missing firewall rule, two LAN addresses, and an active QR session. Prove:

```ts
it('selects a network and emits the exact start payload', async () => {
  const view = renderDialog({ preflight: readyPreflight, addresses })
  await userEvent.selectOptions(screen.getByRole('combobox', { name: '网络接口' }), '10.0.0.4')
  await userEvent.click(screen.getByRole('button', { name: '生成二维码' }))
  expect(view.emitted().start).toEqual([['10.0.0.4']])
})

it('keeps focus inside and emits close on Escape', async () => {
  const view = renderDialog({ preflight: readyPreflight, addresses })
  await waitFor(() => expect(screen.getByRole('button', { name: '关闭手机采集' })).toHaveFocus())
  await userEvent.keyboard('{Shift>}{Tab}{/Shift}')
  expect(screen.getByRole('dialog')).toContainElement(document.activeElement as HTMLElement)
  await userEvent.keyboard('{Escape}')
  expect(view.emitted().close).toHaveLength(1)
})
```

Also prove the authorization retry state does not expose terminal commands, session copy shows QR/received bytes and emits `stop`, empty addresses emit `refreshAddresses`, and a preflight state replacement recovers focus to Close when the former focused action disappears.

- [x] **Step 2: Run the component test and verify it fails**

```powershell
node node_modules\vitest\vitest.mjs run src\modules\capture\components\CaptureLanDialog.test.ts
```

Expected: FAIL because `CaptureLanDialog.vue` does not exist.

- [x] **Step 3: Implement the presentation component**

Move the LAN markup, all `.lan-*` CSS, and these responsibilities from `CaptureWorkspace.vue`:

```ts
const selectedAddress = ref('')
const minutesRemaining = computed(() => props.session
  ? Math.max(0, Math.ceil(((props.session.expiresAtUtcMs ?? Date.now()) - Date.now()) / 60_000))
  : 0)
const needsRepair = computed(() => props.preflight?.needsFirewallRepair === true)
const ready = computed(() => props.preflight?.canStart === true)
```

Watch `addresses` to keep a valid selected value. Watch session/preflight identity and, after `nextTick`, focus Close only when the active element is no longer inside the dialog. On mount focus Close. The dialog handles Escape, Tab wrapping, and `mousedown.self` close.

Use:

```css
.lan-dialog {
  max-height: min(780px, calc(100vh - 48px));
  max-height: min(780px, calc(100dvh - 48px));
}

.lan-close,
.lan-refresh,
.lan-primary,
.lan-secondary,
.lan-start,
.lan-stop {
  min-width: 44px;
  min-height: 44px;
}
```

Every `.lan-*` instruction/status selector must be at least `font-size: 12px`.

- [x] **Step 4: Run the standalone component test**

```powershell
node node_modules\vitest\vitest.mjs run src\modules\capture\components\CaptureLanDialog.test.ts
```

Expected: all LAN dialog tests PASS.

### Task 2: Replace the inline workspace dialog

**Files:**
- Modify: `src/modules/capture/components/CaptureWorkspace.vue`
- Modify: `src/modules/capture/components/CaptureWorkspace.test.ts`

**Interfaces:**
- Consumes: `CaptureLanDialog` from Task 1.
- Preserves all existing `CaptureWorkspace` props and emits.

- [x] **Step 1: Add the parent integration assertion before rewiring**

Extend the existing phone-dialog test to prove backdrop/component closure returns focus to the original toolbar launcher and the initial toolbar click still emits:

```ts
expect(view.emitted('mobileCapture')).toEqual([['192.168.1.2']])
```

Keep the existing authorization, focus containment, and state-replacement tests as regression evidence.

- [x] **Step 2: Rewire the workspace**

Import `CaptureLanDialog`. Remove the inline LAN template, `.lan-*` CSS, internal dialog refs/focus trap, selected-address watch, display-only LAN computed values, and byte formatter.

Keep only:

```ts
const showLanPanel = ref(false)
const lanLauncher = ref<HTMLButtonElement>()
let lanFocusReturn: HTMLElement | null = null

function startMobileCapture(selectedAddress: string | null = null) {
  emit('mobileCapture', selectedAddress || props.lanAddresses[0]?.address || null)
}
```

Render:

```vue
<CaptureLanDialog
  v-if="showLanPanel"
  :addresses="lanAddresses"
  :preflight="lanPreflight"
  :preflight-busy="lanPreflightBusy"
  :session="lanSession"
  :busy="busy"
  @close="closeLanPanel"
  @start="startMobileCapture"
  @refresh-addresses="emit('refreshLanAddresses')"
  @refresh-preflight="emit('refreshLanPreflight')"
  @stop="emit('stopMobileCapture')"
/>
```

`openLanPanel` still calls `showLanDialog()` and then `startMobileCapture()` when there is no active session. `closeLanPanel` remains the single owner of launcher focus restoration.

- [x] **Step 3: Run focused capture tests**

```powershell
node node_modules\vitest\vitest.mjs run src\modules\capture\components\CaptureLanDialog.test.ts src\modules\capture\components\CaptureWorkspace.test.ts src\app\views\CaptureView.test.ts
```

Expected: all focused capture tests PASS.

### Task 3: Run gates and self-review

**Files:**
- Modify: `docs/superpowers/plans/2026-07-29-capture-lan-dialog-boundary.md`

- [x] **Step 1: Run frontend quality gates**

```powershell
pnpm lint
pnpm typecheck
pnpm test:coverage
pnpm build
```

Expected: every command exits 0 and the production main bundle remains below 300 KB gzip.

- [x] **Step 2: Review the scoped diff**

Confirm:

- no LAN command, prop, event, or payload changed;
- toolbar open, authorization retry, ready network selection, QR session, stop, and error states remain represented;
- focus behavior covers mount, Tab, Escape, conditional state replacement, and launcher return;
- the standalone dialog has 12px minimum `.lan-*` copy, 44px controls, `100dvh`, narrow stacking, and reduced-motion handling;
- `CaptureWorkspace.vue` is materially smaller and no recognition/OCR source changed;
- no generated binding or excluded pre-launch behavior was added.

- [x] **Step 3: Mark the plan complete**

Only check items after the exact verification succeeds. Do not stage or commit.
