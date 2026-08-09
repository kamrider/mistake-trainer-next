# Settings Unsaved Navigation Guard Implementation Plan

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:subagent-driven-development (recommended) or superpowers:executing-plans to implement this plan task-by-task. Steps use checkbox (`- [ ]`) syntax for tracking.

**Goal:** 阻止设置页未保存偏好在路由离开或窗口关闭时静默丢失，同时复用现有无障碍确认对话框提供明确的放弃选择。

**Architecture:** 新建 `useUnsavedChangesGuard`，接收 dirty/busy 读取器和可选路由守卫注册器，内部复用 `useActionConfirmation`。busy 时拒绝离开并通知页面；dirty 时异步询问用户；窗口 `beforeunload` 在 dirty/busy 时触发原生关闭保护。设置页只把两个偏好控制器状态合并后接入，并渲染现有 `ActionConfirmDialog`。

**Tech Stack:** Vue 3 Composition API、Vue Router 4、TypeScript、Vitest、Testing Library。

## Global Constraints

- 不处理许可证、隐私、客服、账户删除、设备迁移、Windows 更新恢复或支持 SLA 等上线前事项。
- 不新增依赖，不修改 Rust/OCR 算法或生成绑定，不暂存、不提交。
- 不使用 `window.confirm`；路由内导航必须使用现有可访问对话框。
- 同一时刻只允许一个未决导航确认；重复导航共享同一个决策，取消必须留在设置页，确认后由最新导航继续。
- 保存中的偏好不能被“放弃”绕过；先阻止离开，保存完成后用户可再次导航。
- 相同设置路由的 query/section 变化不触发离开确认。

---

### Task 1: 通用未保存更改守卫失败测试

**Files:**
- Create: `src/app/composables/useUnsavedChangesGuard.test.ts`
- Create: `src/app/composables/useUnsavedChangesGuard.ts`

**Interfaces:**

```ts
type NavigationAttempt = () => boolean | Promise<boolean>
interface UnsavedChangesGuardOptions {
  dirty: () => boolean
  busy: () => boolean
  onBusy: () => void
  registerNavigation?: (attempt: NavigationAttempt) => () => void
  confirmation: ActionConfirmationRequest
}
```

- [x] **Step 1: 写 clean/dirty 路由行为测试**

用测试宿主组件捕获 `attempt`。clean 返回 true；dirty 打开 `alertdialog` 并保持 Promise pending，取消后返回 false，确认后返回 true。

- [x] **Step 2: 写 busy 与重复请求测试**

busy 时同步返回 false、调用 onBusy 且不打开对话框。已有 dirty 对话框时第二次 attempt 不创建新对话框，而是共享同一个决策 Promise；一次确认或取消同时结算所有等待导航。

- [x] **Step 3: 写生命周期和 beforeunload 测试**

dirty/busy 时派发 cancelable `beforeunload` 必须 `defaultPrevented=true`；clean 不阻止。组件卸载后注销路由守卫和事件监听，并把未决确认结算为 false。

- [x] **Step 4: 运行测试确认失败**

Run: `pnpm exec vitest run src/app/composables/useUnsavedChangesGuard.test.ts`

Expected: FAIL，因为守卫尚不存在。

---

### Task 2: 实现通用守卫

**Files:**
- Create: `src/app/composables/useUnsavedChangesGuard.ts`
- Test: `src/app/composables/useUnsavedChangesGuard.test.ts`

- [x] **Step 1: 组合确认控制器**

调用 `useActionConfirmation()` 并暴露其 `current/confirm/cancel`；`attemptLeave()` 先检查 busy，再检查 dirty，最后调用 `ask(confirmation)`。

- [x] **Step 2: 注册路由回调与单飞**

setup 期间通过 `registerNavigation` 注册 `attemptLeave`。守卫缓存当前 decision Promise，重复 dirty attempt 共享它，避免 Vue Router 取消第一导航后最新导航也被拒绝；卸载时调用注销函数。

- [x] **Step 3: 实现关闭窗口保护**

mounted 时监听 `window.beforeunload`；dirty 或 busy 时调用 `event.preventDefault()` 并设置 `event.returnValue`。卸载时移除监听。

- [x] **Step 4: 运行定向覆盖率**

Run: `pnpm exec vitest run src/app/composables/useUnsavedChangesGuard.test.ts --coverage --coverage.include=src/app/composables/useUnsavedChangesGuard.ts`

Expected: PASS，statements、branches、functions、lines 均为 100%。

---

### Task 3: 设置页路由接入

**Files:**
- Modify: `src/app/views/SettingsView.vue`
- Modify: `src/app/views/SettingsView.test.ts`

- [x] **Step 1: 写 dirty 导航确认回归**

使用 memory router 与真实 `RouterView` 渲染设置页。修改科目后导航到其他路由，断言仍在 settings、对话框安全取消按钮获得焦点；取消保持草稿和路由，第二次导航并确认后才离开。

- [x] **Step 2: 写保存中阻断回归**

偏好保存 deferred 期间导航，断言路由不变、显示“正在保存” alert 且没有放弃对话框。保存完成后再次导航成功。

- [x] **Step 3: 接入路由注册器**

设置页通过现有可选 `appRouter` 注册 beforeEach：仅当 `from.name === 'settings' && to.name !== 'settings'` 时调用 guard attempt；同 settings query 直接放行。dirty 合并两个控制器，busy 合并两个 saving。

- [x] **Step 4: 渲染确认对话框**

请求文案明确“未保存的科目配置或训练节奏会丢失”，安全取消按钮为“继续编辑”，确认按钮为“放弃修改并离开”，tone=danger。组件事件接 guard confirm/cancel。

- [x] **Step 5: 运行设置页与对话框回归**

Run: `pnpm exec vitest run src/app/composables/useUnsavedChangesGuard.test.ts src/app/components/ActionConfirmDialog.test.ts src/app/views/SettingsView.test.ts`

Run: `pnpm typecheck`

Expected: PASS。

---

### Task 4: 全量门禁与复核

**Files:**
- Verify: `docs/superpowers/plans/2026-07-30-settings-unsaved-navigation-guard.md`

- [x] **Step 1: 运行完整门禁**

Run: `pnpm exec vitest run --coverage`

Run: `pnpm lint`

Run: `pnpm typecheck`

Run: `pnpm build`

- [x] **Step 2: 本地代码复核**

按商业软件标准检查路由 Promise 结算、重复导航、busy 竞态、焦点恢复、卸载清理、窗口关闭保护、同路由 query 放行和用户排除范围；修复发现的问题并重跑相关门禁。

- [x] **Step 3: 更新计划记录**

记录最终测试数量、覆盖率和构建结果，确认工作区未暂存、未提交。

## Verification Record

- 定向守卫覆盖率：4 tests passed；statements、branches、functions、lines 均为 100%。
- 设置页与确认框回归：49 tests passed；连续导航确认后采用最新目的地。
- 全量测试：92 files / 535 tests passed。
- 全量覆盖率：statements 80.73%、branches 78.50%、functions 76.19%、lines 82.95%。
- `pnpm lint`、`pnpm typecheck`、`pnpm build` 均通过；生产构建转换 2032 modules，无构建警告。
- 本地复核修复：重复 dirty 导航由拒绝后续请求改为共享同一 decision Promise，避免 Vue Router 取消首个导航后最新导航也被拒绝；删除单飞结算中的不可达身份分支。
- `git diff --check` 通过（仅 Git 提示现有 LF/CRLF 转换）；`git diff --cached --name-only` 为空，未暂存、未提交。
