# Capture Draft Editor Refresh Resilience Implementation Plan

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:executing-plans to implement this plan task-by-task. Steps use checkbox syntax for tracking.

**Goal:** 防止采集题卡检查器中的未失焦标签和笔记被同题卡后台刷新覆盖，并确保保存始终携带最新权威科目。

**Architecture:** 新建 `useCaptureDraftTextEditor.ts`，把选中题卡的本地标签/笔记缓冲、字段级 dirty 状态、提交版本和权威结果收敛封装为独立 composable。`CaptureWorkspace.vue` 只负责渲染与发出 `updateDraft`；同 ID 的新 detail 在字段仍脏时不得覆盖输入，服务端返回与已提交值一致后才能清理 dirty 状态，切换题卡则加载新题卡。

**Tech Stack:** Vue 3 Composition API、TypeScript、Vitest、Testing Library、现有草稿保存队列与 CaptureWorkspace 组件测试。

## Global Constraints

- 不修改 Rust、Tauri commands、bindings、schema、迁移、依赖或草稿保存队列协议。
- 保持标签分隔规则 `/[，,]/`、trim/filter 行为、笔记 trim、`updateDraft(draft, subject, tags, note)` 事件签名和自动保存文案。
- 同题卡后台刷新不得覆盖未提交或提交中的本地字段；保存成功后的权威值必须解除 dirty 状态。
- 权威标签采用后端相同的稳定去重结果时必须成功收敛 dirty 状态。
- `busy` 期间的 change 必须发给父级保存队列等待，不得在组件内丢弃。
- 保存时使用当前 `selectedDraft.subject`，不得使用独立的过期 subject 缓冲。
- 不修改 `src-tauri/src/infrastructure/recognition_visual_split.rs`。
- 不处理许可证、隐私政策文本、客服、账户删除、设备迁移、更新失败恢复或 SLA。
- 不暂存、不提交，不覆盖工作区已有修改。

---

### Task 1: 建立编辑缓冲红灯测试

**Files:**
- Create: `src/modules/capture/composables/useCaptureDraftTextEditor.test.ts`
- Modify: `src/modules/capture/components/CaptureWorkspace.test.ts`

**Interfaces:**
- Composable 输入：

```ts
Readonly<Ref<CaptureDraftSummary | undefined>>
```

- Composable 输出：

```ts
interface CaptureDraftTextUpdate {
  draft: CaptureDraftSummary
  subject: string
  tags: string[]
  note: string
}

interface CaptureDraftTextEditor {
  tagsText: Ref<string>
  noteText: Ref<string>
  markTagsDirty: () => void
  markNoteDirty: () => void
  prepareSave: () => CaptureDraftTextUpdate | undefined
}
```

- [x] **Step 1: 运行现有工作台测试基线**

Run:

```powershell
pnpm exec vitest run src/modules/capture/components/CaptureWorkspace.test.ts
```

Expected: 当前全部测试通过。

- [x] **Step 2: 编写 composable 行为测试**

创建 `useCaptureDraftTextEditor.test.ts`，覆盖：

```ts
const selectedDraft = ref<CaptureDraftSummary | undefined>(draft('d1', '数学', ['旧标签'], '旧笔记'))
const editor = useCaptureDraftTextEditor(selectedDraft)

editor.tagsText.value = '本地标签，新标签'
editor.markTagsDirty()
editor.noteText.value = '仍在输入的笔记'
editor.markNoteDirty()

selectedDraft.value = draft('d1', '物理', ['旧标签'], '旧笔记')
await nextTick()
expect(editor.tagsText.value).toBe('本地标签，新标签')
expect(editor.noteText.value).toBe('仍在输入的笔记')
expect(editor.prepareSave()).toMatchObject({
  subject: '物理',
  tags: ['本地标签', '新标签'],
  note: '仍在输入的笔记',
})
```

继续断言：

```text
权威 detail 与同版本提交值一致 => dirty 清理并采用规范化显示
后端稳定去重重复标签 => dirty 清理并采用权威标签
提交后又产生更新版本 => 旧权威结果不得覆盖新输入
selected draft ID 改变 => 加载新题卡 tags/note
selected draft 变为 undefined => 清空缓冲
```

- [x] **Step 3: 编写组件回归测试**

在 `CaptureWorkspace.test.ts` 输入标签和笔记但不触发 change，然后用同一
draft ID、更新后的 batch revision 和科目“物理”重新渲染 detail。断言：

