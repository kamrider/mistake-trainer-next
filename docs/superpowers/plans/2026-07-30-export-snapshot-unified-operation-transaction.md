# Export Snapshot Unified Operation Transaction Implementation Plan

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:executing-plans to implement this plan task-by-task. Steps use checkbox (`- [ ]`) syntax for tracking.

**Goal:** 让导出快照的创建、文件生成、删除和恢复共享同一个单航班事务边界，避免初始读取或另一项操作用旧状态覆盖已完成结果，并杜绝持久化成功后的错误误报。

**Architecture:** 扩展现有 `useExportSnapshotMutations`，让它统一拥有四类操作的 busy 状态、命令结果提交和通知副作用隔离；报告页只负责收集表单输入与渲染。页面的初始 `load()` 通过 `blocked` 端口参与互斥，历史组件只接收一个统一的 `operationBusy` 状态。

**Tech Stack:** Vue 3 Composition API、TypeScript、Vitest、Testing Library、生成的 Tauri `AppResult` 绑定。

## Global Constraints

- 不处理许可证、隐私、客服、账户删除、设备迁移、更新失败恢复或支持 SLA 等上线前事项。
- 不新增依赖，不修改生成绑定、Rust/OCR 文件，不暂存、不提交。
- 使用 TDD；持久化命令返回成功后，通知或焦点类副作用失败不得改写为“操作失败”。
- 所有列表替换必须保留当前真实快照，迟到的读取不得覆盖操作期间创建或恢复的结果。

---

### Task 1: 统一操作控制器失败测试

**Files:**
- Modify: `src/modules/export/composables/useExportSnapshotMutations.test.ts`
- Modify: `src/modules/export/composables/useExportSnapshotMutations.ts`

**Interfaces:**
- Consumes: `AppResult<T>`、`ExportCreateInput`、`ExportSnapshotSummary`、`GeneratedExportSummary`、`DeletedExportSnapshotSummary`。
- Produces: 扩展后的 `useExportSnapshotMutations(options)` 测试契约。

- [x] **Step 1: 扩展测试 harness**

加入下列端口并传给控制器：

```ts
const createOperation = vi.fn(async (_input: ExportCreateInput) => success(createdSnapshot))
const generateOperation = vi.fn(async (_snapshotId: string) => success(generatedExport))
const onStatus = vi.fn()
const blocked = vi.fn(() => false)
```

- [x] **Step 2: 写全局单航班失败测试**

使用 deferred 删除确认或 deferred 生成命令启动一个操作，再调用其余三类操作；断言 `createOperation`、`generateOperation`、`deleteOperation`、`restoreOperation` 中只有第一个被调用，并断言 `operationBusy.value === true`，完成 deferred 后恢复为 false。

- [x] **Step 3: 写创建与生成语义失败测试**

覆盖：创建成功把新快照置顶、去除同 id 旧项、写成功状态并调度同步；应用错误/异常保留列表；生成成功只暴露安全文件名；用户取消目录选择的 `success(null)` 不显示伪成功；生成应用错误/异常使用稳定文案；过期快照 id 不调用生成命令。

- [x] **Step 4: 写持久化后副作用隔离失败测试**

让 `scheduleSync` 抛错，分别完成创建、删除、恢复；断言真实列表已经提交，不出现“没有保存/删除/恢复”的错误文案。删除后的回收区刷新仍需执行，恢复后的列表仍需更新。

- [x] **Step 5: 运行测试确认失败**

Run: `pnpm exec vitest run src/modules/export/composables/useExportSnapshotMutations.test.ts`

Expected: FAIL，因为控制器尚无创建/生成端口、统一 `operationBusy`，并且同步调度异常仍进入命令失败分支。

---

### Task 2: 实现统一导出事务

**Files:**
- Modify: `src/modules/export/composables/useExportSnapshotMutations.ts`
- Test: `src/modules/export/composables/useExportSnapshotMutations.test.ts`

**Interfaces:**
- Consumes:

```ts
createOperation: (input: ExportCreateInput) => Promise<AppResult<ExportSnapshotSummary>>
generateOperation: (snapshotId: string) => Promise<AppResult<GeneratedExportSummary | null>>
blocked: () => boolean
onStatus: (message: string) => void
```

- Produces:

```ts
saving: Readonly<Ref<boolean>>
generatingId: Readonly<Ref<string>>
deletingId: Readonly<Ref<string>>
restoringId: Readonly<Ref<string>>
operationBusy: ComputedRef<boolean>
createSnapshot(input: ExportCreateInput): Promise<boolean>
generateSnapshot(snapshot: ExportSnapshotSummary): Promise<boolean>
deleteSnapshot(snapshotId: string): Promise<boolean | void>
restoreSnapshot(deleted: DeletedExportSnapshotSummary): Promise<boolean | void>
```

- [x] **Step 1: 建立统一 busy 状态**

`operationBusy` 必须由 `saving || generatingId || deletingId || restoringId` 计算；每个公开操作开头都检查 `options.blocked() || operationBusy.value`。

- [x] **Step 2: 实现创建事务**

