# Interaction UX Repairs Implementation Plan

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:subagent-driven-development (recommended) or superpowers:executing-plans to implement this plan task-by-task. Steps use checkbox (`- [ ]`) syntax for tracking.

**Goal:** Repair the audited interaction problems across smart image splitting, capture organization, library navigation, and settings without changing the persisted capture or problem data model.

**Architecture:** Keep the existing Vue component boundaries and Tauri command contracts. Add explicit UI state and events where a component currently relies on hidden drag, click, or preview side effects; keep every data mutation routed through the existing `CaptureView` command handlers. Treat recognition review as a modal task surface while preserving its current review/apply command flow.

**Tech Stack:** Vue 3 `<script setup>`, TypeScript, Testing Library, Vitest, existing Lucide icons and scoped CSS.

## Global Constraints

- Preserve all existing dirty-worktree changes and do not stage or commit them automatically.
- Do not change the capture database schema or Rust command signatures for these interaction repairs.
- Keep automatic draft saving and atomic ready-card commit behavior intact.
- Keep drag-and-drop and keyboard shortcuts as secondary power-user paths.
- Every mutation must also have a visible button or select control.
- Destructive and regrouping actions must disclose their exact effect before execution.
- Disabled or future capabilities must not look selectable.

---

### Task 1: Reliable, focused smart-splitting review

**Files:**
- Modify: `src/modules/capture/components/CaptureWorkspace.vue`
- Modify: `src/modules/capture/components/CaptureWorkspace.test.ts`
- Modify: `src/modules/ocr/components/CaptureRecognitionReview.vue`
- Modify: `src/modules/ocr/components/CaptureRecognitionReview.test.ts`

**Interfaces:**
- Consumes: existing `preview(itemId: string)`, `recognitionReview`, `recognitionApply`, and `recognitionResume` events.
- Produces: `CaptureRecognitionReview` event `preview(itemId: string)` and a modal review surface with explicit low-confidence actions.

- [ ] **Step 1: Write failing tests for preview requests and low-confidence semantics**

```ts
it('requests the current source preview when review opens and navigation changes', async () => {
  const view = render(CaptureRecognitionReview, { props: { job: job(), previews: {} } })
  expect(view.emitted('preview')).toEqual([['item-review']])
})

it('replaces unavailable low-confidence controls with safe next actions', async () => {
  await user.click(screen.getByRole('button', { name: /无法安全切分 1/ }))
  expect(screen.queryByRole('button', { name: '接受建议' })).not.toBeInTheDocument()
  expect(screen.getByRole('button', { name: '保留原图' })).toBeVisible()
  expect(screen.getByRole('button', { name: '手工裁剪' })).toBeVisible()
})
```

- [ ] **Step 2: Run the focused tests and verify the new expectations fail**

Run: `pnpm test -- src/modules/ocr/components/CaptureRecognitionReview.test.ts src/modules/capture/components/CaptureWorkspace.test.ts`

Expected: failures for the missing `preview` event, old “未给出建议” label, and inline review surface.

- [ ] **Step 3: Implement preview loading and modal review**

Add `preview: [itemId: string]` to `CaptureRecognitionReview` emits. Watch `current.value?.itemId` with `{ immediate: true }` and emit `preview` whenever the corresponding `previews` entry is absent. Forward the event through `CaptureWorkspace` to its existing `preview` event.

Wrap the review in a fixed overlay with `role="dialog"`, `aria-modal="true"`, one internal scroll container, sticky header/footer, Escape close, and focus return to `CaptureRecognitionEntry`.

- [ ] **Step 4: Simplify review actions by state**

Use these exact low-confidence labels and actions:

```ts
low: '无法安全切分'
```

For low confidence, hide Accept and show:

```vue
<button @click="review(current, 'rejected')">保留原图</button>
<button @click="emit('edit', current)">手工裁剪</button>
```

For stale suggestions, show only “忽略已过期建议”. Render the bulk high-confidence button only when `counts.high > 0`. When no accepted suggestions exist, replace the disabled apply button with explanatory text instead of a dead control.

