# Workspace Transition Guard Implementation Plan

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:subagent-driven-development (recommended) or superpowers:executing-plans to implement this plan task-by-task. Steps use checkbox (`- [ ]`) syntax for tracking.

**Goal:** 防止切换、新建或删除当前学习档案在不触发路由守卫的情况下绕过页面未保存保护并替换数据上下文。

**Architecture:** 新建应用级 `createWorkspaceTransitionGuard` 注册表，活动页面把已有 `attemptLeave()` 同时注册为路由决策和工作区切换决策。`App.vue` 在任何会改变 active profile 的命令前调用注册表；取消或 busy 结果直接终止命令。注册表对并发请求单飞并按注册快照顺序求值，页面卸载时由 `useUnsavedChangesGuard` 统一注销。

**Tech Stack:** Vue 3 provide/inject、Vue Router、TypeScript、Vitest、Testing Library。

## Global Constraints

- 不处理许可证、隐私、客服、账户删除、设备迁移、Windows 更新恢复或支持 SLA 等上线前事项。
- 不新增依赖，不修改 Rust/OCR 算法或生成绑定，不暂存、不提交。
- 不新增另一套确认文案；设置、题库详情和采集页继续使用各自已有的 `ActionConfirmDialog`。
- 档案命令必须在页面许可之后执行；取消、保存中或未决破坏性确认均不得调用命令。
- 重复工作区切换请求共享同一个决策 Promise；许可后仍依赖现有 profile mutation 单飞，最多执行一个命令。
- 重命名档案和删除非当前档案不改变当前数据上下文，不触发工作区离开确认。
- 新建档案、切换档案和删除当前档案会改变 active profile，必须受门禁保护。

---

### Task 1: 应用级门禁注册表

**Files:**
- Create: `src/app/workspace-transition-guard.ts`
- Create: `src/app/workspace-transition-guard.test.ts`

**Interfaces:**
- Produces: `WorkspaceTransitionAttempt = () => boolean | Promise<boolean>`。
- Produces: `WorkspaceTransitionGuard { register(attempt): () => void; attempt(): Promise<boolean> }`。
- Produces: `workspaceTransitionGuardKey: InjectionKey<WorkspaceTransitionGuard>`。

- [x] **Step 1: 写空注册与顺序求值失败测试**

无注册时 attempt 返回 true。注册两个决策时按注册顺序执行；第一个 false 后第二个不执行。注销后不再参与。

- [x] **Step 2: 写并发单飞失败测试**

第一个决策 deferred 时连续调用两次 attempt，只执行一次注册决策；结算 true/false 后两个调用获得相同结果，下一次调用可开启新决策。

- [x] **Step 3: 实现最小注册表**

使用 `Set` 保存 attempt；每次求值复制快照，逐个 await。缓存当前 aggregate Promise，并在 finally 清除。

- [x] **Step 4: 运行定向覆盖率**

Run: `pnpm exec vitest run src/app/workspace-transition-guard.test.ts --coverage --coverage.include=src/app/workspace-transition-guard.ts`

Expected: PASS；statements、branches、functions、lines 均为 100%。

---

### Task 2: 通用未保存守卫注册工作区决策

**Files:**
- Modify: `src/app/composables/useUnsavedChangesGuard.ts`
- Modify: `src/app/composables/useUnsavedChangesGuard.test.ts`

**Interfaces:**
- Adds option: `registerContextTransition?: (attempt: NavigationAttempt) => () => void`。

- [x] **Step 1: 写双注册生命周期失败测试**

setup 时路由 registrar 和 context registrar 各收到同一个 attempt；dirty 确认结果可由任一入口触发；卸载时两个 unregister 均只调用一次。

- [x] **Step 2: 实现可选 context registrar**

setup 期间调用两个可选 registrar；`onBeforeUnmount` 移除 beforeunload 并注销两者。确认单飞仍由同一个 `pendingDecision` 管理。

- [x] **Step 3: 运行守卫覆盖率**

Run: `pnpm exec vitest run src/app/composables/useUnsavedChangesGuard.test.ts --coverage --coverage.include=src/app/composables/useUnsavedChangesGuard.ts`

