# Library Detail Navigation Guard Implementation Plan

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:subagent-driven-development (recommended) or superpowers:executing-plans to implement this plan task-by-task. Steps use checkbox (`- [ ]`) syntax for tracking.

**Goal:** 防止题库详情中的未保存编辑在路由离开、窗口关闭或保存中的关闭操作里静默丢失，并复用统一的无障碍确认交互。

**Architecture:** `ProblemDetailDrawer` 用现有 `useUnsavedChangesGuard` 取代私有确认控制器，让抽屉关闭、相邻题导航、状态切换和路由离开共享同一决策。`LibraryView` 只提供限定在 library 路由边界的守卫注册器；抽屉继续拥有 dirty 计算和提示文案，避免父页面复制编辑状态。保存中且仍为 dirty 时拒绝离开并显示页内 alert，保存确认完成后自动恢复可导航状态。

**Tech Stack:** Vue 3 Composition API、Vue Router、TypeScript、Vitest、Testing Library。

## Global Constraints

- 不处理许可证、隐私、客服、账户删除、设备迁移、Windows 更新恢复或支持 SLA 等上线前事项。
- 不新增依赖，不修改 Rust/OCR 算法或生成绑定，不暂存、不提交。
- 不使用 `window.confirm`；全部应用内离开操作复用现有 `ActionConfirmDialog`。
- 同一 library 路由的 query 变化直接放行；只有从 library 离开到其他路由时才确认。
- dirty 且保存中时拒绝关闭、切题和路由离开，并明确告知“正在保存”；成功或失败结算后允许用户再次操作。
- clean 状态的训练启动和状态操作不得被守卫阻断。

---

### Task 1: 抽屉级离开保护失败测试

**Files:**
- Modify: `src/modules/library/components/ProblemDetailDrawer.test.ts`
- Modify: `src/modules/library/components/ProblemDetailDrawer.vue`

**Interfaces:**
- Consumes: `NavigationAttempt` from `src/app/composables/useUnsavedChangesGuard.ts`。
- Produces: optional prop `registerNavigation?: (attempt: NavigationAttempt) => () => void`。

- [x] **Step 1: 写路由 dirty 决策测试**

渲染抽屉时捕获 `registerNavigation` 的 attempt。编辑科目后调用 attempt，断言只出现一个 `alertdialog`；取消返回 false 并保留输入，确认返回 true。

- [x] **Step 2: 写保存中阻断测试**

dirty 后把 `saving` 更新为 true。调用捕获的 attempt 和点击关闭，均返回/保持不离开；断言显示“题目修改正在保存，请等待完成后再离开。”且不出现放弃对话框。

- [x] **Step 3: 运行测试确认失败**

Run: `pnpm exec vitest run src/modules/library/components/ProblemDetailDrawer.test.ts`

Expected: FAIL，因为抽屉尚未注册路由守卫，保存中仍可进入放弃确认。

---

### Task 2: 抽屉复用通用守卫

**Files:**
- Modify: `src/modules/library/components/ProblemDetailDrawer.vue`
- Test: `src/modules/library/components/ProblemDetailDrawer.test.ts`

- [x] **Step 1: 替换私有确认控制器**

移除 `useActionConfirmation`，调用 `useUnsavedChangesGuard`：`dirty` 读取现有 computed；`busy` 仅在 `props.saving && dirty.value` 时为 true；`registerNavigation` 转发可选 prop；确认文案保持“放弃尚未保存的修改？”。

- [x] **Step 2: 统一所有离开入口**

`requestClose`、`requestStatus`、`requestNavigate` 都调用 guard 的 `attemptLeave()`。busy 回调设置本地 alert；`saving` 结束时清除该提示。clean 状态仍直接放行。

- [x] **Step 3: 渲染保存中提示并验证生命周期**

在抽屉内容中渲染 `role="alert"` 的离开阻断文案。保留现有确认框焦点与卸载结算行为，并由通用守卫提供 `beforeunload` 保护和注册注销。

- [x] **Step 4: 运行定向测试**

Run: `pnpm exec vitest run src/app/composables/useUnsavedChangesGuard.test.ts src/modules/library/components/ProblemDetailDrawer.test.ts`

Run: `pnpm typecheck`

Expected: PASS。

---

### Task 3: 题库真实路由集成

**Files:**
- Modify: `src/app/views/LibraryView.vue`
- Modify: `src/app/views/LibraryView.test.ts`

**Interfaces:**
- Consumes: `NavigationAttempt` and drawer `registerNavigation` prop。
- Produces: `registerDetailNavigation(attempt)`，仅拦截 `from.name === 'library' && to.name !== 'library'`。

- [x] **Step 1: 写真实路由 dirty 回归**

用 memory router + `RouterView` 打开题库与详情，修改科目后 push dashboard。断言仍在 library、确认框安全取消按钮获得焦点；取消保留草稿，再次离开并确认后到 dashboard。

- [x] **Step 2: 写同页 query 与保存中回归**

dirty 时 push `{ name: 'library', query: { section: 'active' } }` 直接成功且不弹窗。把 `problemUpdate` 延迟，点击保存后 push dashboard 必须被拒绝并显示保存提示；命令结算后再次 push 成功。

- [x] **Step 3: 接入路由注册器**

`LibraryView` 定义稳定注册函数并传给抽屉。守卫对同名 library 路由直接返回 true，对其他来源/去向不干预，对离开 library 调用 attempt。

- [x] **Step 4: 运行题库回归**

Run: `pnpm exec vitest run src/modules/library/components/ProblemDetailDrawer.test.ts src/app/views/LibraryView.test.ts`

Run: `pnpm typecheck`

Expected: PASS。

---

### Task 4: 全量门禁与本地复核

**Files:**
- Verify: `docs/superpowers/plans/2026-07-30-library-detail-navigation-guard.md`

- [x] **Step 1: 运行完整门禁**

Run: `pnpm test:coverage`

Run: `pnpm lint`

Run: `pnpm typecheck`

Run: `pnpm build`

- [x] **Step 2: 本地代码复核**

检查路由 Promise 结算、重复导航、保存竞态、clean 训练导航、同页 query、焦点恢复、beforeunload 与卸载注销；修复发现的问题并重跑相关门禁。

- [x] **Step 3: 更新计划记录**

记录最终测试数量、覆盖率、构建结果以及未暂存/未提交状态。

## Verification Record

- 抽屉与真实路由定向回归：2 files / 17 tests passed；通用守卫联合回归：2 files / 11 tests passed。
- 全量测试：92 files / 539 tests passed。
- 全量覆盖率：statements 80.76%、branches 78.54%、functions 76.28%、lines 82.97%。
- `pnpm lint`、`pnpm typecheck`、`pnpm build` 均通过；生产构建转换 2032 modules，无构建警告。
- 本地复核：dirty 的关闭、切题、状态操作、路由离开和 beforeunload 共享一个决策；保存中 dirty 离开被阻止并有 alert；clean 训练导航与同 library query 放行；卸载时注销守卫并结算未决确认。
- `git diff --check` 通过（仅 Git 提示现有 LF/CRLF 转换）；暂存区为空，未暂存、未提交。
