# Settings Preference Save Queue Implementation Plan

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:subagent-driven-development (recommended) or superpowers:executing-plans to implement this plan task-by-task. Steps use checkbox (`- [ ]`) syntax for tracking.

**Goal:** 让科目配置和训练节奏在保存期间继续可编辑，自动续存最新草稿，并阻止页面总刷新静默覆盖未保存偏好。

**Architecture:** 新建通用 `useQueuedPreferenceSave<TInput, TOutput>` 管理 dirty revision、单飞保存循环、最新草稿合并和消息状态。设置页的编辑函数只更新本地不可变草稿并调用 `markChanged()`；保存控制器忽略旧响应对新草稿的覆盖，检测到保存期间的新 revision 时自动发送最新快照，最终只应用最后一次服务端确认结果。

**Tech Stack:** Vue 3 Composition API、TypeScript、Vitest、Testing Library、Tauri 生成绑定。

## Global Constraints

- 不处理许可证、隐私、客服、账户删除、设备迁移、Windows 更新恢复或支持 SLA 等上线前事项。
- 不修改 Rust/OCR 算法、生成绑定，不新增依赖，不暂存、不提交。
- 使用 TDD；保存期间允许继续编辑，旧响应绝不能覆盖更新的本地草稿。
- 应用错误和异常保留最新草稿及 dirty 状态；只有最终 revision 保存成功后才能清除 dirty。
- 页面总刷新不得静默覆盖 dirty 或正在保存的偏好。

---

### Task 1: 偏好保存队列失败测试

**Files:**
- Create: `src/app/composables/useQueuedPreferenceSave.test.ts`
- Create: `src/app/composables/useQueuedPreferenceSave.ts`

**Interfaces:**
- Consumes: `snapshot(): TInput | undefined`、`persist(input): Promise<AppResult<TOutput>>`、`applySaved(output): void`。
- Produces: 只读 `saving/dirty/message` 与 `markChanged()/save()`。

- [x] **Step 1: 写稳定草稿保存测试**

快照成功持久化后应用服务端结果、显示成功文案、dirty=false、saving=false；没有快照时 `save()` 返回 false 且不调用 persist。

- [x] **Step 2: 写保存期间编辑自动续存测试**

第一请求 deferred；调用 `markChanged()` 并替换 snapshot 后完成第一请求。断言旧结果不调用 `applySaved`，控制器自动发第二请求且内容为最新快照，最终只应用第二结果。

- [x] **Step 3: 写单飞与合并测试**

保存中再次 `save()` 返回 false 且不创建重复请求；连续多次 `markChanged()` 只在当前请求完成后发送一次最新快照，并显示“会自动继续保存”状态。

- [x] **Step 4: 写错误与验证测试**

应用错误采用 `userMessage`，异常使用领域稳定失败文案；两者均保留 dirty 与本地草稿。`validate(input)` 返回消息时不得调用 persist。

- [x] **Step 5: 运行测试确认失败**

Run: `pnpm exec vitest run src/app/composables/useQueuedPreferenceSave.test.ts`

Expected: FAIL，因为控制器尚不存在。

---

### Task 2: 实现通用最新草稿保存事务

**Files:**
- Create: `src/app/composables/useQueuedPreferenceSave.ts`
- Test: `src/app/composables/useQueuedPreferenceSave.test.ts`

**Interfaces:**

```ts
interface QueuedPreferenceSaveOptions<TInput, TOutput> {
  snapshot: () => TInput | undefined
  persist: (input: TInput) => Promise<AppResult<TOutput>>
  applySaved: (output: TOutput) => void
  validate?: (input: TInput) => string | undefined
  successMessage: string
  failureMessage: string
  queuedMessage: string
}
```

- [x] **Step 1: 实现 revision 与只读状态**

`markChanged()` 递增 revision、设置 dirty；空闲时清除旧状态消息，保存中显示 queuedMessage。使用 `readonly(ref)` 暴露 saving、dirty、message。

- [x] **Step 2: 实现单飞保存循环**

`save()` 在 saving 或无 snapshot 时返回 false；验证成功后进入循环。每轮记录 requestRevision 并持久化不可变快照；响应成功但 revision 已变化时不应用旧结果，继续读取最新 snapshot。

