# Real Training Dashboard Implementation Plan

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:subagent-driven-development (recommended) or superpowers:executing-plans to implement this plan task-by-task. Steps use checkbox (`- [ ]`) syntax for tracking.

**Goal:** Replace every hard-coded training-desk number with profile-scoped encrypted-library data and deliver truthful empty, ready, completed, loading, and failure states with restrained responsive motion.

**Architecture:** Add a small `dashboard_overview` read model in the existing Rust insights module. The Vue route calls one generated command and passes a typed DTO into a presentational dashboard; a reusable animated-number composable provides reduced-motion-aware numeric transitions without putting business state in the component.

**Tech Stack:** Rust, rusqlite/SQLCipher, Tauri Specta, Vue 3, TypeScript strict, Vitest, Testing Library.

## Global Constraints

- Windows v1 remains offline-first; the dashboard must work without Supabase.
- Every query must scope by both `account_id` and `profile_id`.
- Stored timestamps remain UTC; day buckets use a validated client UTC offset from `-840` through `840` minutes.
- UI motion uses transform and opacity, finishes within 420 ms, and stops under `prefers-reduced-motion: reduce`.
- No dashboard value may be substituted with demo or fallback statistics after a command failure.

---

### Task 1: Dashboard read model

**Files:**
- Modify: `src-tauri/src/modules/insights.rs`
- Modify: `src-tauri/tests/insights_store.rs`

**Interfaces:**
- Produces: `dashboard_overview(connection, account_id, profile_id, now_utc_ms, utc_offset_minutes) -> Result<DashboardOverview, InsightsError>`.
- Produces fields: profile name, active/due counts, reviews today, optional 30-day remembered rate, current streak, unfinished capture batch count, and unfinished capture item count.

- [ ] **Step 1: Write failing integration tests**

Create events on today, yesterday, 29 days ago, and 31 days ago; add data for another profile; add collecting/organizing/completed capture batches. Assert exact scoping, 30-day rate, local-day streak, due count, and unfinished capture counts. Add an invalid UTC-offset assertion.

- [ ] **Step 2: Run the focused test and verify failure**

Run: `.\scripts\cargo-msvc.cmd test --manifest-path src-tauri\Cargo.toml --test insights_store`

Expected: compile failure because `dashboard_overview` and `DashboardOverview` do not exist.

- [ ] **Step 3: Implement the read model**

Add:

```rust
pub struct DashboardOverview {
    pub profile_name: String,
    pub active_problem_count: i32,
    pub due_problem_count: i32,
    pub reviewed_today_count: i32,
    pub remembered_rate_30_days: Option<f64>,
    pub current_streak_days: i32,
    pub pending_capture_batch_count: i32,
    pub pending_capture_item_count: i32,
}
```

Use `now_utc_ms + offset_ms` for positive local-day buckets, return `None` when the 30-day denominator is zero, and reject offsets outside `-840..=840`.

- [ ] **Step 4: Run the focused test and verify pass**

Run: `.\scripts\cargo-msvc.cmd test --manifest-path src-tauri\Cargo.toml --test insights_store`

Expected: all `insights_store` tests pass.

### Task 2: Typed Tauri command

**Files:**
- Modify: `src-tauri/src/commands/insights.rs`
- Modify: `src-tauri/src/bindings.rs`
- Modify: `src/shared/api/bindings.test.ts`
- Generate: `src/shared/api/bindings.ts`

**Interfaces:**
- Produces: `dashboard_overview(utc_offset_minutes: i32) -> AppResult<DashboardOverview>`.

- [ ] **Step 1: Register the command and error mapping**

Call the read model with the runtime account/profile and current UTC time. Map invalid offsets to non-retryable `dashboard_timezone_invalid`; map database failures to retryable `dashboard_overview_failed`.

- [ ] **Step 2: Add a binding contract assertion**

Assert the generated file contains `dashboardOverview`, `DashboardOverview`, and `rememberedRate30Days`.

- [ ] **Step 3: Generate bindings**

Run: `corepack pnpm bindings:generate`

Expected: the typed client exposes `commands.dashboardOverview(utcOffsetMinutes)`.

