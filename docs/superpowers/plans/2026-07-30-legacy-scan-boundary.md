# Legacy Scan Boundary Implementation Plan

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:executing-plans to implement this plan task-by-task. Steps use checkbox (`- [ ]`) syntax for tracking.

**Goal:** 将不可信旧数据目录的扫描、指纹、元数据解析和安全预算抽成私有模块，并阻止超深目录导致无界递归。

**Architecture:** `legacy.rs` 继续拥有候选生命周期、导入事务、加密资产暂存、同步账本和回滚；新的私有 `legacy_scan.rs` 拥有扫描公开 DTO/错误、旧格式反序列化类型、计划构建、树指纹、安全路径校验和所有扫描预算。父模块通过兼容 `pub use` 保持 `modules::legacy::*` 调用路径不变。

**Tech Stack:** Rust 2024、Serde、SHA-256、image、time、Rusqlite/SQLCipher 集成测试、PowerShell architecture contract。

## Global Constraints

- 不修改旧数据 JSON 格式、导入结果、公开 Rust 路径、Tauri command、bindings、schema 或依赖。
- 保留现有 512 档案、2048 目录条目、100000 记录、64 MB 单资产、8 GB 总资产、10000 问题报告预算。
- 新增最大目录深度 32；超过预算必须返回 `LegacyScanError::Io(InvalidData)`，不能 panic、栈溢出或继续遍历。
- 不修改 `src-tauri/src/infrastructure/recognition_visual_split.rs`。
- 不处理许可证、隐私政策文本、客服、账户删除、设备迁移、更新失败恢复或 SLA。
- 不暂存、不提交，不覆盖工作区已有修改。

---

### Task 1: 建立安全回归和失败架构契约

**Files:**
- Modify: `src-tauri/tests/legacy_import_plan.rs`
- Modify: `scripts/rust-boundary-contract.ps1`

**Interfaces:**
- Existing public function:

```rust
pub fn legacy_tree_fingerprint(root: &Path) -> Result<String, LegacyScanError>
```

- Required private module functions: `build_legacy_import_plan`、`legacy_tree_fingerprint`、`scan_legacy_storage`、`collect_fingerprint_files`、`read_bounded`。

- [x] **Step 1: 运行 legacy 基线测试**

Run:

```powershell
.\scripts\cargo-msvc.cmd test --manifest-path src-tauri\Cargo.toml --test legacy_scan --test legacy_import_plan --test legacy_import_store --test legacy_command
```

Expected: 15 项现有测试全部通过。

- [x] **Step 2: 添加深层目录安全回归**

在 `src-tauri/tests/legacy_import_plan.rs` 添加：

```rust
#[test]
fn fingerprint_rejects_excessive_directory_depth() {
    let directory = tempfile::tempdir().unwrap();
    let mut nested = directory.path().to_path_buf();
    for _ in 0..33 {
        nested.push("d");
        std::fs::create_dir(&nested).unwrap();
    }
    std::fs::write(nested.join("asset.bin"), b"content").unwrap();

    let error = legacy_tree_fingerprint(directory.path())
        .expect_err("directory nesting beyond the scan budget must fail");
    assert!(error.to_string().contains("too deeply nested"));
}
```

- [x] **Step 3: 运行回归确认现有缺口**

Run:

```powershell
.\scripts\cargo-msvc.cmd test --manifest-path src-tauri\Cargo.toml --test legacy_import_plan fingerprint_rejects_excessive_directory_depth
```

Expected: FAIL，现实现会为深层目录生成指纹。

- [x] **Step 4: 添加失败架构契约**

在 `scripts/rust-boundary-contract.ps1` 添加：

