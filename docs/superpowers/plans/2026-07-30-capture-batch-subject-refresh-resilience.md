# Capture Batch Subject Refresh Resilience Implementation Plan

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:executing-plans to implement this plan task-by-task. Steps use checkbox syntax for tracking.

**Goal:** 防止采集阶段和整理阶段尚未提交的批次科目选择，被同批次手机上传、导入或其他 detail 刷新恢复为旧值。

**Architecture:** 新建 `useCaptureBatchSubjectDraft.ts`，以批次 ID 和 state 作为编辑会话身份，分别管理采集阶段科目与整理阶段待确认科目的 dirty 状态。同一身份的权威对象刷新只更新 clean 字段；用户选择在权威值匹配后收敛，批次或阶段切换时才无条件重置。

**Tech Stack:** Vue 3 Composition API、TypeScript、Vitest、Testing Library、现有 CaptureWorkspace/CaptureView 测试。

## Global Constraints

- 不修改 Rust、Tauri commands、bindings、schema、迁移、依赖或刷新调度器。
- 保持 `finishCollecting(subject)` 和 `assignBatchSubject(subject)` 事件签名、trim、按钮禁用规则与现有中文文案。
- 同一批次且同一 state 的新 detail 不得覆盖 dirty 选择。
- 权威 subject 与 dirty 选择匹配时必须清理 dirty，并允许之后的服务端变化同步到界面。
- 批次 ID 或 state 改变时必须加载新权威 subject，不能把前一编辑会话带入下一阶段。
- 不修改 `src-tauri/src/infrastructure/recognition_visual_split.rs`。
- 不处理许可证、隐私政策文本、客服、账户删除、设备迁移、更新失败恢复或 SLA。
- 不暂存、不提交，不覆盖工作区已有修改。

---

### Task 1: 建立 subject draft 红灯测试

**Files:**
- Create: `src/modules/capture/composables/useCaptureBatchSubjectDraft.test.ts`
- Modify: `src/modules/capture/components/CaptureWorkspace.test.ts`

**Interfaces:**
- Source：

```ts
interface CaptureBatchSubjectSource {
  id: string
  state: CaptureBatchState
  subject: string
}
```

- Output：

```ts
interface CaptureBatchSubjectDraft {
  collectingSubject: Ref<string>
  pendingSubject: Ref<string>
  markCollectingDirty: () => void
  selectPendingSubject: (subject: string) => void
}
```

- [x] **Step 1: 运行当前工作台基线**

Run:

```powershell
pnpm exec vitest run src/modules/capture/components/CaptureWorkspace.test.ts
```

Expected: 26/26 通过。

- [x] **Step 2: 编写 composable 测试**

创建 `useCaptureBatchSubjectDraft.test.ts`，至少覆盖：

```ts
const source = ref(sourceBatch('batch-1', 'collecting', '数学'))
const draft = useCaptureBatchSubjectDraft(() => source.value)

draft.collectingSubject.value = '物理'
draft.markCollectingDirty()
source.value = sourceBatch('batch-1', 'collecting', '数学')
await nextTick()
expect(draft.collectingSubject.value).toBe('物理')
```

继续断言：

```text
同 ID/state 的 organizing 刷新保留 pendingSubject
权威 subject 匹配 pendingSubject 后清 dirty，下一次权威变化可同步
batch ID 改变 => 两个字段加载新 subject
state collecting -> organizing => 两个字段加载该阶段权威 subject
source undefined => 清空两个字段
```

- [x] **Step 3: 编写工作台回归测试**

在 `CaptureWorkspace.test.ts` 添加两条交互断言：

```text
collecting：选择“物理”后，用同 batch ID/state、增加 item/revision 的 detail
重新渲染；点击结束采集仍发出 finishCollecting('物理')

organizing：点击“化学”后，用同 batch ID/state、增加 revision 的 detail
重新渲染；“化学”仍 selected，点击应用仍发出 assignBatchSubject('化学')
```

- [x] **Step 4: 运行新测试确认红灯**

Run:

```powershell
pnpm exec vitest run src/modules/capture/composables/useCaptureBatchSubjectDraft.test.ts src/modules/capture/components/CaptureWorkspace.test.ts
```

Expected: FAIL，因为 composable 尚不存在，且组件 watcher 会恢复两个 subject。

---

### Task 2: 实现批次科目编辑会话

**Files:**
- Create: `src/modules/capture/composables/useCaptureBatchSubjectDraft.ts`
- Test: `src/modules/capture/composables/useCaptureBatchSubjectDraft.test.ts`

**Interfaces:**
- Produces:

```ts
useCaptureBatchSubjectDraft(
  source: () => CaptureBatchSubjectSource | undefined,
): CaptureBatchSubjectDraft
```

- [x] **Step 1: 实现身份与字段状态**

使用：

```ts
const collectingSubject = ref('')
const pendingSubject = ref('')
let activeBatchId = ''
let activeState: CaptureBatchState | undefined
let collectingDirty = false
let pendingDirty = false
```

`reset(source)` 同时更新身份、两个值并清理 dirty。

- [x] **Step 2: 实现同身份同步**

`watch(source, ..., { immediate: true })`：

```text
source 缺失、ID 改变或 state 改变 => reset
collecting clean => 同步权威 subject
collecting dirty 且本地等于权威 => 同步并清 dirty
collecting dirty 且不等 => 保留本地
pending clean => 同步权威 subject
pending dirty 且本地等于权威 => 同步并清 dirty
pending dirty 且不等 => 保留本地
```

