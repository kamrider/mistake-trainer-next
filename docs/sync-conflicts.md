# 同步冲突与双设备收敛

本文记录 Mistake Trainer Next v1 对可变同步实体的合并规则、冲突恢复路径和
Windows 双设备验收步骤。自动化测试可以证明数据库规则是确定的，但不能替代
真实 Supabase 项目、真实网络中断和两台独立安装包的发布验收。

## 适用范围

冲突中心只处理以下可变实体：

- `learner_profile`：字段 `name`。
- `problem`：字段 `subject`、`tags`、`note`、`status`、
  `timeLimitSeconds` 和有序的 `assets`。
- `export_snapshot`：字段 `title`、有序的 `problemIds` 和
  `configuration`。

`ReviewEvent` 是只追加事件，跨设备取并集；`Asset` 是按明文 SHA-256
去重的不可变内容。两者都不进入字段冲突中心。

## 三向合并真值表

每次拉取可变实体时，Rust 同时比较：

- `base`：上次确认的云端完整快照；
- `local`：本机当前完整实体；
- `remote`：这次收到的云端完整实体。

每个字段独立执行以下规则：

| 本机相对 base | 云端相对 base | 本机与云端 | 结果 |
| --- | --- | --- | --- |
| 未改 | 未改 | 相同 | 采用云端值，不产生新推送 |
| 未改 | 已改 | 不同 | 采用云端值 |
| 已改 | 未改 | 不同 | 保留本机值，生成一个高于云端 revision 的规范 upsert |
| 已改 | 已改 | 相同 | 采用共同值，不产生冲突 |
| 已改 | 已改 | 不同 | 保留本机显示值，并为该字段创建一个真实冲突 |
| 没有 base | 任意 | 相同 | 采用共同值 |
| 没有 base | 任意 | 不同 | 保守地创建冲突，不猜测哪一端更新 |

`tags`、`problemIds` 和 `assets` 在比较前使用规范顺序；`assets` 整体作为一个
有序字段，不能把两端不同的题答顺序静默拼接。创建时间、更新时间、revision
和同步游标是合并元数据，不作为用户可选字段。

同一拉取页中的游标推进、远端快照写入、本地合并、冲突写入和必要的 outbox
替换位于同一个 SQLite 事务中。任一动作失败时整页回滚，游标不前进。

## 冲突期间与解决后的行为

1. 存在未解决冲突时，普通题目编辑、题目状态修改和档案重命名会被拒绝，并引导
   用户前往 **设置 → 同步冲突**。
2. 冲突卡片同时显示“本机版本”和“云端版本”。用户可以逐字段选择，也可以对
   当前实体一次性全部采用某一端；系统不会预选具有破坏性的答案。
3. 同一实体的批量解决在一个事务中完成。最后一个字段解决后才会重新开放普通
   编辑。
4. 若最终内容等于保存的远端快照，实体使用远端 revision，且不会产生多余推送。
5. 若最终内容仍包含本机选择，实体 revision 设为
   `max(local revision, remote revision) + 1`，并只生成一个完整规范 upsert。
6. 已解决记录不会删除。`resolution`、`resolved_value_json` 和
   `resolved_at_utc_ms` 会保留，供诊断和迁移审计。

冲突值通过带 `kind` 标签的 `JsonValue` 返回前端。JSON 数字以字符串承载，
因此超过 JavaScript 安全整数范围的诊断值也不会在 Rust/TypeScript 边界被截断。

## 删除冲突

远端删除与本机未同步修改并发时，字段名为 `__deleted__`：

- 采用本机：保留实体，revision 高于远端 tombstone，并生成一个完整 upsert。
- 采用云端：删除实体、清除该实体的陈旧 outbox，但保留 tombstone 和冲突审计。
- 接受档案删除：其未解决子实体冲突统一记为
  `resolution = 'remote'`、`resolved_value_json = 'null'`；已解决的历史记录不改写。
  该档案的快照和非资产 outbox 被清理，共享资产仍按引用关系保留。

最后一个学习档案不能通过冲突解决或云端删除而消失。删除当前档案时，应用先选择
一个仍存在的替代档案。

## 数据库结构与迁移编号

schema v13 的 `sync_entity_snapshots` 以
`(account_id, entity_type, entity_id)` 为主键，保存 `profile_id`、远端
`revision`、完整 `payload_json` 和更新时间。`sync_conflicts_open_field_idx`
保证同一账户、实体、字段最多只有一个未解决冲突。

备份验证要求 v13 同时具有快照表、冲突审计列和对应索引，并拒绝混入其他账户的
快照。v12 备份可恢复后迁移到 v13。未来“自动框题”固定使用 schema v14
（`0014_capture_region_suggestions.sql`），不得再次占用 0013。

## 自动化证据

