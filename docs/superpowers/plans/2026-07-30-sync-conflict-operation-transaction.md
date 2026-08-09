# 同步冲突操作事务整改计划

> 执行约束：采用测试驱动；不处理许可证、隐私、客服、账户删除、设备迁移、更新失败恢复、支持 SLA 等上线前事项；不提交、不暂存，不碰现有 OCR/Rust 及生成绑定改动。

## 目标

把冲突列表读取和冲突解决收敛到单一操作控制器，避免刷新与解决并发时旧列表覆盖已完成操作，并确保焦点恢复、同步调度或事件监听失败不会被误报为“保存失败”。

## 任务 1：用失败测试固定事务语义

- [x] 新建 `src/modules/sync/composables/useSyncConflictOperations.test.ts`。
- [x] 覆盖最新读取获胜、旧成功/旧异常忽略、应用错误/异常保留现有列表。
- [x] 覆盖读取或其他操作进行中时拒绝解决。
- [x] 覆盖解决成功原子更新列表和状态、解决失败保留列表、通知副作用异常不撤销成功。
- [x] 覆盖解决过程中触发的刷新在操作结束后只补跑一次。
- [x] 在 `SyncConflictCenter.test.ts` 增加刷新期间所有解决按钮禁用的交互回归。
- [x] 先运行聚焦测试，确认新增场景在实现前失败。

## 任务 2：实现共享冲突操作控制器

- [x] 新建 `src/modules/sync/composables/useSyncConflictOperations.ts`。
- [x] 控制器统一拥有 `conflicts`、`loading`、`busyKey`、错误和成功状态。
- [x] `reload()` 采用递增请求序号，只有最新请求可提交状态；解决期间刷新排队。
- [x] `resolve()` 在读取/写入期间互斥，成功原子替换快照并隔离调度及事件副作用。
- [x] 聚焦运行控制器测试与覆盖率，确保新控制器 100% 行、语句、函数和分支覆盖。

## 任务 3：接入冲突中心

- [x] `SyncConflictCenter.vue` 使用控制器，删除重复的命令状态机。
- [x] 字段与整组解决均在控制器返回成功后恢复焦点；焦点异常不得修改命令结果。
- [x] 刷新时禁用逐字段、整组按钮和打开批量确认入口。
- [x] 保留 `defineExpose({ reload })` 与现有用户文案/可访问性行为。
- [x] 运行组件聚焦测试和类型检查。

## 任务 4：质量门禁与复核

- [x] 运行完整测试及覆盖率、lint、typecheck、build。
- [x] 按商业软件标准本地复核竞态、失败语义、可访问性和改动边界。
- [x] 修复重要发现并重新运行受影响门禁。

## 验证命令

```powershell
pnpm exec vitest run src/modules/sync/composables/useSyncConflictOperations.test.ts src/modules/sync/components/SyncConflictCenter.test.ts
pnpm exec vitest run src/modules/sync/composables/useSyncConflictOperations.test.ts --coverage --coverage.include=src/modules/sync/composables/useSyncConflictOperations.ts
pnpm exec vitest run --coverage
pnpm lint
pnpm typecheck
pnpm build
```