Expected: PASS；四项覆盖率均为 100%。

---

### Task 3: 活动编辑页面接入门禁

**Files:**
- Modify: `src/app/views/SettingsView.vue`
- Modify: `src/app/views/CaptureView.vue`
- Modify: `src/app/views/LibraryView.vue`
- Modify: `src/modules/library/components/ProblemDetailDrawer.vue`
- Modify: `src/modules/library/components/ProblemDetailDrawer.test.ts`

**Interfaces:**
- Consumes: injected `workspaceTransitionGuardKey`。
- Adds drawer prop: `registerContextTransition?: (attempt: NavigationAttempt) => () => void`。

- [x] **Step 1: 设置与采集页注册**

两个路由页 inject 可选 registry，并把 `registry.register` 传给各自 `useUnsavedChangesGuard` 的 `registerContextTransition`。没有 App provider 的独立测试保持可运行。

- [x] **Step 2: 题库详情注册**

`LibraryView` inject registry 并把 registrar 传给 `ProblemDetailDrawer`；抽屉转发给通用守卫。单元测试验证 context registrar 收到 attempt 且卸载注销。

- [x] **Step 3: 运行页面定向回归**

Run: `pnpm exec vitest run src/app/composables/useUnsavedChangesGuard.test.ts src/modules/library/components/ProblemDetailDrawer.test.ts src/app/views/SettingsView.test.ts src/app/views/CaptureView.test.ts`

Run: `pnpm typecheck`

Expected: PASS。

---

### Task 4: 档案命令前置门禁

**Files:**
- Modify: `src/app/App.vue`
- Modify: `src/app/App.profile.test.ts`

- [x] **Step 1: 写取消档案切换失败测试**

给 App 注入一个活动页面决策：第一次 false。点击切换档案后断言 `profileSelect` 未调用、active profile 和路由不变；第二次 true 后才调用一次命令并刷新工作区。

- [x] **Step 2: 写并发与 mutation 范围测试**

门禁 deferred 时重复触发切换，注册决策只执行一次，许可后 profile 命令只执行一次。新建档案和删除当前档案受保护；重命名和删除非当前档案不调用门禁。

- [x] **Step 3: provide 注册表并前置检查**

`App.vue` 创建并 provide registry。`selectProfile`、`createProfile`、删除当前 profile 先 await `attempt()`；false 直接返回。保留现有 `useProfileManagement` 的 busy、错误、刷新和同步语义。

- [x] **Step 4: 运行 App 与档案组件回归**

Run: `pnpm exec vitest run src/app/workspace-transition-guard.test.ts src/app/App.profile.test.ts src/modules/profiles/components/ProfileSwitcher.test.ts`

Run: `pnpm typecheck`

Expected: PASS。

---

### Task 5: 全量门禁与本地复核

**Files:**
- Verify: `docs/superpowers/plans/2026-07-30-workspace-transition-guard.md`

- [x] **Step 1: 运行完整门禁**

Run: `pnpm test:coverage`

Run: `pnpm lint`

Run: `pnpm typecheck`

Run: `pnpm build`

- [x] **Step 2: 本地代码复核**

检查注册顺序、重复请求、组件卸载、页面确认单飞、档案命令前置顺序、create/delete 范围、profile mutation 失败、路由刷新和多对话框可访问性；修复发现的问题并重跑相关门禁。

- [x] **Step 3: 更新计划记录**

记录最终测试数量、覆盖率、构建结果以及未暂存/未提交状态。

## Verification Record

- `pnpm test:coverage`: 93 个测试文件、564 项测试全部通过；statements 81.12%、branches 78.68%、functions 76.92%、lines 83.36%。
- `pnpm lint`: 通过，0 warning / 0 error。
- `pnpm typecheck`: 通过。
- `pnpm build`: 通过，Vite 转换 2033 个模块并生成生产资源。
- 测试探针消除 lint 告警后，`src/app/App.profile.test.ts` 16 项定向回归再次通过。
- `git diff --check`: 通过；暂存区为空。本批及既有整改均保持未暂存、未提交。
