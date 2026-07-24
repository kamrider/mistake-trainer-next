# Background Sync Experience Implementation Plan

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:subagent-driven-development (recommended) or superpowers:executing-plans to implement this plan task-by-task. Steps use checkbox (`- [ ]`) syntax for tracking.

**Goal:** 让 Windows 应用在资料库解锁、网络恢复或重新回到前台时安全地恢复云端会话并同步，同时保证后台同步永远不会关闭正在进行的手机采集。

**Architecture:** Rust 使用进程级 `SyncCoordinator` 发放唯一同步许可，并在持有资料库的 profile-transition lock 后检查 LAN 采集状态；采集中返回稳定的可重试错误而不停止会话。Vue 使用一个小型、可注入的同步控制器串行化自动与手动触发，只监听启动、`online` 和重新可见三个有限事件，不使用常驻轮询；侧栏只展示真实的本地保存、同步中、已同步、延后或需要重试状态。

**Tech Stack:** Rust stable、Tauri 2、Vue 3 Composition API、TypeScript strict、Vitest、Testing Library、tauri-specta。

## Global Constraints

- Windows 是唯一 v1 发布平台；浏览器预览不得调用原生命令。
- 本地写入始终优先且不因云端失败回滚。
- 手机采集会话不得被后台或手动同步隐式停止。
- 同时最多运行一个真实同步任务；重复触发复用前端 Promise 或由 Rust 返回稳定错误。
- 不新增定时轮询、系统托盘常驻任务、Realtime 或新的数据库迁移。
- 普通动效只使用 `transform` 和 `opacity`，并尊重 `prefers-reduced-motion`。
- 所有公共命令继续返回 `AppResult<T>`；令牌、路径和数据库句柄不进入 Vue 状态。

---

## File Structure

- Create `src-tauri/src/modules/sync_coordinator.rs`: 进程级唯一同步许可及 RAII 释放。
- Modify `src-tauri/src/modules/mod.rs`: 导出同步协调模块。
- Modify `src-tauri/src/lib.rs`: 注册一个应用级 `SyncCoordinator`。
- Modify `src-tauri/src/commands/sync.rs`: 获取同步许可；持有 profile-transition lock 时检查而不是关闭 LAN 会话。
- Create `src/app/sync-controller.ts`: Vue 可注入接口、状态类型和纯状态文案映射。
- Create `src/app/sync-controller.test.ts`: 状态文案与触发串行化的单元契约。
- Modify `src/app/App.vue`: 恢复会话、有限事件触发、全局同步状态和成功后的工作区刷新。
- Modify `src/app/App.profile.test.ts`: 桌面启动、离线、采集延后、重复触发和恢复网络测试。
- Modify `src/app/AppShell.vue`: 侧栏同步状态的语义 class 与克制过渡。
- Modify `src/app/AppShell.test.ts`: 状态语义和无障碍播报测试。
- Modify `src/app/views/SettingsView.vue`: 手动同步优先调用注入的全局控制器；无宿主时保留命令回退。
- Modify `src/app/views/SettingsView.test.ts`: 手动同步委托和登录后同步测试。

### Task 1: Rust 同步唯一许可

**Files:**
- Create: `src-tauri/src/modules/sync_coordinator.rs`
- Modify: `src-tauri/src/modules/mod.rs`
- Modify: `src-tauri/src/lib.rs`

**Interfaces:**
- Produces: `SyncCoordinator::try_begin(&self) -> Option<SyncPermit>`
- Produces: `SyncCoordinator::is_running(&self) -> bool`
- Guarantees: `SyncPermit` 在成功、错误或 panic unwind 路径 drop 时释放许可。

- [ ] **Step 1: Write the failing unit tests**

