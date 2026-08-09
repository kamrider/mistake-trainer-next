# Durable Action Transition Guard Implementation Plan

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:subagent-driven-development (recommended) or superpowers:executing-plans to implement this plan task-by-task. Steps use checkbox (`- [ ]`) syntax for tracking.

**Goal:** 统一阻止训练与导出持久化操作期间的路由离开、档案切换和窗口关闭，消除数据上下文竞态与散落守卫代码。

**Architecture:** 新建 `useDurableActionGuard`，只处理 busy 临界区，不承担 dirty 确认。组合函数注册可选工作区决策、监听 `beforeunload` 并提供可复用 `attemptLeave()`；页面仍通过 Vue Router 的 `onBeforeRouteLeave` 接入路由生命周期。训练页迁移现有手写逻辑，报告页首次接入三类离开保护。

**Tech Stack:** Vue 3 composables、Vue Router、TypeScript、Vitest、Testing Library。

## Global Constraints

- 不处理许可证、隐私、客服、账户删除、设备迁移、Windows 更新恢复或支持 SLA 等上线前事项。
- 不新增依赖，不修改 Rust/OCR 算法或生成绑定，不暂存、不提交。
- 正常空闲状态不得阻断导航、档案操作或窗口关闭。
- busy 时只显示页面已有 alert 文案，不新增确认对话框。
- 训练页文案保持“正在保存训练进度，请稍候再离开。”；报告页使用“导出操作正在进行，请等待完成后再离开。”
- 独立组件测试没有 App provider 时保持可运行。

---

### Task 1: 通用 durable action guard

**Files:**
- Create: `src/app/composables/useDurableActionGuard.ts`
- Create: `src/app/composables/useDurableActionGuard.test.ts`

**Interfaces:**
- Produces: `DurableActionAttempt = () => boolean`。
- Produces: `useDurableActionGuard({ busy, onBlocked, registerContextTransition? })`。
- Returns: `{ attemptLeave }`。

- [x] **Step 1: 写失败测试**

覆盖空闲 true、busy false 且调用一次 `onBlocked`、context registrar 收到同一个 attempt、busy 时 `beforeunload` 被阻止、空闲不阻止、卸载注销并移除监听。

- [x] **Step 2: 运行测试确认模块缺失**

Run: `pnpm exec vitest run src/app/composables/useDurableActionGuard.test.ts`

Expected: FAIL，模块不存在。

- [x] **Step 3: 实现组合函数**

setup 时可选注册 context attempt；mounted 添加 `beforeunload`；unmount 移除监听并调用 unregister。`attemptLeave()` 在 busy 时调用 `onBlocked` 并 false，否则 true。

- [x] **Step 4: 运行定向覆盖率**

Run: `pnpm exec vitest run src/app/composables/useDurableActionGuard.test.ts --coverage --coverage.include=src/app/composables/useDurableActionGuard.ts`

Expected: 四项覆盖率均为 100%。

---

### Task 2: 训练页迁移通用守卫

**Files:**
- Modify: `src/app/views/ReviewView.vue`
- Modify: `src/app/views/ReviewView.test.ts`

**Interfaces:**
- Consumes: `useDurableActionGuard`。

- [x] **Step 1: 扩展训练关闭测试**

持久化 deferred 时派发 `beforeunload`，断言被阻止；空闲时不阻止；卸载后不阻止。

- [x] **Step 2: 迁移实现**

删除手写 registry 注册与卸载逻辑。组合函数接收 `durableActionBusy`、现有提示赋值和可选 `workspaceTransitionGuard.register`；`onBeforeRouteLeave(attemptLeave)`。

- [x] **Step 3: 运行训练定向回归**

Run: `pnpm exec vitest run src/app/composables/useDurableActionGuard.test.ts src/app/views/ReviewView.test.ts src/app/workspace-transition-guard.test.ts`

Run: `pnpm typecheck`

Expected: 全部通过。

---

### Task 3: 报告导出接入三类保护

**Files:**
- Modify: `src/app/views/ReportView.vue`
- Modify: `src/app/views/ReportView.test.ts`

**Interfaces:**
- Consumes: `useDurableActionGuard`、`workspaceTransitionGuardKey`。

- [x] **Step 1: 写导出临界区失败测试**

使用真实 workspace registry 和 routed ReportView。让 `exportGenerate` deferred；操作期间路由 push 保持 report、workspace attempt false、`beforeunload` 被阻止并显示统一 alert。操作完成后三者均允许。卸载后窗口监听被移除。

- [x] **Step 2: 实现报告页接入**

inject 可选 workspace registry。以 `operationBusy` 为 busy，blocked 时写入报告页错误文案；注册 context、路由 leave 与 beforeunload。

- [x] **Step 3: 运行报告与档案编排回归**

Run: `pnpm exec vitest run src/app/views/ReportView.test.ts src/app/App.profile.test.ts src/app/views/ReviewView.test.ts`

Run: `pnpm typecheck`

Expected: 全部通过。

---

### Task 4: 完整门禁与复核

**Files:**
- Verify: `docs/superpowers/plans/2026-07-30-durable-action-transition-guard.md`

- [x] **Step 1: 运行完整门禁**

Run: `pnpm test:coverage`

Run: `pnpm lint`

Run: `pnpm typecheck`

Run: `pnpm build`

- [x] **Step 2: 本地代码复核**

检查 busy 覆盖范围、正常离开、重复调用、窗口监听生命周期、路由和档案切换一致性、原生文件生成取消、错误提示与无 provider 兼容；修复发现的问题。

- [x] **Step 3: 更新验证记录**

记录最终测试数、覆盖率、构建模块数、`git diff --check` 和暂存区状态。

## Verification Record

- `pnpm test:coverage`: 94 个测试文件、569 项测试全部通过；statements 81.24%、branches 78.84%、functions 77.07%、lines 83.47%。
- `pnpm lint`: 通过，0 warning / 0 error。
- `pnpm typecheck`: 通过。
- `pnpm build`: 通过，Vite 转换 2034 个模块并生成生产资源。
- 通用守卫定向覆盖率：statements、branches、functions、lines 均为 100%。
- `git diff --check`: 通过；暂存区为空。本批和既有整改均未暂存、未提交。
