# Review Workspace Transition Guard Implementation Plan

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:subagent-driven-development (recommended) or superpowers:executing-plans to implement this plan task-by-task. Steps use checkbox (`- [ ]`) syntax for tracking.

**Goal:** 阻止训练进度持久化期间的档案切换、新建或删除当前档案绕过训练页现有路由保护。

**Architecture:** `ReviewView` 继续保留现有 `onBeforeRouteLeave`，同时把同一条 busy 判断注册到应用级 `WorkspaceTransitionGuard`。正常且可恢复的训练会话不增加退出确认；只有评分提交、考试位置保存或专注进度持久化正在执行时返回 false，并复用现有错误提示。

**Tech Stack:** Vue 3 provide/inject、Vue Router、TypeScript、Vitest、Testing Library。

## Global Constraints

- 不处理许可证、隐私、客服、账户删除、设备迁移、Windows 更新恢复或支持 SLA 等上线前事项。
- 不新增依赖，不修改 Rust/OCR 算法或生成绑定，不暂存、不提交。
- 正常训练可安全恢复，不新增“是否退出训练”确认；只保护持久化中的短暂临界区。
- 路由离开和工作区切换必须复用同一判断与同一用户提示。
- 独立渲染测试没有 App provider 时必须保持可运行。

---

### Task 1: 训练页注册工作区切换判断

**Files:**
- Modify: `src/app/views/ReviewView.vue`
- Modify: `src/app/views/ReviewView.test.ts`

**Interfaces:**
- Consumes: `workspaceTransitionGuardKey`。
- Produces: 训练页活动期间注册 `() => boolean`，卸载时注销。

- [x] **Step 1: 写失败测试**

在训练页测试中提供真实 `createWorkspaceTransitionGuard()`：评分提交 deferred 时，调用 `guard.attempt()` 返回 false、显示“正在保存训练进度，请稍候再离开。”且当前命令未被重复执行；提交完成后再次 attempt 返回 true。卸载后 attempt 也返回 true。

- [x] **Step 2: 运行测试确认失败**

Run: `pnpm exec vitest run src/app/views/ReviewView.test.ts`

Expected: 新测试在持久化期间错误返回 true。

- [x] **Step 3: 实现最小接入**

在 `ReviewView` inject 可选 registry。提取 `attemptDurableTransition()`：busy 时设置现有错误并返回 false，否则 true。路由守卫调用它；registry 存在时注册它，并在 `onBeforeUnmount` 注销。

- [x] **Step 4: 运行定向回归**

Run: `pnpm exec vitest run src/app/views/ReviewView.test.ts src/app/workspace-transition-guard.test.ts src/app/App.profile.test.ts`

Run: `pnpm typecheck`

Expected: 全部通过。

---

### Task 2: 完整门禁与复核

**Files:**
- Verify: `docs/superpowers/plans/2026-07-30-review-workspace-transition-guard.md`

- [x] **Step 1: 运行完整门禁**

Run: `pnpm test:coverage`

Run: `pnpm lint`

Run: `pnpm typecheck`

Run: `pnpm build`

- [x] **Step 2: 本地复核**

检查正常训练不阻断、busy 类型完整、路由与档案切换提示一致、并发命令不重复、卸载注销、无 provider 兼容和可访问提示；修复问题并重跑相关门禁。

- [x] **Step 3: 更新验证记录**

记录测试数量、覆盖率、构建模块数、`git diff --check` 与暂存区状态。

## Verification Record

- `pnpm test:coverage`: 93 个测试文件、565 项测试全部通过；statements 81.17%、branches 78.79%、functions 76.94%、lines 83.41%。
- `pnpm lint`: 通过，0 warning / 0 error。
- `pnpm typecheck`: 通过。
- `pnpm build`: 通过，Vite 转换 2033 个模块并生成生产资源。
- `git diff --check`: 通过；暂存区为空。本批和既有整改均未暂存、未提交。