```rust
#[test]
fn only_one_sync_permit_can_exist() {
    let coordinator = SyncCoordinator::default();
    let permit = coordinator.try_begin().expect("first permit");
    assert!(coordinator.is_running());
    assert!(coordinator.try_begin().is_none());
    drop(permit);
    assert!(!coordinator.is_running());
}

#[test]
fn cloned_coordinators_share_the_same_permit() {
    let coordinator = SyncCoordinator::default();
    let clone = coordinator.clone();
    let _permit = clone.try_begin().expect("shared permit");
    assert!(coordinator.try_begin().is_none());
}
```

- [ ] **Step 2: Run the focused test and verify it fails**

Run: `.\scripts\cargo-msvc.cmd test --manifest-path src-tauri\Cargo.toml sync_coordinator`

Expected: FAIL because `modules::sync_coordinator` does not exist.

- [ ] **Step 3: Implement the RAII coordinator**

```rust
#[derive(Clone, Default)]
pub struct SyncCoordinator {
    running: Arc<AtomicBool>,
}

impl SyncCoordinator {
    pub fn try_begin(&self) -> Option<SyncPermit> {
        self.running
            .compare_exchange(false, true, Ordering::AcqRel, Ordering::Acquire)
            .ok()
            .map(|_| SyncPermit { running: Arc::clone(&self.running) })
    }
}

impl Drop for SyncPermit {
    fn drop(&mut self) {
        self.running.store(false, Ordering::Release);
    }
}
```

Register exactly one instance with `app.manage(modules::sync_coordinator::SyncCoordinator::default())`.

- [ ] **Step 4: Run the focused test and verify it passes**

Run: `.\scripts\cargo-msvc.cmd test --manifest-path src-tauri\Cargo.toml sync_coordinator`

Expected: PASS for both permit tests.

- [ ] **Step 5: Commit**

```powershell
git add src-tauri/src/modules/sync_coordinator.rs src-tauri/src/modules/mod.rs src-tauri/src/lib.rs
git commit -m "feat: serialize cloud sync jobs"
```

### Task 2: 手机采集期间延后同步

**Files:**
- Modify: `src-tauri/src/commands/sync.rs`

**Interfaces:**
- Consumes: `SyncCoordinator::try_begin()`
- Produces: `SYNC_ALREADY_RUNNING` with retryable `true`.
- Produces: `SYNC_CAPTURE_ACTIVE` with retryable `true`.
- Guarantees: `sync_now` never calls `CaptureLanManager::stop()`.

- [ ] **Step 1: Write failing command-policy tests**

Extract two side-effect-free helpers and test their exact policy:

```rust
#[test]
fn active_capture_defers_sync_without_stopping_it() {
    assert_eq!(
        sync_admission(true, true),
        Err(SyncAdmissionError::CaptureActive)
    );
}

#[test]
fn a_running_sync_rejects_a_duplicate() {
    assert_eq!(
        sync_admission(false, false),
        Err(SyncAdmissionError::AlreadyRunning)
    );
}
```

The first boolean is “permit acquired”; the second is “LAN session active”. The helper must not own or mutate `CaptureLanManager`.

- [ ] **Step 2: Run the focused tests and verify they fail**

Run: `.\scripts\cargo-msvc.cmd test --manifest-path src-tauri\Cargo.toml commands::sync::tests`

Expected: FAIL because `SyncAdmissionError` and `sync_admission` do not exist.

- [ ] **Step 3: Implement admission in the real command**

Add `coordinator: State<'_, SyncCoordinator>` to `sync_now`. After auth/session checks:

```rust
let Some(_permit) = coordinator.try_begin() else {
    return Ok(AppResult::failure(
        "SYNC_ALREADY_RUNNING",
        "同步已经在进行，请稍候。",
        true,
        "sync-already-running",
    ));
};
```

Inside the blocking worker, acquire `profile_transition` first, then:

```rust
if capture_lan
    .status(current_utc_millis())
    .map_err(|_| "sync_capture_status_failed")?
    .is_some()
{
    return Err("sync_capture_active");
}
```