- [x] **Step 3: 实现最终提交与失败保持**

只有 revision 等于 requestRevision 时调用 applySaved、dirty=false 并显示成功消息。应用错误或异常立即停止、保持 dirty，并在 finally 清除 saving。

- [x] **Step 4: 运行定向覆盖率**

Run: `pnpm exec vitest run src/app/composables/useQueuedPreferenceSave.test.ts --coverage --coverage.include=src/app/composables/useQueuedPreferenceSave.ts`

Expected: PASS，statements、branches、functions、lines 均为 100%。

---

### Task 3: 科目与训练节奏接入

**Files:**
- Modify: `src/app/views/SettingsView.vue`
- Modify: `src/app/views/SettingsView.test.ts`

- [x] **Step 1: 写科目保存期间编辑回归**

启动科目保存 deferred 后切换另一科目或音效；完成第一响应后断言自动发送第二请求，旧服务端结果没有撤销新编辑，最后显示保存成功且采用第二响应。

- [x] **Step 2: 写训练节奏保存期间编辑回归**

保存 `every_10` 期间改为 `session_start`；第一响应完成后自动续存 `session_start`，界面不会短暂回退为旧策略，最终只显示最新策略已保存。

- [x] **Step 3: 接入两个控制器**

删除页面手写 saving/message refs 与两个 save try/catch。两个控制器分别构造不可变 `SubjectPreferencesInput`/`ReviewPreferencesInput`，命令结果用 `normalizeAppResult`；所有有效偏好编辑函数调用对应 `markChanged()`。

- [x] **Step 4: 保持验证与错误语义**

科目控制器 validate 保证至少一个启用科目；服务端错误和异常不重置草稿。训练节奏最终成功文案继续说明从下一轮普通训练生效。

- [x] **Step 5: 运行设置页回归与类型检查**

Run: `pnpm exec vitest run src/app/composables/useQueuedPreferenceSave.test.ts src/app/components/SettingsSubjectPanel.test.ts src/app/components/SettingsReviewPanel.test.ts src/app/views/SettingsView.test.ts`

Run: `pnpm typecheck`

Expected: PASS。

---

### Task 4: 未保存草稿保护与全量复核

**Files:**
- Modify: `src/app/views/SettingsView.vue`
- Modify: `src/app/views/SettingsView.test.ts`
- Verify: `docs/superpowers/plans/2026-07-30-settings-preference-save-queue.md`

- [x] **Step 1: 写总刷新保护测试**

修改任一偏好但不保存后点击“刷新”，断言不调用第二次偏好读取，页面保留草稿并显示“请先保存偏好”的 alert；保存成功后刷新恢复可用。

- [x] **Step 2: 实现 load guard**

`load()` 在任一控制器 dirty 或 saving 时返回 false，保留所有当前状态并设置明确 errorMessage；初次 mounted load 与干净状态刷新保持原行为。

- [x] **Step 3: 运行完整门禁**

Run: `pnpm exec vitest run --coverage`

Run: `pnpm lint`

Run: `pnpm typecheck`

Run: `pnpm build`

- [x] **Step 4: 本地代码复核与计划记录**

按商业软件标准检查丢失更新、队列合并、失败保留、可访问状态、刷新保护、用户排除范围和工作区隔离；修复发现的问题并重跑相关门禁。记录最终测试数量、覆盖率与构建结果，保持未暂存、未提交。

## Verification Record

- 保存队列定向覆盖率：statements、branches、functions、lines 均为 100%。
- 偏好与设置页定向回归：4 个测试文件、57 个测试全部通过（其中设置页与两个面板 50 个，控制器 7 个）。
- 完整测试：91 个测试文件、528 个测试全部通过。
- 完整覆盖率：statements 80.63%、branches 78.42%、functions 76.01%、lines 82.86%。
- `pnpm lint`、`pnpm typecheck`、`pnpm build` 均退出码 0；生产构建转换 2031 个模块，无新增 bundle warning。
- 本地复核修正：保存控制器暴露单调递增 edit revision；设置刷新只在启动 revision 未变化时应用偏好读取结果，防止刷新开始后编辑并保存仍被旧读取覆盖。
- 未修改 Windows 更新、上线前事项、Rust/OCR 算法或生成绑定；未暂存、未提交。
