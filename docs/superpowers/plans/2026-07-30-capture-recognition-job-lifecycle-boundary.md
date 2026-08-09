# Capture Recognition Job Lifecycle Boundary Implementation Plan

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:subagent-driven-development (recommended) or superpowers:executing-plans to implement this plan task-by-task. Steps use checkbox (`- [ ]`) syntax for tracking.

**Goal:** 将识别任务创建、领取、建议写入、人工复核、取消、失败和重启恢复的状态机及 SQL 从大型 `capture_recognition` 模块提取为独立私有生命周期模块，同时保持 Worker、命令和公开 API 完全兼容。

**Architecture:** 新建 `capture_recognition_job.rs` 作为 `capture_recognition` 的私有子模块，拥有识别任务生命周期行为、任务/建议读取和状态迁移 SQL。父模块继续作为公开门面并拥有共享 DTO、错误类型、区域校验、快照哈希以及识别结果应用/撤销事务，通过 `pub use` 保留所有当前函数路径。

**Tech Stack:** Rust 2024、rusqlite/SQLCipher、serde/serde_json、specta、UUID v7、PowerShell 架构契约、Cargo integration tests。

## Global Constraints

- 不修改 `src-tauri/src/infrastructure/recognition_visual_split.rs` 及任何 OCR/版面识别算法。
- 不处理许可证、隐私、客服、账户删除、设备迁移、更新失败恢复或支持 SLA 等上线前事项。
- 保持 Tauri 命令、Worker import、Specta bindings、数据库 schema、任务状态字符串、建议 JSON、错误优先级和幂等语义不变。
- 不改变 `apply_capture_recognition`、`revert_capture_recognition`、识别账本或故障注入路径。
- 不新增依赖，不暂存、不提交；保留工作区全部既有修改。
- 先让架构契约失败，再移动生产代码；父模块不得复制保留生命周期 SQL。

---

### Task 1: 锁定识别任务生命周期所有权

**Files:**
- Modify: `scripts/rust-boundary-contract.ps1`

**Interfaces:**
- Requires: `src-tauri/src/modules/capture_recognition_job.rs`
- Enforces: 十个公开任务操作和私有建议读取只能由生命周期模块定义。

- [x] **Step 1: 添加失败架构契约**

```powershell
$jobFunctions = @(
    'create_or_resume_recognition_job',
    'get_active_recognition_job',
    'get_recognition_job_by_id',
    'store_recognition_suggestion',
    'review_recognition_suggestion',
    'cancel_recognition_job',
    'reset_abandoned_recognition_work',
    'claim_next_recognition_item',
    'finish_recognition_item_without_suggestion',
    'fail_recognition_job'
)
foreach ($function in $jobFunctions) {
    Require-Pattern 'src-tauri/src/modules/capture_recognition_job.rs' `
        "(?m)^pub fn $function" `
        "Recognition job lifecycle function $function must remain in the job module"
    Reject-Pattern 'src-tauri/src/modules/capture_recognition.rs' `
        "(?m)^pub fn $function" `
        "Recognition job lifecycle function $function must not move back into the recognition facade"
}
Require-Pattern 'src-tauri/src/modules/capture_recognition_job.rs' '(?m)^fn list_suggestions' `
    'Recognition suggestion reads must remain in the job lifecycle module'
Reject-Pattern 'src-tauri/src/modules/capture_recognition.rs' '(?m)^fn list_suggestions' `
    'Recognition suggestion SQL must not move back into the recognition facade'
```

- [x] **Step 2: 运行契约确认失败**

Run: `pnpm contract:rust-boundaries`

Expected: FAIL，指出 `capture_recognition_job.rs` 缺失。

---

### Task 2: 提取完整识别任务状态机

**Files:**
- Create: `src-tauri/src/modules/capture_recognition_job.rs`
- Modify: `src-tauri/src/modules/capture_recognition.rs`

**Interfaces:**
- Consumes from parent:

```rust
CaptureRecognitionDecision
CaptureRecognitionError
CaptureRecognitionJob
CaptureRecognitionJobState
CaptureRecognitionReviewBand
CaptureRecognitionRole
CaptureRecognitionSuggestion
CaptureRecognitionSuggestionState
ClaimedCaptureRecognitionItem
CreateCaptureRecognitionJob
ReviewCaptureRecognitionSuggestion
StoreCaptureRecognitionSuggestion
MAX_JOB_ITEMS
capture_item_snapshot_hash
review_band
validate_regions
```

- Public compatibility:

```rust
pub use capture_recognition_job::{
    cancel_recognition_job, claim_next_recognition_item, create_or_resume_recognition_job,
    fail_recognition_job, finish_recognition_item_without_suggestion,
    get_active_recognition_job, get_recognition_job_by_id,
    reset_abandoned_recognition_work, review_recognition_suggestion,
    store_recognition_suggestion,
};
```

- [x] **Step 1: 移动任务创建和读取**

原样移动 `create_or_resume_recognition_job`、`get_active_recognition_job`、`get_recognition_job_by_id` 和私有 `list_suggestions`。保持：

```rust
if let Some(existing) = get_active_recognition_job(...) {
    return Ok(existing);
}
```

以及 account/profile/batch 所有权条件、任务排序和建议行顺序不变。