```powershell
Require-Pattern 'src-tauri/src/modules/legacy_scan.rs' `
    '(?m)^pub fn build_legacy_import_plan' `
    'Legacy import plan construction must remain in the legacy scan module'
Require-Pattern 'src-tauri/src/modules/legacy_scan.rs' `
    '(?m)^pub fn legacy_tree_fingerprint' `
    'Legacy tree fingerprinting must remain in the legacy scan module'
Require-Pattern 'src-tauri/src/modules/legacy_scan.rs' `
    '(?m)^pub fn scan_legacy_storage' `
    'Legacy storage inspection must remain in the legacy scan module'
Require-Pattern 'src-tauri/src/modules/legacy_scan.rs' `
    '(?m)^fn collect_fingerprint_files' `
    'Legacy traversal budgets must remain in the legacy scan module'
Require-Pattern 'src-tauri/src/modules/legacy_scan.rs' `
    'const MAX_DIRECTORY_DEPTH: usize = 32;' `
    'Legacy directory traversal must remain depth bounded'
```

并拒绝父模块重新定义上述三个公开扫描函数、`LegacyStore`、`MemberSource` 或 `BoundedReadError`。

- [x] **Step 5: 运行契约确认失败**

Run: `pnpm contract:rust-boundaries`

Expected: FAIL，`legacy_scan.rs` 尚不存在。

---

### Task 2: 提取扫描边界并实施遍历预算

**Files:**
- Create: `src-tauri/src/modules/legacy_scan.rs`
- Modify: `src-tauri/src/modules/legacy.rs`

**Interfaces:**
- Child consumes:

```rust
LegacyAssetPlan
LegacyImportPlan
LegacyMemberPlan
LegacyProblemPlan
LegacyRating
LegacyReviewPlan
```

- Parent compatibility:

```rust
#[path = "legacy_scan.rs"]
mod legacy_scan;

pub use legacy_scan::{
    LegacyIssue, LegacyScanError, LegacyScanReport, build_legacy_import_plan,
    legacy_tree_fingerprint, scan_legacy_storage,
};
```

- [x] **Step 1: 移动完整扫描实现**

将 `LegacyScanError` 起至文件末尾的连续代码移动到 `legacy_scan.rs`，包括：

```text
LegacyScanError / LegacyIssue / LegacyScanReport
LegacyStore / LegacyFile / MemberSource / BoundedReadError
build_legacy_import_plan / legacy_tree_fingerprint / scan_legacy_storage
all scan-only helpers
```

同时将扫描专属常量移动到子模块；父模块只保留 `TOMBSTONE_RETENTION_MILLIS`。移除父模块仅供扫描使用的 `Component`、`Read`、`GenericImageView`、`Deserialize`、`Sha256` 和 `time` imports，由编译器确认最终最小导入。

- [x] **Step 2: 保持公开路径兼容**

父模块声明私有子模块并按 Interfaces re-export；不修改 tests、commands 或调用者的 `modules::legacy::*` import。

- [x] **Step 3: 在遍历过程中限制深度和数量**

新增：

```rust
const MAX_DIRECTORY_DEPTH: usize = 32;
const MAX_FINGERPRINT_FILES: usize = MAX_RECORDS + MAX_DIRECTORY_ENTRIES;
```

入口调用：

```rust
collect_fingerprint_files(&canonical_root, &canonical_root, 0, &mut files)?;
```

helper 签名：

```rust
fn collect_fingerprint_files(
    root: &Path,
    directory: &Path,
    depth: usize,
    files: &mut Vec<(String, PathBuf)>,
) -> Result<(), LegacyScanError>
```

在读取目录前拒绝 `depth > MAX_DIRECTORY_DEPTH`。按剩余全局文件预算使用 `read_dir(...).take(remaining.saturating_add(1))`；多出一个条目立即返回 `InvalidData("legacy tree contains too many files")`，递归时使用 `depth.saturating_add(1)`。删除遍历完成后的迟到数量检查。

- [x] **Step 4: 运行深度定向回归**

Run:

```powershell
.\scripts\cargo-msvc.cmd test --manifest-path src-tauri\Cargo.toml --test legacy_import_plan fingerprint_rejects_excessive_directory_depth
```

Expected: PASS，错误包含 `too deeply nested`。

- [x] **Step 5: 运行扫描与计划测试**

