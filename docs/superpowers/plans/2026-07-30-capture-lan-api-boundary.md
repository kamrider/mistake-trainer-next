# Capture LAN API Boundary Implementation Plan

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:executing-plans to implement this plan task-by-task. Steps use checkbox (`- [ ]`) syntax for tracking.

**Goal:** 将手机局域网采集的 HTTP API、上传流、授权和错误响应从会话管理器中拆出，并确保所有认证 API 响应明确禁止缓存。

**Architecture:** `capture_lan.rs` 继续拥有公开 DTO、会话生命周期、监听器、超时状态和私网地址选择；新的私有 `capture_lan_api.rs` 拥有 Axum router、移动页面资源、所有 API handler、上传临时文件流程、授权和 API 错误映射；测试移至 `capture_lan_api_tests.rs`，作为 API 子模块的测试后代访问私有实现。父模块只通过 `pub(super) fn build_router(Arc<ServerState>) -> Router` 调用子模块，公开 Rust/Tauri API 不变。

**Tech Stack:** Rust 2024、Axum、Tokio、Rusqlite/SQLCipher、Tower test service、PowerShell architecture contract。

## Global Constraints

- 不修改 Tauri command 标识、bindings、数据库 schema、依赖、移动端协议路径或 JSON 字段。
- 不修改 `src-tauri/src/infrastructure/recognition_visual_split.rs`。
- 不处理许可证、隐私政策文本、客服、账户删除、设备迁移、更新失败恢复或 SLA。
- 保留 token 哈希常量时间比较、Host/Origin 校验、30 分钟空闲和 2 小时绝对超时、50 MB 单文件上限及 30 秒上传停滞超时。
- 不暂存、不提交，不覆盖工作区已有修改。

---

### Task 1: 建立隐私回归和架构契约

**Files:**
- Modify: `src-tauri/src/modules/capture_lan.rs`
- Modify: `scripts/rust-boundary-contract.ps1`

**Interfaces:**
- Existing API test router: `build_router(Arc<ServerState>) -> Router`
- Required response header: `Cache-Control: no-store`
- Required boundary entry: `capture_lan_api::build_router`

- [x] **Step 1: 运行 LAN 模块基线测试**

Run:

```powershell
.\scripts\cargo-msvc.cmd test --manifest-path src-tauri\Cargo.toml --lib capture_lan::tests
```

Expected: 10 项现有测试全部通过。

- [x] **Step 2: 添加认证 API 禁缓存回归**

在现有 `mobile_page_hardens_headers_and_keeps_heic_decoder_lazy` 测试之后添加：

```rust
#[test]
fn authenticated_api_responses_are_never_cacheable() {
    let server = TestServer::new();
    let runtime = tokio::runtime::Builder::new_current_thread()
        .enable_all()
        .build()
        .expect("runtime");

    let success = runtime
        .block_on(server.router.clone().oneshot(server.request(
            Method::GET,
            "/api/v1/session",
            Body::empty(),
        )))
        .expect("session response");
    assert_eq!(
        success
            .headers()
            .get(header::CACHE_CONTROL)
            .and_then(|value| value.to_str().ok()),
        Some("no-store")
    );

    let unauthorized = runtime
        .block_on(server.router.oneshot(
            Request::builder()
                .uri("/api/v1/session")
                .header(header::HOST, "127.0.0.1:3210")
                .header(header::AUTHORIZATION, "Bearer wrong-token")
                .body(Body::empty())
                .expect("unauthorized request"),
        ))
        .expect("unauthorized response");
    assert_eq!(unauthorized.status(), StatusCode::UNAUTHORIZED);
    assert_eq!(
        unauthorized
            .headers()
            .get(header::CACHE_CONTROL)
            .and_then(|value| value.to_str().ok()),
        Some("no-store")
    );
}
```

- [x] **Step 3: 运行回归确认现有缺口**

Run:

```powershell
.\scripts\cargo-msvc.cmd test --manifest-path src-tauri\Cargo.toml --lib authenticated_api_responses_are_never_cacheable
```

Expected: FAIL，成功响应没有 `Cache-Control`。

- [x] **Step 4: 添加失败架构契约**

在 `scripts/rust-boundary-contract.ps1` 中要求：

