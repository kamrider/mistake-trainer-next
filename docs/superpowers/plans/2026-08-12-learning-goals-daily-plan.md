# Learning Goals And Daily Plan Implementation Plan

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:subagent-driven-development (recommended) or superpowers:executing-plans to implement this plan task-by-task. Steps use checkbox (`- [ ]`) syntax for tracking.

**Goal:** Let each learner set daily review and study-time goals and show an actionable daily plan computed from real due cards and today's completed work.

**Architecture:** Extend profile preferences with bounded goal fields and expose typed get/save commands. Add goal and plan fields to the dashboard overview so one read remains a consistent snapshot. The dashboard edits goals inline and recomputes progress only after a successful save.

**Tech Stack:** SQLCipher/SQLite migration 0018, Rust, Tauri Specta bindings, Vue 3, Vitest.

## Global Constraints

- Defaults: 20 reviews per day and 20 minutes per day.
- Valid review target: 1 through 200; valid minute target: 5 through 240.
- Date boundaries use the caller's validated UTC offset and never the machine locale string.
- Goal-save failure must keep the last persisted goal visible.
- Profile deletion cascades goal data; backup/restore includes it through the encrypted database.

---

### Task 1: Persist bounded profile learning goals

**Files:**
- Create: `src-tauri/migrations/0018_learning_goals.sql`
- Modify: `src-tauri/src/infrastructure/database.rs`
- Modify: `src-tauri/src/modules/preferences.rs`
- Modify: `src-tauri/src/commands/preferences.rs`
- Modify: `src-tauri/src/bindings.rs`
- Test: `src-tauri/tests/database_schema.rs`
- Test: `src-tauri/tests/preferences.rs`

**Interfaces:**
- Consumes: active `account_id` and `profile_id`.
- Produces: `LearningGoal { dailyReviewTarget: i32, dailyMinutesTarget: i32 }` and `LearningGoalInput`.

- [ ] **Step 1: Write migration and validation tests**

```rust
assert_eq!(goal.daily_review_target, 20);
assert_eq!(goal.daily_minutes_target, 20);
assert!(save_learning_goal(&connection, account, profile, SaveLearningGoal {
    daily_review_target: 0,
    daily_minutes_target: 20,
}, now).is_err());
```

- [ ] **Step 2: Run tests to verify schema/API are absent**

Run: `.\scripts\cargo-msvc.cmd test --manifest-path src-tauri\Cargo.toml --test database_schema --test preferences`

Expected: FAIL because migration 0018 and goal APIs do not exist.

- [ ] **Step 3: Add the migration**

```sql
ALTER TABLE profile_preferences ADD COLUMN daily_review_target INTEGER NOT NULL DEFAULT 20 CHECK(daily_review_target BETWEEN 1 AND 200);
ALTER TABLE profile_preferences ADD COLUMN daily_minutes_target INTEGER NOT NULL DEFAULT 20 CHECK(daily_minutes_target BETWEEN 5 AND 240);
```

Advance `user_version` from 17 to 18 in one transaction and reject versions newer than 18.

- [ ] **Step 4: Add domain and command types**

```rust
pub struct LearningGoal {
    pub daily_review_target: i32,
    pub daily_minutes_target: i32,
}

pub struct SaveLearningGoal {
    pub daily_review_target: i32,
    pub daily_minutes_target: i32,
}
```

Expose `learning_goal_get` and `learning_goal_save`, map invalid bounds to `learning_goal_invalid`, and preserve the existing row's subject and review preferences during upsert.

- [ ] **Step 5: Run tests and generate bindings**

Run: `.\scripts\cargo-msvc.cmd test --manifest-path src-tauri\Cargo.toml --test database_schema --test preferences`

Expected: PASS.

Run: `pnpm bindings:check`

Expected: bindings contain `learningGoalGet` and `learningGoalSave` and no uncommitted generation diff remains.

- [ ] **Step 6: Commit**

```bash
git add src-tauri/migrations/0018_learning_goals.sql src-tauri/src/infrastructure/database.rs src-tauri/src/modules/preferences.rs src-tauri/src/commands/preferences.rs src-tauri/src/bindings.rs src-tauri/tests/database_schema.rs src-tauri/tests/preferences.rs src/shared/api/bindings.ts
git commit -m "feat: persist learner daily goals"
```

### Task 2: Compute a truthful daily plan snapshot

**Files:**
- Modify: `src-tauri/src/modules/insights.rs`
- Modify: `src-tauri/src/modules/insights_read_repository.rs`
- Modify: `src-tauri/src/commands/insights.rs`
- Test: `src-tauri/tests/insights_store.rs`