以下测试组成确定性的数据库收敛证据：

- `sync_pull`：不同字段自动合并、同字段只生成一个冲突、新版本关闭过时冲突、
  远端删除与本机修改冲突、重复变更和游标事务性。
- `sync_conflicts`：逐字段/整实体采用本机或云端、无额外推送、单一新 revision、
  非法值回滚、跨账户/档案隐藏、删除与档案级清理、审计保留。
- `sync_push`：网络失败后使用相同 operation ID 幂等重放，不遗留错误确认。
- `database_schema` 与 `backup_store`：v12 → v13、唯一开放冲突索引和备份边界。
- `problem_lifecycle` 与 `profile_store`：冲突期间普通编辑保护。
- `SyncConflictCenter.test.ts` 与 `SettingsView.test.ts`：展示、键盘焦点、繁忙态、
  错误保留、减少动态效果和解决后的计数刷新。

开发机验证命令：

```powershell
pnpm lint
pnpm typecheck
pnpm test
pnpm build
.\scripts\cargo-msvc.cmd test --manifest-path src-tauri\Cargo.toml --all-targets
pnpm bindings:check
```

SQLCipher 的 `VirtualLock` 1453 和 OpenSSL 静态库缺失调试 PDB 是当前 Windows
开发机上的已知非致命警告；任何测试失败、panic 或新的安全警告都阻断发布。

## Windows 双设备验收

使用一个一次性 Supabase 开发项目、两个全新本地数据目录和同一个已验证邮箱账户。
设备 A、B 必须安装同一候选构建。每个场景开始前记录两端 pull cursor、目标实体
revision、开放冲突数、outbox 数和相关资产 SHA-256。

### 场景矩阵

- [ ] **不同字段**：两端同步到同一题目 revision；A 离线改科目，B 离线改笔记。
  先同步 B，再同步 A，再同步 B。最终两端同时包含两个修改、revision 相同且没有
  开放冲突。
- [ ] **相同字段**：A、B 离线写入不同笔记。先同步 B，再同步 A。A 只出现一个
  `note` 冲突；重启 A 后冲突仍存在，题目普通编辑仍被锁定。
- [ ] **相同结果**：A、B 离线把同一字段改成完全相同的值。依次同步后不得出现
  冲突或额外 revision。
- [ ] **档案名称**：两端离线把同一档案改成不同名称。确认冲突卡片不暴露账户 ID、
  本地路径或令牌；逐字段采用一个名称后，两端收敛。
- [ ] **远端删除**：A 离线修改题目笔记，B 删除该题并同步。A 同步后原题仍可在
  冲突卡片中识别。分别用一次性数据测试“保留本机”和“接受删除”两条路径。
- [ ] **本机解决**：A 对同字段冲突采用本机版本并同步，B 再同步。两端内容和
  revision 相等，只产生一个规范 upsert，解决审计仍保留。
- [ ] **云端解决**：A 对同字段冲突采用云端版本。确认 A 不新增该实体 outbox；
  B、A 再各同步一次后计数保持稳定。
- [ ] **长时间离线**：A 连续离线编辑多个不同字段并重启，B 在线产生至少三个更高
  revision。恢复 A 后，只对真正同字段分歧提示冲突，其他字段自动合并。
- [ ] **重放页面**：在开发代理中让同一 change page 返回两次，并在一次上传确认前
  中断连接。重试后不得有重复开放冲突、重复 ReviewEvent 或卡住的
  `processing` operation。
- [ ] **开放冲突重启**：保留一个未解决冲突，完全退出并重新启动 A。设置页计数、
  两端值和可选动作必须恢复；显式退出账户后本地库锁定。

### 最终不变量

每个场景完成最后一轮双向同步后同时检查：

- [ ] 两端规范实体的用户字段和 revision 完全相等。
- [ ] 两端 pull cursor 只增不减，并达到云端当前最大 `change_seq`。
- [ ] 每个账户/实体/字段最多一个开放冲突，已解决审计行仍存在。
- [ ] `sync_operations` 中没有超时后仍为 `processing` 的记录，也没有同一实体的
  陈旧 upsert/delete 并存。
- [ ] ReviewEvent ID 集合在两端相等且无重复，重算出的到期时间相同。
- [ ] 相关 Asset 的明文 SHA-256、题答角色和顺序相同；源图字节不因冲突解决改变。
- [ ] 接受删除后 tombstone 仍存在；恢复或保留产生的新 revision 高于删除 revision。
- [ ] 设置页的待上传数、开放冲突数和最后同步时间与数据库状态一致。

真实 Supabase、RLS、两台实体设备和断网代理的勾选结果应随候选版本归档。若没有
这些外部条件，只能记录“自动化规则通过”，不得将本节标记为发布验收完成。