- [x] **Step 2: 移动建议写入与人工复核**

原样移动 `store_recognition_suggestion` 和 `review_recognition_suggestion`，继续调用父模块的 `validate_regions` 与 `review_band`；保持 low band 不可接受、编辑区域重新校验、processed_items 和 review 状态迁移规则不变。

- [x] **Step 3: 移动 Worker 领取和终止路径**

原样移动 `cancel_recognition_job`、`reset_abandoned_recognition_work`、`claim_next_recognition_item`、`finish_recognition_item_without_suggestion` 和 `fail_recognition_job`。保持重启时删除旧建议并重放完整任务、领取顺序、幂等取消和失败码长度限制不变。

- [x] **Step 4: 建立私有模块和兼容 re-export**

父模块添加：

```rust
#[path = "capture_recognition_job.rs"]
mod capture_recognition_job;
```

并按 Interfaces re-export。删除父模块中的原实现，不修改 Worker、命令、bindings 或测试 import。

- [x] **Step 5: 运行识别集成测试**

Run: `.\scripts\cargo-msvc.cmd test --manifest-path src-tauri\Cargo.toml --test capture_recognition`

Expected: 所有非 ignore 测试通过，真实运行时语料测试维持既有 ignore 状态。

---

### Task 3: Worker、命令和静态门禁

**Files:**
- Verify: `src-tauri/src/infrastructure/capture_recognition_worker.rs`
- Verify: `src-tauri/src/commands/capture_recognition.rs`
- Verify: `src-tauri/src/infrastructure/recognition_visual_split.rs`

**Interfaces:**
- Preserves: Worker 的领取/完成/失败调用，命令的创建/查询/复核/取消调用，以及 `modules::capture_recognition::*` 路径。

- [x] **Step 1: 运行命令与 Worker 相关编译门禁**

Run: `.\scripts\cargo-msvc.cmd clippy --manifest-path src-tauri\Cargo.toml --all-targets --all-features -- -D warnings`

Expected: 零告警通过，证明命令、Worker 和测试调用路径仍可编译。

- [x] **Step 2: 运行架构与格式检查**

Run: `pnpm contract:rust-boundaries`

Run: `.\scripts\cargo-msvc.cmd fmt --manifest-path src-tauri\Cargo.toml --all -- --check`

Expected: 均通过。

- [x] **Step 3: 本地代码复核**

确认生命周期 SQL 不在父模块重复出现；应用/撤销事务和共享校验未被移动；子模块保持私有；命令、Worker、bindings、schema、依赖和 OCR 文件未因本批修改；运行 `git diff --check` 并确认暂存区为空。

- [x] **Step 4: 记录验证结果**

记录识别测试数量、ignore 数量、架构契约、Clippy、格式、模块行数、暂存区状态和范围排除。

## Self-Review

- 需求覆盖：十个生命周期入口以及它们唯一的建议读取辅助函数全部纳入边界；应用/撤销事务明确排除，避免跨批扩大风险。
- Placeholder scan：无 TBD、TODO、未定义函数或模糊验证步骤。
- 类型一致性：全部函数签名由现有实现直接保持；父模块共享类型和私有校验可由子模块访问，外部继续通过父模块 re-export。

## Verification Record

- 架构 TDD：新增规则后先因 `capture_recognition_job.rs` 缺失而失败；完成提取后 `pnpm contract:rust-boundaries` 通过。
- 识别集成：25 项中 23 项通过、2 项真实语料/固定哈希 OCR 运行时测试按既有配置忽略。
- 覆盖范围：通过用例覆盖任务所有权、重复启动恢复、建议校验与置信区间、人工复核、幂等取消、Worker 领取/失败/坏资源隔离、重启重放、应用/撤销原子性以及三处故障注入无残留。
- 静态门禁：`cargo fmt --check`、全 targets/features `cargo clippy -D warnings`、Rust 边界契约和 `git diff --check` 均通过。
- 所有权：十个生命周期入口及 `list_suggestions` 只在 `capture_recognition_job.rs` 定义；父模块不存在任务创建、job item 创建、processed_items 生命周期更新或建议列表读取的重复实现。
- 事务隔离：`apply_capture_recognition`、`revert_capture_recognition`、`latest_capture_recognition_operation`、识别账本、故障注入、快照哈希、区域校验和 staging 清理均留在父模块，未改动实现。
- 模块体量：`capture_recognition.rs` 从 2016 行降至 1415 行；新私有 `capture_recognition_job.rs` 为 623 行。
- API/调用边界：子模块保持私有，十个函数经父模块 re-export；命令、Worker、bindings、schema、依赖和测试 import 未因本批修改。
- 本地代码审查：无 Critical、Important 或 Minor 遗留问题；迁移前后状态字符串、SQL 条件、排序、幂等和错误映射一致。
- 工作区：Git 暂存区为空，未暂存、未提交；命令、Worker 与用户已有的 `recognition_visual_split.rs` 修改均未触碰。
- 环境噪声：Windows 测试仍输出既有 OpenSSL PDB 与 SQLCipher `VirtualLock` 警告，但测试命令退出码为 0。
- 范围排除：未处理许可证、隐私、客服、账户删除、设备迁移、更新失败恢复或支持 SLA。