- [x] **Step 3: 实现用户意图入口**

```ts
function markCollectingDirty() {
  collectingDirty = true
}

function selectPendingSubject(subject: string) {
  pendingSubject.value = subject
  pendingDirty = true
}
```

- [x] **Step 4: 运行 composable 测试**

Run:

```powershell
pnpm exec vitest run src/modules/capture/composables/useCaptureBatchSubjectDraft.test.ts
```

Expected: 全部通过。

---

### Task 3: 接入 CaptureWorkspace

**Files:**
- Modify: `src/modules/capture/components/CaptureWorkspace.vue`
- Test: `src/modules/capture/components/CaptureWorkspace.test.ts`

**Interfaces:**
- Consumes:

```ts
const activeBatchSubjectSource = computed(() => props.detail?.batch)
const {
  collectingSubject: batchSubject,
  pendingSubject: pendingBatchSubject,
  markCollectingDirty,
  selectPendingSubject,
} = useCaptureBatchSubjectDraft(() => activeBatchSubjectSource.value)
```

- [x] **Step 1: 删除无条件 subject 重置**

删除组件本地 `batchSubject`、`pendingBatchSubject` refs，以及
`watch(() => props.detail, ...)` 中对二者的赋值；保留 split index、选中
draft/material 的修复逻辑。

- [x] **Step 2: 接入用户意图事件**

采集 select 保留 `v-model="batchSubject"` 并添加：

```vue
@change="markCollectingDirty"
```

整理科目按钮改为：

```vue
@click="selectPendingSubject(subject)"
```

其余显示、禁用和提交函数保持不变。

- [x] **Step 3: 运行工作台与 composable 测试**

Run:

```powershell
pnpm exec vitest run src/modules/capture/composables/useCaptureBatchSubjectDraft.test.ts src/modules/capture/components/CaptureWorkspace.test.ts
```

Expected: 全部通过。

---

### Task 4: 商用前端门禁与复审

**Files:**
- Verify: `src/modules/capture/composables/useCaptureBatchSubjectDraft.ts`
- Verify: `src/modules/capture/composables/useCaptureBatchSubjectDraft.test.ts`
- Verify: `src/modules/capture/components/CaptureWorkspace.vue`
- Verify: `src/modules/capture/components/CaptureWorkspace.test.ts`
- Verify: `src/app/views/CaptureView.vue`
- Verify: `src-tauri/src/infrastructure/recognition_visual_split.rs`

**Interfaces:**
- Preserves: Workspace props/emits、refresh scheduler、batch lifecycle、LAN upload、import、recognition、draft editor 和导航守卫。

- [x] **Step 1: 运行聚焦测试、全量测试、类型和 lint**

Run:

```powershell
pnpm exec vitest run src/modules/capture/composables/useCaptureBatchSubjectDraft.test.ts src/modules/capture/components/CaptureWorkspace.test.ts src/app/views/CaptureView.test.ts
pnpm test
pnpm typecheck
pnpm lint
```

Expected: 全部通过。

- [x] **Step 2: 本地代码复审**

确认身份由 batch ID + state 决定；两个字段 dirty 独立；权威匹配后清理；
批次/阶段切换和 source 移除均 reset；公开事件、父级刷新调度和生命周期
未改变；Rust、bindings、schema、迁移、依赖和 OCR 文件未因本批修改。

- [x] **Step 3: 最终差异检查**

Run:

```powershell
git diff --check
git diff --cached --quiet
```

Expected: 无空白错误，暂存区为空。

- [x] **Step 4: 记录验证结果**

在本文追加红/绿灯、测试数量、全量门禁、模块行数、本地复审和范围排除。

## Self-Review

- 需求覆盖：采集选择、整理待确认选择、权威成功收敛、批次切换、阶段切换和
  source 移除均有测试。
- Placeholder scan：无 TBD、TODO、未定义接口或模糊错误处理。
- 类型一致性：source 使用 bindings 中现有 `CaptureBatchState`；
  composable 返回名称与组件解构及模板调用完全一致。

## Verification Record

- `CaptureWorkspace` 基线：26/26 通过。
- 红灯同时证明三个事实：新 composable 尚不存在；采集阶段用户选择的
  “物理”在同批次新增图片刷新后恢复为“数学”；整理阶段未确认的“化学”
  在同批次 revision 刷新后失去 selected 状态。
- 实现后 composable 与工作台聚焦测试：2 个文件、32/32 通过。
- 最终受影响测试：3 个文件、68/68 通过；包含 subject draft 4 项、
  Workspace 28 项和 CaptureView 36 项。
- 完整前端回归：96 个测试文件、584/584 通过。
- `pnpm typecheck` 与全仓 `pnpm lint` 均通过。
- `useCaptureBatchSubjectDraft.ts` 为 71 行；`CaptureWorkspace.vue` 为
  1308 行。新增状态机替代了组件内两个裸 ref 和每次 detail 刷新的无条件
  赋值。
- 编辑会话身份严格使用 batch ID + state；采集和整理 dirty 独立，权威
  subject 匹配后清理，批次/阶段切换或 source 移除时 reset。
- 本地复审未发现 Critical 或 Important 问题。Workspace props/emits、
  CaptureView refresh scheduler 与 batch lifecycle、Rust、bindings、
  schema、迁移、依赖和 OCR 文件未因本批改变。
- `git diff --check` 和本批文件尾随空白扫描均通过；计划无未勾选步骤，
  暂存区为空。Git 仍输出工作区已有 LF 到 CRLF 提示。