Map `sync_capture_active` to public code `SYNC_CAPTURE_ACTIVE` and copy `手机采集正在进行；结束拍摄后会自动继续同步，当前上传不会被打断。`. Preserve the existing stable cloud error mapping for all other failures.

- [ ] **Step 4: Verify Rust and binding contracts**

Run:

```powershell
.\scripts\cargo-msvc.cmd test --manifest-path src-tauri\Cargo.toml commands::sync::tests
pnpm bindings:check
```

Expected: PASS and no generated binding drift except the added internal Tauri state parameter, which is not exposed to TypeScript.

- [ ] **Step 5: Commit**

```powershell
git add src-tauri/src/commands/sync.rs
git commit -m "fix: defer sync during phone capture"
```

### Task 3: 有限事件驱动的全局同步控制器

**Files:**
- Create: `src/app/sync-controller.ts`
- Create: `src/app/sync-controller.test.ts`
- Modify: `src/app/App.vue`
- Modify: `src/app/App.profile.test.ts`

**Interfaces:**
- Produces: `SyncPhase = 'local_only' | 'signed_out' | 'offline' | 'idle' | 'syncing' | 'synced' | 'deferred_capture' | 'retry_waiting'`
- Produces: `SyncController.run(reason: 'startup' | 'online' | 'visible' | 'manual') -> Promise<AppResult<SyncNowReport>>`
- Produces: `syncControllerKey: InjectionKey<SyncController>`
- Guarantees: 同期触发复用同一个 Promise；自动失败不弹阻塞提示。

- [ ] **Step 1: Write failing controller contract tests**

```ts
it('maps every phase to truthful local-first copy', () => {
  expect(syncStatusCopy('syncing')).toEqual({ label: '正在安全同步', tone: 'active' })
  expect(syncStatusCopy('deferred_capture')).toEqual({ label: '手机采集中 · 稍后同步', tone: 'waiting' })
  expect(syncStatusCopy('retry_waiting')).toEqual({ label: '本地已保存 · 等待重试', tone: 'warning' })
})

it('coalesces concurrent triggers into one command', async () => {
  const first = controller.run('online')
  const second = controller.run('visible')
  expect(invokeSync).toHaveBeenCalledOnce()
  expect(await first).toEqual(await second)
})
```

- [ ] **Step 2: Run the focused frontend tests and verify they fail**

Run: `pnpm vitest run src/app/sync-controller.test.ts src/app/App.profile.test.ts`

Expected: FAIL because the controller module and startup behavior do not exist.

- [ ] **Step 3: Implement the controller and App orchestration**

`App.vue` must:

1. Provide the controller before route views mount.
2. After an unlocked workspace initializes, call `authRestore()`.
3. Only call `syncNow()` when restored status is `connected`.
4. Treat `offline` and `AUTH_NETWORK` as `offline`.
5. Treat `SYNC_CAPTURE_ACTIVE` as `deferred_capture`.
6. On success set `synced`, reload profiles and increment `profileEpoch`.
7. Listen for `window.online`.
8. Listen for `document.visibilitychange` only when `document.visibilityState === 'visible'`.
9. Remove both listeners on unmount.
10. Never install an interval.

The controller keeps one in-flight promise:

```ts
let inFlight: Promise<AppResult<SyncNowReport>> | undefined
async function run(reason: SyncTrigger) {
  if (inFlight) return inFlight
  inFlight = performSync(reason).finally(() => { inFlight = undefined })
  return inFlight
}
```

Automatic triggers first restore auth so expired access tokens can refresh. Browser preview remains `local_only`.

- [ ] **Step 4: Run focused tests and verify they pass**

Run: `pnpm vitest run src/app/sync-controller.test.ts src/app/App.profile.test.ts`

Expected: PASS for startup connected/offline, duplicate trigger, deferred capture and successful refresh cases.

- [ ] **Step 5: Commit**

```powershell
git add src/app/sync-controller.ts src/app/sync-controller.test.ts src/app/App.vue src/app/App.profile.test.ts
git commit -m "feat: sync after startup and network recovery"
```