- [ ] **Step 5: Run the focused tests**

Run: `pnpm test -- src/modules/ocr/components/CaptureRecognitionReview.test.ts src/modules/capture/components/CaptureWorkspace.test.ts`

Expected: all focused tests pass.

---

### Task 2: Explicit material and card actions

**Files:**
- Modify: `src/modules/capture/components/CaptureThumbnail.vue`
- Modify: `src/modules/capture/components/CaptureThumbnail.test.ts`
- Modify: `src/modules/capture/components/CaptureDraftCard.vue`
- Modify: `src/modules/capture/components/CaptureDraftCard.test.ts`
- Modify: `src/modules/capture/components/CaptureWorkspace.vue`
- Modify: `src/modules/capture/components/CaptureWorkspace.test.ts`

**Interfaces:**
- Consumes: existing `stageItemRole`, `moveItem`, `mergeCard`, `crop`, and `revertCrop` events.
- Produces: visible material selection state and explicit buttons that call the same existing events.

- [ ] **Step 1: Write failing tests for explicit role and movement controls**

```ts
it('selects a material without changing its role, then changes role explicitly', async () => {
  await user.click(within(loose).getByLabelText('待配对超长文件名图片.png'))
  expect(view.emitted('stageItemRole')).toBeUndefined()
  await user.click(screen.getByRole('button', { name: '设为答案' }))
  expect(view.emitted('stageItemRole')).toEqual([['loose', 'answer']])
})

it('offers explicit new-card and return-to-library actions', async () => {
  expect(screen.getByRole('button', { name: '用所选素材新建题卡' })).toBeVisible()
  expect(within(secondCard).getByRole('button', { name: '将当前图片移回素材库' })).toBeVisible()
})
```

- [ ] **Step 2: Run capture component tests and verify failure**

Run: `pnpm test -- src/modules/capture/components/CaptureThumbnail.test.ts src/modules/capture/components/CaptureDraftCard.test.ts src/modules/capture/components/CaptureWorkspace.test.ts`

Expected: failures because click currently toggles role and explicit actions do not exist.

- [ ] **Step 3: Separate selection from mutation**

Track `selectedMaterialId` in `CaptureWorkspace`. Thumbnail activation selects the material. Render a compact selected-material toolbar containing “设为题面”, “设为答案”, “新建题卡”, and one “加入第 N 题” action per visible draft. Keep pointer drag and keyboard shortcuts working.

- [ ] **Step 4: Repair card action semantics**

When `answerItems.length === 0`, replace the flip button with “添加答案”; it selects/focuses the material library and does not flip to an empty face. Add “将当前图片移回素材库” as a visible button. Rename “恢复原图” to “撤销裁剪”.

- [ ] **Step 5: Collapse non-actionable empty zones**

When there are no unassigned materials, render a compact “素材已全部配对” summary instead of the full sticky strip. Hide the new-card drop target and selected-material actions when no material is available.

- [ ] **Step 6: Run capture component tests**

Run: `pnpm test -- src/modules/capture/components/CaptureThumbnail.test.ts src/modules/capture/components/CaptureDraftCard.test.ts src/modules/capture/components/CaptureWorkspace.test.ts`

Expected: all focused tests pass.

---

### Task 3: Safer batch-wide actions and clearer workbench hierarchy

**Files:**
- Modify: `src/modules/capture/components/CaptureWorkspace.vue`
- Modify: `src/modules/capture/components/CaptureWorkspace.test.ts`
- Modify: `src/app/views/CaptureView.vue`
- Modify: `src/app/views/CaptureView.test.ts`

**Interfaces:**
- Consumes: existing `assignBatchSubject`, `applyLayout`, `commitReady`, `discardBatch`, and import events.
- Produces: confirmation state local to `CaptureWorkspace`; no new Tauri commands.

- [ ] **Step 1: Write failing tests for confirmation and disabled states**