Run:

```powershell
.\scripts\cargo-msvc.cmd test --manifest-path src-tauri\Cargo.toml --test legacy_scan --test legacy_import_plan
```

Expected: 10 项全部通过。

---

### Task 3: 导入兼容与静态门禁

**Files:**
- Verify: `src-tauri/tests/legacy_import_store.rs`
- Verify: `src-tauri/tests/legacy_command.rs`
- Verify: `src-tauri/src/commands/legacy.rs`
- Verify: `src-tauri/src/infrastructure/recognition_visual_split.rs`

**Interfaces:**
- Preserves: 计划重扫、源指纹校验、加密暂存、事务回滚、同步操作和可逆导入账本。

- [x] **Step 1: 运行完整 legacy 测试**

Run:

```powershell
.\scripts\cargo-msvc.cmd test --manifest-path src-tauri\Cargo.toml --test legacy_scan --test legacy_import_plan --test legacy_import_store --test legacy_command
```

Expected: 16 项全部通过。

- [x] **Step 2: 运行架构、格式和 Clippy**

Run:

```powershell
pnpm contract:rust-boundaries
.\scripts\cargo-msvc.cmd fmt --manifest-path src-tauri\Cargo.toml --all -- --check
.\scripts\cargo-msvc.cmd clippy --manifest-path src-tauri\Cargo.toml --all-targets --all-features -- -D warnings
```

Expected: 全部通过。

- [x] **Step 3: 本地代码复核**

确认扫描类型、JSON 解析、路径验证、指纹和预算只在子模块；父模块继续拥有候选/导入/回滚；深度与文件数量在遍历过程中生效；公开 re-export 与错误转换不变；commands、bindings、schema、依赖和 OCR 文件未因本批修改。

- [x] **Step 4: 运行最终差异检查**

Run:

```powershell
git diff --check
git diff --cached --quiet
```

Expected: 无空白错误，暂存区为空。

- [x] **Step 5: 记录验证结果**

在本文追加基线、红灯、绿灯、完整测试、静态门禁、模块行数、审查结果、暂存区状态和范围排除。

## Self-Review

- 需求覆盖：扫描职责拆分、公开路径兼容、深度/数量实时预算、导入兼容与静态门禁均有明确证据。
- Placeholder scan：无 TBD、TODO、未定义函数或模糊错误处理步骤。
- 类型一致性：扫描 DTO 与错误由子模块定义并由父模块 re-export；`LegacyImportPlan` 仍由父模块定义，子模块只构造该类型，不形成公开循环依赖。

## Verification Record

- 基线：拆分前 15 项 legacy 测试通过。
- 红灯：33 层目录测试先失败并返回了指纹；架构契约先因 `legacy_scan.rs` 不存在而失败。
- 绿灯：深度定向回归 1/1 通过；扫描与计划测试 10/10 通过。
- 完整链路：候选 2、计划 3、导入/回滚 4、扫描 7，共 16/16 通过。
- 静态门禁：Rust architecture boundary contract、`cargo fmt --check`、全目标 Clippy `-D warnings` 均通过。
- 实现强化：计划中的“文件数预算”升级为累计目录条目预算；在 `read_dir` 收集时即时限制整个树的文件和目录总量，覆盖范围更强。
- 模块规模：`legacy.rs` 从 2266 行降为 1296 行；私有 `legacy_scan.rs` 为 1023 行。
- 本地复审：未发现 Critical 或 Important 问题；扫描公开 DTO、解析类型、指纹、递归和预算均归子模块，导入事务与回滚仍归父模块。
- 范围核对：本批未改 Tauri commands、bindings、schema、依赖或 `recognition_visual_split.rs`；关键排除文件时间戳仍为本批开始前。
- 工作区：`git diff --check` 通过，暂存区为空；所有本批修改保持未暂存、未提交。
- 环境告警：测试仍输出既有 OpenSSL PDB `LNK4099` 与 SQLCipher `VirtualLock LastError=1453` 告警，但命令退出码为 0。