### Task 4: 统一手动同步与全局状态反馈

**Files:**
- Modify: `src/app/AppShell.vue`
- Modify: `src/app/AppShell.test.ts`
- Modify: `src/app/views/SettingsView.vue`
- Modify: `src/app/views/SettingsView.test.ts`

**Interfaces:**
- Consumes: `SyncController.run('manual')`
- Consumes: `{ label, tone }` from `syncStatusCopy`.
- Guarantees: Settings 单独渲染时仍回退到 `commands.syncNow()`，便于组件隔离和测试。

- [ ] **Step 1: Write failing UI tests**

```ts
it('announces a deferred capture without calling the native command twice', async () => {
  const run = vi.fn().mockResolvedValue(failure(
    'SYNC_CAPTURE_ACTIVE',
    '手机采集正在进行；结束拍摄后会自动继续同步，当前上传不会被打断。',
    true,
    'capture-active',
  ))
  // provide syncControllerKey, click “立即同步”
  expect(run).toHaveBeenCalledWith('manual')
  expect(api.syncNow).not.toHaveBeenCalled()
})

it('renders the active sync tone with polite live status', () => {
  // render AppShell with syncStatus { label: '正在安全同步', tone: 'active' }
  expect(screen.getByRole('status')).toHaveAttribute('data-tone', 'active')
})
```

- [ ] **Step 2: Run focused UI tests and verify they fail**

Run: `pnpm vitest run src/app/AppShell.test.ts src/app/views/SettingsView.test.ts`

Expected: FAIL because AppShell has only a string prop and Settings calls `commands.syncNow()` directly.

- [ ] **Step 3: Implement unified feedback**

Replace AppShell’s `systemStatus: string` with:

```ts
syncStatus: {
  label: string
  tone: 'neutral' | 'active' | 'success' | 'waiting' | 'warning'
}
```

Apply `data-tone`, keep `aria-live="polite"` and `role="status"`. Animate only the icon/text opacity and a single icon scale during transitions; remove animation under reduced motion.

In Settings:

```ts
const globalSync = inject(syncControllerKey, undefined)
const result = globalSync
  ? await globalSync.run('manual')
  : normalizeNativeSync(await commands.syncNow())
```

Do not call `load()` after the global controller already refreshed the workspace; only reload Settings overview/conflicts. Update login success copy to say the first sync starts automatically.

- [ ] **Step 4: Run all frontend quality gates**

Run:

```powershell
pnpm lint
pnpm typecheck
pnpm test
pnpm build
```

Expected: zero lint warnings, zero type errors, all tests pass, production build succeeds.

- [ ] **Step 5: Run Rust and Windows acceptance gates**

Run:

```powershell
.\scripts\cargo-msvc.cmd test --manifest-path src-tauri\Cargo.toml --all-targets
pnpm bindings:check
pnpm tauri build
```

Expected: all Rust targets pass, bindings are clean, and the NSIS installer is produced. SQLCipher `VirtualLock` warning 1453 and OpenSSL PDB LNK4099 remain non-fatal environment warnings if they appear.

- [ ] **Step 6: Commit**

```powershell
git add src/app/AppShell.vue src/app/AppShell.test.ts src/app/views/SettingsView.vue src/app/views/SettingsView.test.ts
git commit -m "feat: show truthful global sync progress"
```

## Self-Review

- Spec coverage: startup, online and visible triggers are covered; capture protection, duplicate prevention, local-first failure, manual delegation, cleanup and reduced motion each have an implementation task.
- Deliberately excluded: periodic polling, Realtime, tray/background service, sync-history UI and database migration. None are required for this closed-loop milestone.
- Placeholder scan: no TBD/TODO or unspecified error handling remains.
- Type consistency: `SyncController.run`, `SyncPhase`, `syncControllerKey` and the AppShell `syncStatus` prop use the same names across Tasks 3–4.