### Task 3: Reduced-motion animated numbers

**Files:**
- Create: `src/shared/composables/useAnimatedNumber.ts`
- Create: `src/shared/composables/useAnimatedNumber.test.ts`

**Interfaces:**
- Produces: `useAnimatedNumber(source: MaybeRefOrGetter<number>, durationMs?: number): Readonly<Ref<number>>`.

- [ ] **Step 1: Write failing component-harness tests**

Assert a changed source reaches the rounded target after animation frames, cancels superseded frames, and updates immediately when reduced motion is active.

- [ ] **Step 2: Implement the composable**

Animate from the displayed value using `requestAnimationFrame` and cubic ease-out. Clamp non-finite inputs to zero, cancel on unmount, and subscribe to the reduced-motion media query.

- [ ] **Step 3: Run focused tests**

Run: `corepack pnpm test -- src/shared/composables/useAnimatedNumber.test.ts`

Expected: all animated-number tests pass.

### Task 4: Truthful dashboard presentation

**Files:**
- Modify: `src/modules/dashboard/components/TrainingDashboard.vue`
- Modify: `src/modules/dashboard/components/TrainingDashboard.test.ts`

**Interfaces:**
- Consumes: `DashboardOverview | undefined`, `loading`, and `errorMessage`.
- Emits: `start-review`, `open-inbox`, `open-library`, `open-report`, and `retry`.

- [ ] **Step 1: Add state and action tests**

Cover loading skeleton, retryable failure, no-problem onboarding, due-review CTA, all-clear CTA, null retention label, real pending-image count, report action, and reduced-motion-safe number rendering.

- [ ] **Step 2: Replace hard-coded copy and metrics**

Render only DTO values. Use `录入第一道错题`, `开始复习 N 道`, or `查看题库` according to state. Replace `4 组待配对图片` with the real unfinished batch/image counts.

- [ ] **Step 3: Add meaningful motion**

Animate only data reveal, numeric transitions, CTA arrow movement, hover lift, and the all-clear seal. Add a reduced-motion media rule that removes every transition and animation.

- [ ] **Step 4: Run focused component tests**

Run: `corepack pnpm test -- src/modules/dashboard/components/TrainingDashboard.test.ts`

Expected: all dashboard component tests pass.

### Task 5: Route integration

**Files:**
- Modify: `src/app/views/DashboardView.vue`
- Create: `src/app/views/DashboardView.test.ts`

**Interfaces:**
- Consumes: `commands.dashboardOverview(-new Date().getTimezoneOffset())`.

- [ ] **Step 1: Write the failing view test**

Mock the typed command, assert the current timezone offset is passed, verify successful data reaches the component, and verify retry invokes a second request.

- [ ] **Step 2: Implement loading/error/success orchestration**

Keep prior data visible only during an explicit refresh, never invent values, and wire actions to `review`, `inbox`, `library`, and `report` routes.

- [ ] **Step 3: Run focused view and app navigation tests**

Run: `corepack pnpm test -- src/app/views/DashboardView.test.ts src/app/App.test.ts`

Expected: route integration and sidebar cycling pass.

### Task 6: Documentation and quality gates

**Files:**
- Modify: `docs/architecture.md`
- Modify: `docs/plans/foundation.md`

- [ ] **Step 1: Correct stale architecture text**

Document that LAN capture installs one persistent, app-scoped Windows Firewall rule only after explicit elevation, instead of claiming the app never modifies the firewall.

- [ ] **Step 2: Record the real-dashboard invariant**

State that dashboard statistics are local read models and that failure states never display demo values.

- [ ] **Step 3: Run all gates**

Run: `corepack pnpm lint`, `corepack pnpm typecheck`, `corepack pnpm test`, `.\scripts\cargo-msvc.cmd test --manifest-path src-tauri\Cargo.toml --all-targets --quiet`, `corepack pnpm bindings:check`, and `corepack pnpm tauri build`.

Expected: every command exits zero; Windows release executable is produced.

- [ ] **Step 4: Review and commit**

Review the complete diff against this plan, then commit with `feat: connect the real training dashboard` and push `feature/capture-library` after the quality gates pass.