**Interfaces:**
- Consumes: learning goal, due schedule rows, and review events inside the caller's local day.
- Produces: `DailyPlanOverview` embedded in `DashboardOverview`.

- [ ] **Step 1: Add calculation tests**

```rust
assert_eq!(overview.daily_plan.review_target, 20);
assert_eq!(overview.daily_plan.completed_reviews, 7);
assert_eq!(overview.daily_plan.remaining_reviews, 13);
assert_eq!(overview.daily_plan.due_reviews, 9);
assert_eq!(overview.daily_plan.suggested_reviews, 13);
assert_eq!(overview.daily_plan.estimated_minutes, 13);
```

Also cover an all-clear day, progress above target, timezone midnight, and zero historical duration.

- [ ] **Step 2: Define the typed snapshot**

```rust
pub struct DailyPlanOverview {
    pub review_target: i32,
    pub minutes_target: i32,
    pub completed_reviews: i32,
    pub remaining_reviews: i32,
    pub due_reviews: i32,
    pub suggested_reviews: i32,
    pub estimated_minutes: i32,
}
```

Set `suggested_reviews = max(due_reviews, remaining_reviews)`. Estimate minutes from the learner's bounded 30-day mean review duration, falling back to one minute per review, and cap the displayed estimate at 240.

- [ ] **Step 3: Run focused insights tests**

Run: `.\scripts\cargo-msvc.cmd test --manifest-path src-tauri\Cargo.toml --test insights_store`

Expected: PASS for all date, target, and duration cases.

- [ ] **Step 4: Regenerate bindings and commit**

```bash
git add src-tauri/src/modules/insights.rs src-tauri/src/modules/insights_read_repository.rs src-tauri/src/commands/insights.rs src-tauri/tests/insights_store.rs src/shared/api/bindings.ts
git commit -m "feat: calculate the learner daily plan"
```

### Task 3: Add dashboard goal editing and plan progress

**Files:**
- Create: `src/modules/dashboard/components/LearningPlanPanel.vue`
- Create: `src/modules/dashboard/components/LearningPlanPanel.test.ts`
- Modify: `src/modules/dashboard/components/TrainingDashboard.vue`
- Modify: `src/modules/dashboard/components/TrainingDashboard.test.ts`
- Modify: `src/app/views/DashboardView.vue`
- Modify: `src/app/views/DashboardView.test.ts`

**Interfaces:**
- Consumes: `DashboardOverview.dailyPlan`, `learningGoalGet`, and `learningGoalSave`.
- Produces: accessible progress copy, inline number fields, save/cancel behavior, and a refreshed dashboard snapshot after save.

- [ ] **Step 1: Write component behavior tests**

```ts
await user.click(screen.getByRole('button', { name: '调整学习目标' }))
await user.clear(screen.getByLabelText('每日复习题数'))
await user.type(screen.getByLabelText('每日复习题数'), '30')
await user.click(screen.getByRole('button', { name: '保存目标' }))
expect(saveGoal).toHaveBeenCalledWith({ dailyReviewTarget: 30, dailyMinutesTarget: 20 })
```

Cover invalid values, save failure, Escape/cancel, keyboard focus, completed target, overdue workload, and reduced motion.

- [ ] **Step 2: Implement the panel**

Render `今日完成 completed/target`, suggested review count, estimated minutes, and an explicit explanation when due work exceeds the configured target. Use native number inputs with `min`, `max`, and server-side validation as the final authority.

- [ ] **Step 3: Refresh after successful save only**

`DashboardView.vue` calls `learningGoalSave(input)`, announces failure without replacing `overview`, and calls `loadOverview()` after success.

- [ ] **Step 4: Run frontend checks**

Run: `pnpm exec vitest run src/modules/dashboard/components/LearningPlanPanel.test.ts src/modules/dashboard/components/TrainingDashboard.test.ts src/app/views/DashboardView.test.ts`

Expected: PASS.

Run: `pnpm lint`

Expected: zero warnings.

Run: `pnpm typecheck`

Expected: exit code 0.

- [ ] **Step 5: Commit**

```bash
git add src/modules/dashboard/components/LearningPlanPanel.vue src/modules/dashboard/components/LearningPlanPanel.test.ts src/modules/dashboard/components/TrainingDashboard.vue src/modules/dashboard/components/TrainingDashboard.test.ts src/app/views/DashboardView.vue src/app/views/DashboardView.test.ts
git commit -m "feat: add learning goals and daily plan"
```