```text
两个输入仍显示本地内容
触发 change 后 updateDraft 事件携带“物理”、解析后的本地标签和本地笔记
busy=true 时 change 仍发出，由父级队列负责等待
```

- [x] **Step 4: 运行新测试确认红灯**

Run:

```powershell
pnpm exec vitest run src/modules/capture/composables/useCaptureDraftTextEditor.test.ts src/modules/capture/components/CaptureWorkspace.test.ts
```

Expected: FAIL，因为 composable 尚不存在，且组件当前会在同题卡刷新时覆盖本地输入。

---

### Task 2: 实现字段级编辑缓冲

**Files:**
- Create: `src/modules/capture/composables/useCaptureDraftTextEditor.ts`
- Test: `src/modules/capture/composables/useCaptureDraftTextEditor.test.ts`

**Interfaces:**
- Produces: `useCaptureDraftTextEditor(selectedDraft): CaptureDraftTextEditor`
- `prepareSave()` 返回当前 draft 对象、当前权威 subject、规范化 tags 和 note。

- [x] **Step 1: 实现规范化与字段状态**

实现：

```ts
function parseTags(value: string) {
  return value.split(/[，,]/).map(tag => tag.trim()).filter(Boolean)
}

function formatTags(tags: string[]) {
  return tags.join('，')
}

function canonicalTags(tags: string[]) {
  const seen = new Set<string>()
  return tags.filter(tag => {
    if (seen.has(tag)) return false
    seen.add(tag)
    return true
  })
}

interface SubmittedField<T> {
  editVersion: number
  value: T
}
```

每个字段分别保存 dirty、递增 edit version 和最后一次 submitted
`{ editVersion, value }`；不得用一个全局 dirty 标记耦合标签和笔记。

- [x] **Step 2: 实现同题卡权威同步规则**

`watch(selectedDraft, ..., { immediate: true })` 按以下规则执行：

```text
无 draft => 清空 active ID、输入、dirty 和 submitted
draft ID 改变 => 无条件加载新 draft，并重置字段状态
同 draft 且字段 clean => 接受权威值
同 draft 且字段 dirty、权威值匹配同 edit version 的 submitted => 接受权威值并清 dirty
同 draft 且字段 dirty、无匹配提交 => 保留本地值
```

字符串数组使用按位置严格相等；笔记与提交时的 trim 结果比较。

- [x] **Step 3: 实现保存快照**

`prepareSave()`：

```ts
const draft = selectedDraft.value
if (!draft) return undefined
const tags = parseTags(tagsText.value)
const note = noteText.value.trim()
// 仅为 dirty 字段记录当前 editVersion 对应的 submitted 值；
// 标签 submitted 值使用与后端一致的稳定去重结果。
return {
  draft,
  subject: draft.subject.trim(),
  tags,
  note,
}
```

用户在提交后继续编辑会增加 edit version，因此先返回的旧权威结果不能清理新版本 dirty 状态。

- [x] **Step 4: 运行 composable 测试**

Run:

```powershell
pnpm exec vitest run src/modules/capture/composables/useCaptureDraftTextEditor.test.ts
```

Expected: 全部通过。

---

### Task 3: 接入工作台并验证交互

**Files:**
- Modify: `src/modules/capture/components/CaptureWorkspace.vue`
- Test: `src/modules/capture/components/CaptureWorkspace.test.ts`

**Interfaces:**
- Consumes:

```ts
const {
  tagsText: draftTags,
  noteText: draftNote,
  markTagsDirty,
  markNoteDirty,
  prepareSave,
} = useCaptureDraftTextEditor(selectedDraft)
```

- [x] **Step 1: 替换组件内易覆盖缓冲**

删除 `draftSubject`、`draftTags`、`draftNote` refs 和
`watch(selectedDraft, ...)`。接入 composable，并在标签 input、笔记
textarea 上分别添加：

```vue
@input="markTagsDirty"
@input="markNoteDirty"
```

保留现有 `v-model`、maxlength、placeholder 和 `@change`。

- [x] **Step 2: 使用 composable 保存快照**

将 `saveSelectedDraft()` 改为：

```ts
function saveSelectedDraft() {
  const update = prepareSave()
  if (!update) return
  emit('updateDraft', update.draft, update.subject, update.tags, update.note)
}
```

组件不得因 `busy` 丢弃事件；`CaptureView` 的 `useCaptureDraftSaveQueue`
已经以 `isBlocked: () => busy.value` 保存 pending 更新，并在解除阻塞后 flush。

- [x] **Step 3: 运行工作台与 composable 测试**

Run:

