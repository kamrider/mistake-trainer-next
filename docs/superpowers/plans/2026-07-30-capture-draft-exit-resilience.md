# Capture Draft Exit Resilience Implementation Plan

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:subagent-driven-development (recommended) or superpowers:executing-plans to implement this plan task-by-task. Steps use checkbox (`- [ ]`) syntax for tracking.

**Goal:** 确保采集草稿的最新文字修改在排队、保存中或保存失败时不会因关闭详情、切换批次、路由离开或窗口关闭而静默丢失，并提供明确重试路径。

**Architecture:** 扩展 `useCaptureDraftSaveQueue`，把运行中、排队中和失败待重试状态统一发布为只读快照；失败更新保留在队列内，但不自动无限重试，只有显式 `retry()` 或同一草稿的新编辑才继续。`CaptureView` 使用现有 `useUnsavedChangesGuard` 组合该状态：可继续保存的 pending/running 属于 busy，直接阻止离开；failed 属于 dirty，允许用户重试或通过可访问对话框明确放弃。`CaptureWorkspace` 只负责展示失败重试按钮，不持有事务状态。

**Tech Stack:** Vue 3 Composition API、Vue Router、TypeScript、Vitest、Testing Library。

## Global Constraints

- 不处理许可证、隐私、客服、账户删除、设备迁移、Windows 更新恢复或支持 SLA 等上线前事项。
- 不新增依赖，不修改 Rust/OCR 算法或生成绑定，不暂存、不提交。
- 不使用 `window.confirm`；失败草稿的放弃必须使用现有 `ActionConfirmDialog`。
- 排队或保存中的草稿不允许被“放弃”绕过；必须等待保存结算。只有失败待重试状态允许明确放弃。
- 同一草稿的新编辑覆盖旧失败快照并自动恢复保存；不同草稿和不同批次的快照不得互相覆盖。
- 队列不得对同一失败无限自动重试；revision conflict 仍最多自动修复一次。
- clean 状态的批次切换、路由导航、OCR 设置跳转和窗口关闭不得被误拦。

---

### Task 1: 保存队列状态与失败快照测试

**Files:**
- Modify: `src/modules/capture/composables/useCaptureDraftSaveQueue.test.ts`
- Modify: `src/modules/capture/composables/useCaptureDraftSaveQueue.ts`

**Interfaces:**
- Produces: `CaptureDraftSaveQueueState { pending: boolean; running: boolean; retryRequired: boolean }`。
- Produces: queue method `retry(): Promise<void>` and option `onStateChange(state: CaptureDraftSaveQueueState): void`。

- [x] **Step 1: 写状态快照失败测试**

enqueue 后立即发布 `pending/running`；perform 进行中为 `running=true`；最终成功后三个状态均 false。第一笔运行时加入第二笔，第一笔成功后状态仍显示存在未保存更新，第二笔成功后才 clean。

- [x] **Step 2: 写失败保留和显式重试测试**

`failed` 或异常结果后断言 `retryRequired=true` 且 update 快照仍被队列持有，不发生自动第二次 perform。调用 `retry()` 后以完全相同输入再执行；成功后 clean。

- [x] **Step 3: 写新编辑覆盖失败测试**

失败后 enqueue 同一 draft 的更新版本，断言旧失败快照被替换且新版本自动保存。若失败期间已有更新版本排队，不得把旧失败重新插回覆盖它。

- [x] **Step 4: 运行测试确认失败**

Run: `pnpm exec vitest run src/modules/capture/composables/useCaptureDraftSaveQueue.test.ts`

Expected: FAIL，因为队列尚未公开状态、保留失败快照或实现 retry。

---

### Task 2: 实现可恢复保存队列

**Files:**
- Modify: `src/modules/capture/composables/useCaptureDraftSaveQueue.ts`
- Test: `src/modules/capture/composables/useCaptureDraftSaveQueue.test.ts`

- [x] **Step 1: 建模 active 与 failed 更新**

增加 `activeUpdate` 和按 key 保存的 `failed` 快照。`publishState()` 每次 enqueue、开始、结算、retain、clear、dispose 后报告 `{ pending: pending.size > 0, running, retryRequired: failed.size > 0 }`。

- [x] **Step 2: 保留失败但停止自动循环**

perform 返回 failed、第二次 revision conflict 或抛错时，只有不存在同 key 新版本才把当前更新写入 failed。末尾只自动 flush `pending`，不得自动执行 failed。

- [x] **Step 3: 实现 retry 与覆盖规则**

`retry()` 将当前活动批次的 failed 快照移动回 pending，再调用 flush。enqueue 同 key 时删除 failed 并写入新 generation；retainBatch/clear/dispose 同时处理 failed。

- [x] **Step 4: 运行定向覆盖率**

