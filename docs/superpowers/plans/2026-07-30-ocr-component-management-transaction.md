# OCR Component Management Transaction Implementation Plan

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:executing-plans to implement this plan task-by-task. Steps use checkbox (`- [ ]`) syntax for tracking.

**Goal:** 让 OCR 组件安装、移除和能力刷新共享同一个事务语义，避免持久化操作成功后因二次探测失败而显示旧按钮、误报失败或夸大功能已经启用。

**Architecture:** 新建 `useOcrComponentManagement` 管理能力快照、请求 epoch、组件变更单飞和用户消息。组件命令成功后先用命令返回的 `OcrComponentStatus` 更新组件快照，再尝试读取完整 `OcrCapabilityStatus`；刷新失败保留已确认的组件状态并明确标记完整能力待刷新。设置页只负责提供命令适配器和渲染只读状态。

**Tech Stack:** Vue 3 Composition API、TypeScript、Vitest、Testing Library、Tauri 生成绑定。

## Global Constraints

- 不处理许可证、隐私、客服、账户删除、设备迁移、Windows 更新恢复或支持 SLA 等上线前事项。
- 不修改 Rust/OCR 算法文件和生成绑定，不新增依赖，不暂存、不提交。
- 使用 TDD；旧能力探测不得覆盖更新的组件事务，组件成功不得因为后续刷新失败而被误报成失败。
- 只有完整能力状态明确 `automaticRecognitionEnabled` 时，才提示题号定位增强已启用。

---

### Task 1: 控制器失败测试

**Files:**
- Create: `src/modules/ocr/composables/useOcrComponentManagement.test.ts`
- Create: `src/modules/ocr/composables/useOcrComponentManagement.ts`

- [x] **Step 1: 定义能力读取与组件命令依赖**

测试通过注入的 `fetchCapability/installComponent/removeComponent` 返回 `AppResult`，不依赖真实 Tauri 命令。

- [x] **Step 2: 写安装成功与完整刷新测试**

断言组件命令返回值立即进入快照；完整刷新成功后原子替换整个能力状态。仅当刷新结果启用自动识别时才显示“增强已就绪”。

- [x] **Step 3: 写持久化成功、刷新失败的降级测试**

覆盖应用错误和异常。断言安装/移除仍返回成功，保留命令确认的组件状态，并提示“完整能力状态暂时未刷新”，不得显示“没有安装完成/没有移除”。

- [x] **Step 4: 写命令失败和竞态测试**

应用错误使用服务端消息，异常使用稳定降级消息，旧探测晚到不能覆盖新事务；busy 时重复安装、移除和刷新均不得启动新调用。

- [x] **Step 5: 运行测试确认失败**

Run: `pnpm exec vitest run src/modules/ocr/composables/useOcrComponentManagement.test.ts`

Expected: FAIL，因为控制器文件尚不存在。

---

### Task 2: 实现 OCR 组件事务控制器

**Files:**
- Create: `src/modules/ocr/composables/useOcrComponentManagement.ts`
- Test: `src/modules/ocr/composables/useOcrComponentManagement.test.ts`

- [x] **Step 1: 实现只读状态和 latest-start-wins 探测**

暴露只读 `capability/busy/message`；每次能力读取递增 epoch，只有最新请求可以提交。

- [x] **Step 2: 实现组件返回值合并**

按组件 id 替换能力快照中的单项，不猜测 `recognitionFeature` 或 `automaticRecognitionEnabled` 等只能由完整探测确认的派生字段。

- [x] **Step 3: 实现持久化成功后的刷新降级**

安装/移除命令成功先提交组件状态，再在同一 epoch 内刷新完整能力。刷新失败保留局部真值并返回成功；命令失败才使用失败文案。

- [x] **Step 4: 实现单飞和消息真实性**

busy 时所有组件变更及外部刷新返回 false。安装完成后根据完整能力状态选择“增强已就绪”或“模型已安装”；刷新失败显示部分成功消息。

- [x] **Step 5: 运行定向覆盖率**

Run: `pnpm exec vitest run src/modules/ocr/composables/useOcrComponentManagement.test.ts --coverage --coverage.include=src/modules/ocr/composables/useOcrComponentManagement.ts`

Expected: PASS，四项覆盖率均达到 100%。

---

### Task 3: 设置页接入与体验回归

**Files:**
- Modify: `src/app/views/SettingsView.vue`
- Modify: `src/app/views/SettingsView.test.ts`

- [x] **Step 1: 写设置页部分成功回归**

模拟安装命令成功、能力刷新失败，断言按钮切换为“移除模型”，状态明示完整能力待刷新，并且不出现“增强已就绪”。

- [x] **Step 2: 写未启用能力的真实文案回归**

模拟完整刷新成功但 `automaticRecognitionEnabled: false`，断言只提示模型已安装，不声称智能切图已经自动启用。

- [x] **Step 3: 接入控制器**

删除页面内 `ocrCapability/ocrBusy/ocrMessage` 和三个散落函数；用命令适配器创建控制器，页面加载调用 `refreshCapability()`，面板绑定只读状态。

- [x] **Step 4: 收紧刷新入口**

OCR 组件变更期间禁用设置页总刷新，防止用户启动互相矛盾的状态探测；其他设置行为保持不变。

- [x] **Step 5: 运行 OCR 与设置页测试及类型检查**

Run: `pnpm exec vitest run src/modules/ocr/composables/useOcrComponentManagement.test.ts src/modules/ocr/components/OcrCapabilityPanel.test.ts src/app/views/SettingsView.test.ts`

Run: `pnpm typecheck`

Expected: PASS。

---

### Task 4: 全量门禁与复核

**Files:**
- Verify: `docs/superpowers/plans/2026-07-30-ocr-component-management-transaction.md`

- [x] **Step 1: 运行完整门禁**

Run: `pnpm exec vitest run --coverage`

Run: `pnpm lint`

Run: `pnpm typecheck`

Run: `pnpm build`

- [x] **Step 2: 本地代码复核**

按商业软件标准检查竞态、消息真实性、只读边界、可访问禁用状态、用户排除范围与未提交改动隔离；修复本批问题后重跑相关门禁。

- [x] **Step 3: 更新计划记录**

将已完成步骤勾选，并记录最终测试数量、覆盖率和构建结果；保持工作区未暂存、未提交。

## Verification Record

- 控制器定向覆盖率：statements、branches、functions、lines 均为 100%。
- OCR/设置页定向回归：3 个测试文件、55 个测试全部通过。
- 完整测试：90 个测试文件、517 个测试全部通过。
- 完整覆盖率：statements 80.43%、branches 78.29%、functions 75.82%、lines 82.69%。
- `pnpm lint`、`pnpm typecheck`、`pnpm build` 均退出码 0；生产构建转换 2030 个模块，无新增 bundle warning。
- 本地复核修正：能力派生状态暂未刷新时，面板只有在 `automaticRecognitionEnabled` 且 small 组件仍为 `installed` 时才展示“题号增强已启用”。
- 未修改 Windows 更新链路、Rust/OCR 算法或生成绑定；未暂存、未提交。