```powershell
pnpm exec vitest run src/modules/capture/composables/useCaptureDraftTextEditor.test.ts src/modules/capture/components/CaptureWorkspace.test.ts
```

Expected: 全部通过；刷新回归测试证明未失焦输入不丢失。

---

### Task 4: 商用前端门禁与复审

**Files:**
- Verify: `src/modules/capture/composables/useCaptureDraftTextEditor.ts`
- Verify: `src/modules/capture/composables/useCaptureDraftTextEditor.test.ts`
- Verify: `src/modules/capture/components/CaptureWorkspace.vue`
- Verify: `src/modules/capture/components/CaptureWorkspace.test.ts`
- Verify: `src/app/views/CaptureView.vue`
- Verify: `src-tauri/src/infrastructure/recognition_visual_split.rs`

**Interfaces:**
- Preserves: Workspace props/emits、draft save queue、navigation guard、retry UI、subject card editing、LAN/recognition/crop interactions。

- [x] **Step 1: 运行受影响测试、类型和前端门禁**

Run:

```powershell
pnpm exec vitest run src/modules/capture/composables/useCaptureDraftTextEditor.test.ts src/modules/capture/components/CaptureWorkspace.test.ts src/app/views/CaptureView.test.ts
pnpm typecheck
pnpm lint
```

Expected: 全部通过。

- [x] **Step 2: 本地代码复审**

确认 dirty 是字段级；同 ID 刷新只在 clean 或匹配提交时收敛；旧提交不能
覆盖新编辑；切换/移除 draft 正确清空；保存使用当前 draft subject；
组件公开面和父级保存队列未改变；Rust、bindings、schema、依赖和 OCR
文件未因本批修改。

- [x] **Step 3: 最终差异检查**

Run:

```powershell
git diff --check
git diff --cached --quiet
```

Expected: 无空白错误，暂存区为空。

- [x] **Step 4: 记录验证结果**

在本文追加红/绿灯、测试数量、类型/lint 门禁、组件行数变化、本地复审、
已有工具链警告和范围排除。

## Self-Review

- 需求覆盖：同题卡刷新、成功收敛、提交后继续编辑、题卡切换、题卡移除和
  最新科目均有明确实现与测试。
- Placeholder scan：无 TBD、TODO、未定义接口或模糊错误处理。
- 类型一致性：composable 返回字段与工作台解构名称一致；
  `CaptureDraftTextUpdate` 使用现有 `CaptureDraftSummary` 和事件参数类型。

## Verification Record

- 现有 `CaptureWorkspace` 基线：25/25 通过。
- 首次红灯同时证明两点：composable 文件不存在；同题卡 detail 刷新后，
  正在输入的标签实际从“本地标签，新标签”变为空字符串。
- 新 composable 与组件首次绿灯：29/29 通过。
- 本地复审新增两条红灯：重复标签无法与后端稳定去重结果收敛；`busy=true`
  时组件丢弃 change。两条均按预期失败，修复后聚焦测试 30/30 通过。
- 最终受影响测试：3 个文件、66/66 通过；包含 composable 4 项、
  Workspace 26 项和 CaptureView 36 项。
- 完整前端回归：95 个测试文件、578/578 通过。
- `pnpm typecheck` 与全仓 `pnpm lint` 均通过。
- `useCaptureDraftTextEditor.ts` 为 148 行；`CaptureWorkspace.vue` 为
  1304 行。新增代码量用于把原先隐式、易覆盖的三个 refs/watch 变为可独立
  测试的字段级版本状态机，而不是追求表面行数下降。
- 组件已删除 `draftSubject` 和 `watch(selectedDraft)`；保存从当前
  `selectedDraft.subject` 构造，标签和笔记只在 clean 或匹配同 edit
  version 的 submitted 权威值时收敛。
- 重复标签的 submitted 匹配采用与 Rust 后端相同的稳定去重；对外发送的
  tags 数组仍保留原有解析行为，由后端继续负责最终校验和规范化。
- `busy` 期间 change 继续发出，父级既有保存队列保留 pending 更新并在
  busy 解除后 flush，因此导航守卫能看到未完成保存。
- 本地复审未发现剩余 Critical 或 Important 问题。Workspace props/emits、
  CaptureView 保存队列、Rust、bindings、schema、迁移、依赖和 OCR 文件
  未因本批改变。
- `git diff --check` 和本批文件尾随空白扫描均通过；计划无未勾选步骤，
  暂存区为空。Git 仍输出工作区已有 LF 到 CRLF 提示。