创建前清空错误和状态，捕获标题、题目顺序和布局输入；成功后用 `[result.data, ...snapshots.filter(id 不同)]` 原子替换列表，显示 `已保存“标题”，随时可以从下方重新生成。`，通过独立 try/catch 调度同步。应用错误和异常不得修改列表。

- [x] **Step 3: 实现生成事务**

仅允许当前列表中仍存在的快照；成功且结果非 null 时显示 `已生成 文件名，共 N 题。`；结果为 null 时安静结束；应用错误显示服务端文案，异常显示 `文件没有生成，请检查目标目录空间与权限后重试。`。

- [x] **Step 4: 收紧删除和恢复成功边界**

命令调用的 try/catch 必须在成功提交列表之前结束。列表更新后，`scheduleSync` 使用不会向外抛出的通知函数；删除后的回收区刷新失败只显示 `快照已删除，但回收区暂时没有刷新成功。`，不得显示删除失败。

- [x] **Step 5: 运行聚焦测试与定向覆盖率**

Run: `pnpm exec vitest run src/modules/export/composables/useExportSnapshotMutations.test.ts`

Run: `pnpm exec vitest run src/modules/export/composables/useExportSnapshotMutations.test.ts --coverage --coverage.include=src/modules/export/composables/useExportSnapshotMutations.ts`

Expected: PASS；新控制器的 statements、branches、functions、lines 均为 100%。

---

### Task 3: 报告页和历史组件接入

**Files:**
- Modify: `src/app/views/ReportView.vue`
- Modify: `src/app/views/ReportView.test.ts`
- Modify: `src/modules/export/components/ExportSnapshotHistory.vue`
- Modify: `src/modules/export/components/ExportSnapshotHistory.test.ts`
- Modify: `src/modules/export/components/ExportCandidatePicker.vue`
- Modify: `src/modules/export/components/ExportCandidatePicker.test.ts`

**Interfaces:**
- Consumes: Task 2 返回的 `saving`、`generatingId`、`operationBusy`、四个操作方法。
- Produces: `ExportSnapshotHistory.operationBusy: boolean` 和 `ExportCandidatePicker.disabled: boolean` 展示契约。

- [x] **Step 1: 写初始读取覆盖创建的失败回归**

在 `ReportView.test.ts` 让 `exportList` deferred、候选题立即成功；断言保存按钮在列表请求完成前禁用且 `exportCreate` 未调用，列表完成后按钮才启用。

- [x] **Step 2: 写生成与删除互斥的失败回归**

让 `exportGenerate` deferred；点击生成后断言当前及其他快照的生成、删除、恢复按钮全部禁用，尝试点击不会调用删除/恢复；生成结束后恢复可用。

- [x] **Step 3: 接入控制器**

删除 `ReportView.vue` 内联 `saving`、`generatingId`、`createSnapshot` 命令状态机和 `generateSnapshot` 命令状态机。页面把规范化命令作为端口传入控制器，表单包装函数传递 `title.trim()`、`orderedSelectedProblemIds` 的副本和 `layout`。

- [x] **Step 4: 锁定跨操作界面**

页面刷新按钮在 `loading || operationBusy` 时禁用，`load()` 在操作中直接返回；保存按钮在 `loading || operationBusy || candidateLoading` 时禁用。`ExportSnapshotHistory` 的生成、删除、恢复按钮统一使用 `operationBusy`。`ExportCandidatePicker` 新增 `disabled` prop，在保存期间禁用来源、搜索、全选、清空及候选 checkbox，但不显示伪加载状态；快照名称和排版 fieldset 同步禁用。

- [x] **Step 5: 运行组件测试和类型检查**

Run: `pnpm exec vitest run src/app/views/ReportView.test.ts src/modules/export/components/ExportSnapshotHistory.test.ts src/modules/export/components/ExportCandidatePicker.test.ts`

Run: `pnpm typecheck`

Expected: PASS，且已有创建、回收区、生成和窄屏语义不回退。

---

### Task 4: 全量门禁与本地代码复核

**Files:**
- Modify if needed: 上述本批次文件
- Verify: `docs/superpowers/plans/2026-07-30-export-snapshot-unified-operation-transaction.md`

- [x] **Step 1: 运行完整质量门禁**

Run: `pnpm exec vitest run --coverage`

Run: `pnpm lint`

Run: `pnpm typecheck`

Run: `pnpm build`

Expected: 全部退出码为 0，无新增 bundle size warning。

- [x] **Step 2: 按商用标准复核**

检查四类操作是否真正共享单航班、初始读取是否能覆盖用户操作、持久化后副作用是否会误报、错误文案是否区分完全失败与部分成功、所有禁用状态是否有真实原生 `disabled`。

- [x] **Step 3: 修复发现并复验**

只修改本批次文件；运行受影响聚焦测试、`pnpm typecheck` 和 `git diff --check -- src/app/views/ReportView.vue src/app/views/ReportView.test.ts src/modules/export/components/ExportSnapshotHistory.vue src/modules/export/components/ExportSnapshotHistory.test.ts src/modules/export/components/ExportCandidatePicker.vue src/modules/export/components/ExportCandidatePicker.test.ts`，确认无重要发现后勾选计划。