```ts
it('does not apply a batch subject until the user confirms', async () => {
  await user.click(within(subjectBar).getByRole('button', { name: '化学' }))
  expect(view.emitted('assignBatchSubject')).toBeUndefined()
  await user.click(screen.getByRole('button', { name: '应用到整批' }))
  expect(view.emitted('assignBatchSubject')).toEqual([['化学']])
})

it('labels regrouping as a whole-batch action and disables it without materials', () => {
  expect(screen.getByRole('button', { name: '重新分组全部图片' })).toBeVisible()
})
```

- [ ] **Step 2: Run workspace/view tests and verify failure**

Run: `pnpm test -- src/modules/capture/components/CaptureWorkspace.test.ts src/app/views/CaptureView.test.ts`

Expected: failures for immediate subject mutation and old template action copy.

- [ ] **Step 3: Add staged batch-subject selection**

Store a pending subject locally. Clicking a subject only selects it; “应用到整批” emits the existing command. Show “将覆盖当前题卡科目；单题仍可随后修改” and a success status after the parent returns `saveState="saved"`.

- [ ] **Step 4: Make regrouping explicit and contextual**

Rename the action to “重新分组全部图片” when drafts exist and “按模板生成题卡” when no drafts exist. Disable it if there are no items or no meaningful layout change. Replace the native confirmation copy with an in-app confirmation panel listing the number of cards, notes, and images affected.

- [ ] **Step 5: Consolidate intake controls and future capability copy**

Use one primary “添加素材” control with “电脑选择图片” and phone capture as explicit sub-actions, retain the drop zone, and keep `Ctrl+V` as helper copy. Collapse “全自动识题·未开放” to a one-line non-interactive note linked conceptually to Settings; do not render it as an operational card.

- [ ] **Step 6: Clarify commit blockers**

Show the first few concrete blockers such as “第 2 题：缺答案” and label the main action “保存全部就绪题（N）”. Keep the button disabled at zero and preserve the existing atomic `commitReady` event.

- [ ] **Step 7: Run workspace/view tests**

Run: `pnpm test -- src/modules/capture/components/CaptureWorkspace.test.ts src/app/views/CaptureView.test.ts`

Expected: all focused tests pass.

---

### Task 4: Remove duplicate and misleading global actions

**Files:**
- Modify: `src/modules/dashboard/components/TrainingDashboard.vue`
- Modify: `src/modules/dashboard/components/TrainingDashboard.test.ts`
- Modify: `src/modules/library/components/LibraryWorkspace.vue`
- Modify: `src/modules/library/components/LibraryWorkspace.test.ts`
- Modify: `src/modules/library/components/ProblemDetailDrawer.vue`
- Modify: `src/modules/library/components/ProblemDetailDrawer.test.ts`
- Modify: `src/modules/capture/components/CaptureWorkspace.vue`
- Modify: `src/modules/capture/components/CaptureWorkspace.test.ts`

**Interfaces:**
- Preserve existing emitted navigation, selection, archive, recycle, and training events.

- [ ] **Step 1: Write failing tests for new labels and action hierarchy**

```ts
expect(screen.getByRole('button', { name: '添加素材' })).toBeVisible()
expect(screen.getByRole('button', { name: '批量管理' })).toBeVisible()
expect(screen.getByRole('button', { name: '更多题目操作' })).toBeVisible()
```

- [ ] **Step 2: Run dashboard/library tests and verify failure**

Run: `pnpm test -- src/modules/dashboard/components/TrainingDashboard.test.ts src/modules/library/components/LibraryWorkspace.test.ts src/modules/library/components/ProblemDetailDrawer.test.ts`

Expected: failures for old “录入新错题”, “选择当前结果”, and exposed secondary actions.

- [ ] **Step 3: Simplify dashboard routing**

Keep one dynamic hero primary action. Remove the duplicate hero “整理采集箱” button when the same destination already appears in “接下来”; retain the route row with live counts.

- [ ] **Step 4: Clarify library actions and card scanning**

Rename “录入新错题” to “添加素材” and “选择当前结果” to “批量管理”. Enter selection mode before showing checkboxes and batch actions. Normalize preview containers to a stable aspect-ratio with `object-fit: contain`.

