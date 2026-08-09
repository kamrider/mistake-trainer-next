# Profile-Scoped Operation Guards Implementation Plan

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:subagent-driven-development (recommended) or superpowers:executing-plans to implement this plan task-by-task. Steps use checkbox (`- [ ]`) syntax for tracking.

**Goal:** 阻止题库和采集页的当前档案相关写操作在执行期间被路由离开、档案切换或窗口关闭打断。

**Architecture:** 题库页使用 `useDurableActionGuard` 保护批量状态变更和训练/考试会话创建；题目详情继续复用 `useUnsavedChangesGuard`，但任何详情写操作 busy 都必须阻断。采集页扩展现有草稿离开守卫的 busy 判定，使共享 `busy`、`recognitionBusy`、草稿保存和破坏性确认都进入同一离开决策，不再新增另一套确认交互。

**Tech Stack:** Vue 3 composables、Vue Router、TypeScript、Vitest、Testing Library。

## Global Constraints

- 不处理许可证、隐私、客服、账户删除、设备迁移、Windows 更新恢复或支持 SLA 等上线前事项。
- 不新增依赖，不修改 Rust/OCR 算法或生成绑定，不暂存、不提交。
- 题目详情写操作未完成前阻止关闭，确保失败反馈仍可见；采集批次内部关闭继续由现有 late-result ownership 机制处理。
- busy 期间阻断路由、档案切换和窗口关闭；结束后立即恢复并清除仅由阻断产生的临时提示。
- 失败结果、用户输入、选中项和现有明确确认对话框必须保留。

---

### Task 1: 题库批量与会话创建保护

**Files:**
- Modify: `src/app/views/LibraryView.vue`
- Modify: `src/app/views/LibraryView.test.ts`
- Modify: `src/modules/library/composables/useLibraryReviewLaunch.ts`

**Interfaces:**
- Consumes: `useDurableActionGuard`、`workspaceTransitionGuardKey`。

- [x] **Step 1: 写批量状态临界区失败测试**

使用 routed LibraryView 和真实 workspace registry，让 `problemChangeStatus` deferred。执行期间断言 workspace attempt false、路由仍为 library、`beforeunload` 被阻止并显示“题库操作正在进行，请等待完成后再离开。”；结算后全部恢复且临时提示清除。

- [x] **Step 2: 更新会话创建竞态预期**

把现有“late session result”测试改为：`reviewManualStart` pending 时导航被阻止，结算成功后只由创建流程进入 review；不得先离开再被 late result 覆盖。

- [x] **Step 3: 实现页面级 durable guard**

`libraryOperationBusy = computed(() => Boolean(changingBatchStatus.value || startingExperience.value))`。注册 context、`onBeforeRouteLeave` 和 beforeunload；busy 结束时只清除精确临时文案。

- [x] **Step 4: 运行题库定向回归**

Run: `pnpm exec vitest run src/app/views/LibraryView.test.ts src/app/composables/useDurableActionGuard.test.ts src/app/App.profile.test.ts`

Run: `pnpm typecheck`

Expected: 全部通过。

---

### Task 2: 题目详情任何写操作均阻断离开

**Files:**
- Modify: `src/modules/library/components/ProblemDetailDrawer.vue`
- Modify: `src/modules/library/components/ProblemDetailDrawer.test.ts`
- Modify: `src/app/views/LibraryView.test.ts`

**Interfaces:**
- Consumes: existing `saving?: boolean`。

- [x] **Step 1: 写无脏编辑 busy 失败测试**

drawer `saving=true` 且没有编辑改动时，registered navigation attempt 返回 false并显示“题目操作正在完成，请等待完成后再离开。”；`saving=false` 后返回 true。

- [x] **Step 2: 调整 busy 判定与文案**

`busy: () => Boolean(props.saving)`，不再依赖 dirty。更新详情和页面测试的稳定文案。

- [x] **Step 3: 运行详情与题库回归**

Run: `pnpm exec vitest run src/modules/library/components/ProblemDetailDrawer.test.ts src/app/views/LibraryView.test.ts`

Expected: 全部通过。

---

### Task 3: 采集所有当前档案操作进入现有离开守卫

**Files:**
- Modify: `src/app/views/CaptureView.vue`
- Modify: `src/app/views/CaptureView.test.ts`

**Interfaces:**
- Consumes: existing `useUnsavedChangesGuard` and workspace registry registration。

- [x] **Step 1: 写采集操作临界区失败测试**

使用 routed CaptureView 和真实 workspace registry，让 `captureBatchAssignSubject` deferred。执行期间 workspace attempt false、路由保持 inbox、`beforeunload` 被阻止，提示“采集操作正在完成，请等待完成后再离开。”；结算后全部恢复且提示清除。

- [x] **Step 2: 扩展 busy 判定**

把 `busy.value`、`recognitionBusy.value`、草稿 pending/running 和破坏性确认合并进现有守卫 busy。`onBusy` 按破坏性确认、草稿保存、其他采集操作顺序选择文案。

- [x] **Step 3: 清理临时提示**

watch 合并后的 busy computed；结束时仅清除草稿保存或采集操作临时阻断文案，不清除真实操作错误。

- [x] **Step 4: 运行采集与档案回归**

Run: `pnpm exec vitest run src/app/views/CaptureView.test.ts src/app/App.profile.test.ts src/app/composables/useUnsavedChangesGuard.test.ts`

Run: `pnpm typecheck`

Expected: 全部通过。

---

### Task 4: 完整门禁与复核

**Files:**
- Verify: `docs/superpowers/plans/2026-07-30-profile-scoped-operation-guards.md`

- [x] **Step 1: 运行完整门禁**

Run: `pnpm test:coverage`

Run: `pnpm lint`

Run: `pnpm typecheck`

Run: `pnpm build`

- [x] **Step 2: 本地代码复核**

检查 page/drawer 注册顺序、会话创建成功导航、批量失败、内部关闭详情/批次、识别与导入 busy、草稿 retry、破坏性确认、临时提示生命周期和无 provider 兼容；修复发现的问题。

- [x] **Step 3: 更新验证记录**

记录测试数量、覆盖率、构建模块数、`git diff --check` 与暂存区状态。

## Verification Record

- 定向回归：7 个测试文件、85 项测试全部通过。
- 全量覆盖率：94 个测试文件、573 项测试全部通过；Statements 81.39%、Branches 78.89%、Functions 77.32%、Lines 83.61%。
- 静态门禁：`pnpm lint`、`pnpm typecheck` 全部通过。
- 生产构建：`pnpm build` 通过，共转换 2034 个模块。
- 差异格式：`git diff --check` 通过；仅输出工作区既有的 LF/CRLF 转换警告。
- 本地复核：页面/抽屉注册顺序、会话成功后的内部导航放行、批量状态结算、详情写入期间关闭、采集批次 late-result ownership、草稿/识别/导入 busy、破坏性确认及临时提示清理均符合计划约束。
- 版本控制：未暂存、未提交；未修改用户已有的 Rust/OCR 与生成绑定改动。