```powershell
Require-Pattern 'src-tauri/src/modules/capture_lan_api.rs' `
    '(?m)^pub\(super\) fn build_router' `
    'Capture LAN HTTP routing must remain in the LAN API module'
Require-Pattern 'src-tauri/src/modules/capture_lan_api.rs' `
    '(?m)^async fn upload_item' `
    'Capture LAN upload streaming must remain in the LAN API module'
Require-Pattern 'src-tauri/src/modules/capture_lan_api.rs' `
    '(?m)^fn authorize' `
    'Capture LAN request authorization must remain in the LAN API module'
Require-Pattern 'src-tauri/src/modules/capture_lan_api.rs' `
    '(?m)^async fn harden_api_response' `
    'Capture LAN authenticated responses must remain non-cacheable'
```

并拒绝父模块重新定义 `build_router`、`upload_item`、`authorize`、`ApiError`。

- [x] **Step 5: 运行契约确认失败**

Run: `pnpm contract:rust-boundaries`

Expected: FAIL，`capture_lan_api.rs` 尚不存在。

---

### Task 2: 提取 HTTP API 并统一安全响应

**Files:**
- Create: `src-tauri/src/modules/capture_lan_api.rs`
- Create: `src-tauri/src/modules/capture_lan_api_tests.rs`
- Modify: `src-tauri/src/modules/capture_lan.rs`

**Interfaces:**
- Consumes from parent: `ServerState`、`ActiveSession`、`SessionActivity`、`CaptureLanContext`、`run_server`、`session_temp_root`、`constant_time_eq`、`current_utc_millis`。
- Produces:

```rust
pub(super) fn build_router(state: Arc<ServerState>) -> Router
```

- Parent declaration:

```rust
#[path = "capture_lan_api.rs"]
mod capture_lan_api;
```

- [x] **Step 1: 移动 API 生产代码**

将 `build_router` 起至 `impl IntoResponse for ApiError` 止的连续生产代码移动到 `capture_lan_api.rs`，同时移动：

```rust
MOBILE_PAGE
HEIC2ANY_SCRIPT
MAX_ORIGINAL_UPLOAD_BYTES
UPLOAD_STALL_TIMEOUT
```

父模块 `run_server` 改为调用 `capture_lan_api::build_router`。仅将 `build_router` 设为 `pub(super)`；其余 handler、payload 和 `ApiError` 保持私有。

- [x] **Step 2: 移动 API 测试**

把原 `#[cfg(test)] mod tests` 的模块内容移动到 `capture_lan_api_tests.rs`，并由 API 子模块声明：

```rust
#[cfg(test)]
#[path = "capture_lan_api_tests.rs"]
mod tests;
```

测试通过 `use super::*;` 使用 API 私有函数，并显式从父模块导入会话测试夹具所需私有类型。不得通过生产可见性扩大来满足测试。

- [x] **Step 3: 统一加固所有 API 响应**

在 API 子模块添加：

```rust
async fn harden_api_response(mut response: Response) -> Response {
    response.headers_mut().insert(
        header::CACHE_CONTROL,
        HeaderValue::from_static("no-store"),
    );
    response.headers_mut().insert(
        header::X_CONTENT_TYPE_OPTIONS,
        HeaderValue::from_static("nosniff"),
    );
    response
}
```

将它只应用到 `/api/v1` router：

```rust
let api = Router::new()
    // existing routes and body limit
    .layer(middleware::map_response(harden_api_response));
```

移动页面和本地 HEIC 脚本继续使用各自已有缓存策略。

- [x] **Step 4: 运行定向回归**

Run:

```powershell
.\scripts\cargo-msvc.cmd test --manifest-path src-tauri\Cargo.toml --lib authenticated_api_responses_are_never_cacheable
```

Expected: PASS，成功和未授权响应均为 `Cache-Control: no-store`。

- [x] **Step 5: 运行完整 LAN 模块测试**

Run:

```powershell
.\scripts\cargo-msvc.cmd test --manifest-path src-tauri\Cargo.toml --lib capture_lan_api::tests
```

Expected: 11 项测试全部通过。

---

### Task 3: 静态门禁与审查