- [ ] **Step 5: Improve detail drawer hierarchy**

Move “编辑题目” into the drawer header. Keep training as the sole primary footer action. Put archive and recycle actions under “更多题目操作”, and add previous/next navigation when neighboring IDs are available without changing existing single-item behavior.

- [ ] **Step 6: Improve batch identity and deletion**

Display an automatic fallback name using subject plus formatted update time. Move batch deletion behind a “更多批次操作” menu; retain the existing confirmation and discard event.

- [ ] **Step 7: Run dashboard, library, and workspace tests**

Run: `pnpm test -- src/modules/dashboard/components/TrainingDashboard.test.ts src/modules/library/components/LibraryWorkspace.test.ts src/modules/library/components/ProblemDetailDrawer.test.ts src/modules/capture/components/CaptureWorkspace.test.ts`

Expected: all focused tests pass.

---

### Task 5: Honest settings capability states and discoverable navigation

**Files:**
- Modify: `src/app/views/SettingsView.vue`
- Modify: `src/app/views/SettingsView.test.ts`
- Modify: `src/app/components/SettingsSectionNav.vue`
- Modify: `src/app/components/SettingsSectionNav.test.ts`

**Interfaces:**
- Preserve the existing `chooseBackend('local-only' | 'supabase')` flow.
- Do not call `chooseBackend('tencent')` while the adapter is unavailable.

- [ ] **Step 1: Write failing tests for unavailable backend and navigation affordances**

```ts
expect(screen.getByRole('button', { name: /腾讯云/ })).toBeDisabled()
expect(screen.getByText('规划中')).toBeVisible()
expect(screen.getByRole('button', { name: '查看更多设置' })).toBeVisible()
```

- [ ] **Step 2: Run settings tests and verify failure**

Run: `pnpm test -- src/app/views/SettingsView.test.ts src/app/components/SettingsSectionNav.test.ts`

Expected: failures because Tencent is clickable and the hidden horizontal overflow has no visible control.

- [ ] **Step 3: Render honest backend capability states**

Extend the local `backendOptions` presentation model with `available` and `badge`. Disable unavailable entries, remove radio-like selected affordance from them, and display “规划中”. Replace visible `outbox` jargon with “待同步变更”.

- [ ] **Step 4: Add visible settings navigation controls**

Track whether the horizontal scroller has hidden content on the left or right. Add compact previous/next buttons with accessible labels “查看前面的设置” and “查看更多设置”, plus edge fades. Update them on mount, scroll, resize, and section changes.

- [ ] **Step 5: Run settings tests**

Run: `pnpm test -- src/app/views/SettingsView.test.ts src/app/components/SettingsSectionNav.test.ts`

Expected: all focused tests pass.

---

### Task 6: Regression verification and release build

**Files:**
- Verify only; fix failures in the files changed by Tasks 1–5.

**Interfaces:**
- No new interfaces.

- [ ] **Step 1: Run all Vue tests**

Run: `pnpm test`

Expected: all tests pass.

- [ ] **Step 2: Run static checks**

Run: `pnpm lint`

Expected: exit code 0 with no warnings.

Run: `pnpm typecheck`

Expected: exit code 0.

- [ ] **Step 3: Build the frontend**

Run: `pnpm build`

Expected: mobile vendor check, `vue-tsc`, and Vite production build all pass.

- [ ] **Step 4: Inspect the resulting Windows release interactively**

Launch the existing release or a newly built Tauri executable and verify:

- Smart review loads its current source image and behaves as a modal.
- Low-confidence suggestions have only “保留原图” and “手工裁剪”.
- Material click selects without silently changing role.
- Missing-answer cards show “添加答案”.
- Empty material zones collapse.
- Batch subject and regrouping require explicit confirmation.
- Library and settings labels match their actual behavior.

- [ ] **Step 5: Review the final diff**

Run: `git diff --check`

Expected: no whitespace errors.

Run: `git status --short`

Expected: only the pre-existing worktree changes plus the interaction repair files and this plan; no generated cache or temporary files.