Run: `pnpm exec vitest run src/modules/capture/composables/useCaptureDraftSaveQueue.test.ts --coverage --coverage.include=src/modules/capture/composables/useCaptureDraftSaveQueue.ts`

Expected: PASS；statements、branches、functions、lines 均达到 95% 以上，所有失败/覆盖分支有断言。

---

### Task 3: 采集页离开事务与重试入口

**Files:**
- Modify: `src/app/views/CaptureView.vue`
- Modify: `src/app/views/CaptureView.test.ts`
- Modify: `src/modules/capture/components/CaptureWorkspace.vue`
- Modify: `src/modules/capture/components/CaptureWorkspace.test.ts`

**Interfaces:**
- Consumes: queue `onStateChange` and `retry()`。
- Produces: workspace prop `draftSaveRetryAvailable: boolean` and event `retryDraftSave`。

- [x] **Step 1: 写排队/保存中离开失败测试**

使用真实 memory router + `RouterView`。第一笔 draft update deferred，随后再发第二笔更新；尝试导航到 dashboard、关闭详情和打开另一批次，均保持当前批次并显示“最新草稿仍在保存，请等待完成后再离开。”，不出现放弃对话框。两笔保存完成后导航成功。

- [x] **Step 2: 写失败重试与明确放弃测试**

draft update 失败后断言 workspace 出现“重试保存草稿”。点击重试只调用一次额外命令且保留相同输入；失败状态下路由离开打开 `alertdialog`，取消保留当前批次，确认后才允许离开。

- [x] **Step 3: 写 clean 与窗口生命周期测试**

clean 状态允许同 inbox query、其他路由和批次切换；pending/running/failed 时 `beforeunload` 被阻止；组件卸载后监听与路由守卫注销。

- [x] **Step 4: 接入通用守卫**

`CaptureView` 保存 queue state refs。guard 的 dirty 为任意未保存状态；busy 为 `pending || running`；busy 文案明确等待；failed 使用“放弃尚未保存的采集草稿？”确认。路由注册仅处理从 inbox 离开或 `batchId` 改变；相同 batchId query 直接放行。

- [x] **Step 5: 统一详情与批次切换入口**

新增 `requestCloseDetail()` 和 `requestOpenBatch(batchId)`，都先调用 `attemptLeave()`。确认放弃后再执行原 `leaveDetail/loadDetail`，由 batch watcher 清理失败快照。内部 router replace 不得二次弹窗。

- [x] **Step 6: 增加重试按钮**

`CaptureWorkspace` 在 `saveState === 'error' && draftSaveRetryAvailable` 时显示原生 button“重试保存草稿”，点击 emit `retryDraftSave`；busy 时 disabled。父页连接 `draftSaveQueue.retry()`。

- [x] **Step 7: 运行采集回归**

Run: `pnpm exec vitest run src/modules/capture/composables/useCaptureDraftSaveQueue.test.ts src/modules/capture/components/CaptureWorkspace.test.ts src/app/views/CaptureView.test.ts`

Run: `pnpm typecheck`

Expected: PASS。

---

### Task 4: 全量门禁与本地复核

**Files:**
- Verify: `docs/superpowers/plans/2026-07-30-capture-draft-exit-resilience.md`

- [x] **Step 1: 运行完整门禁**

Run: `pnpm test:coverage`

Run: `pnpm lint`

Run: `pnpm typecheck`

Run: `pnpm build`

- [x] **Step 2: 本地代码复核**

检查双更新竞态、失败快照覆盖、revision conflict、显式重试单飞、批次切换、内部 query replace、路由 Promise、beforeunload、卸载清理、按钮可访问性和 clean 导航；修复发现的问题并重跑相关门禁。

- [x] **Step 3: 更新计划记录**

记录最终测试数量、覆盖率、构建结果以及未暂存/未提交状态。

## Verification Record

- 保存队列定向回归：20 tests passed；statements 100%、branches 98.03%、functions 100%、lines 100%。
- 采集三层定向回归：queue + workspace + view 共 79 tests passed；额外确认框并存回归通过。
- 全量测试：92 files / 557 tests passed。
- 全量覆盖率：statements 81.03%、branches 78.76%、functions 76.64%、lines 83.28%。
- `pnpm lint`、`pnpm typecheck`、`pnpm build` 均通过；生产构建转换 2032 modules，无构建警告。
- 本地复核修复：clean 批次入口保持同步快速路径；不同草稿的失败状态不会被后续成功遮蔽；浏览器 `batchId` 历史会同步详情；已有破坏性确认时不会叠加草稿离开对话框。
- 排队/运行中的草稿阻止路由、关闭详情、切批次和 beforeunload；失败快照只能显式重试或经确认放弃；组件卸载注销守卫。
- `git diff --check` 通过（仅 Git 提示现有 LF/CRLF 转换）；`git diff --cached --name-only` 为空，未暂存、未提交。