**Files:**
- Verify: `src-tauri/src/modules/capture_lan.rs`
- Verify: `src-tauri/src/modules/capture_lan_api.rs`
- Verify: `src-tauri/src/modules/capture_lan_api_tests.rs`
- Verify: `src-tauri/src/commands/capture_lan.rs`
- Verify: `src-tauri/src/infrastructure/recognition_visual_split.rs`

**Interfaces:**
- Preserves: `CaptureLanManager`、`CaptureLanContext`、`CaptureLanSession`、`CaptureLanAddress`、`CaptureLanError` 的原公开路径。
- Preserves: `/mobile/`、`/mobile/vendor/heic2any.js`、`/api/v1/session`、上传、预览、裁剪、恢复、删除和完成路径。

- [x] **Step 1: 运行架构、格式和 Clippy**

Run:

```powershell
pnpm contract:rust-boundaries
.\scripts\cargo-msvc.cmd fmt --manifest-path src-tauri\Cargo.toml --all -- --check
.\scripts\cargo-msvc.cmd clippy --manifest-path src-tauri\Cargo.toml --all-targets --all-features -- -D warnings
```

Expected: 全部通过。

- [x] **Step 2: 本地代码复核**

确认父模块不再包含 HTTP route、handler、payload、授权或 `ApiError`；API 子模块保持私有；API 测试没有迫使扩大生产可见性；所有认证成功/错误响应经同一 middleware 添加 `no-store`；命令、bindings、schema、依赖、移动页面和 OCR 文件未因本批修改。

- [x] **Step 3: 运行最终差异检查**

Run:

```powershell
git diff --check
git diff --cached --quiet
```

Expected: 无空白错误，暂存区为空。

- [x] **Step 4: 记录验证结果**

在本文追加基线、红灯、绿灯、完整测试、静态门禁、模块行数、审查结果、暂存区状态和范围排除。

## Self-Review

- 需求覆盖：API 职责拆分、成功与错误响应禁缓存、协议兼容、测试隔离和静态门禁均有明确步骤。
- Placeholder scan：无 TBD、TODO、未定义函数或“稍后补充”步骤。
- 类型一致性：父模块持有 `ServerState`，API 只暴露 `pub(super) build_router`；测试是 API 子模块后代，不需要扩大其他生产符号可见性。

## Verification Record

- 基线：拆分前 `capture_lan::tests` 共 10 项，全部通过。
- 行为红灯：`authenticated_api_responses_are_never_cacheable` 首次失败，认证成功响应的 `Cache-Control` 实际为 `None`。
- 架构红灯：子模块创建前 `pnpm contract:rust-boundaries` 因缺少 `capture_lan_api.rs` 按预期失败。
- 实现：整个 `/api/v1` router 统一经过 `harden_api_response`，所有成功与错误响应写入 `Cache-Control: no-store` 和 `X-Content-Type-Options: nosniff`；移动页面与 HEIC 脚本继续使用原策略。
- 边界校正：首次编译指出机械切片带走了地址选择、批次序号和会话临时目录 helper；这些管理器职责已原样移回父模块，API 子模块只保留路由、handler、payload、授权和错误映射。
- 定向绿灯：成功响应和无效 token 响应的禁缓存断言均通过。
- 完整 LAN 测试：11 项通过，覆盖私网地址、接口选择、常量时间 token、Host/Origin/过期校验、媒体类型、禁缓存、页面安全头、幂等上传、会话恢复与预览、无损裁剪/恢复、完成和监听端口关闭。
- 静态门禁：Rust 架构边界契约、`cargo fmt --check`、全目标全特性 Clippy（`-D warnings`）均通过。
- 最终模块规模：会话管理父模块 504 行，HTTP API 生产模块 761 行，API 测试模块 757 行；原单文件为 1945 行。
- 本地审查：八条既有 API route 和 JSON 协议保持不变；`no-store` middleware 覆盖整个嵌套 API；子模块私有且只向父模块暴露 `pub(super) build_router`；未发现 Critical、Important 或 Minor 问题。
- 工作区：`git diff --check` 通过，暂存区为空。本批未修改 command、bindings、schema、依赖、移动页面或 `recognition_visual_split.rs`；这些路径显示的既有修改均保留。
- Windows 环境仍输出既有 OpenSSL 静态 PDB 与 SQLCipher `VirtualLock` 警告，不影响本轮命令退出码。
