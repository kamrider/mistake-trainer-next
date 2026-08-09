# Review History List Transaction Implementation Plan

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:executing-plans to implement this plan task-by-task. Steps use checkbox (`- [ ]`) syntax for tracking.

**Goal:** 让复习历史筛选替换、游标分页和精确重试共享一个请求事务，避免旧 cursor 与新筛选组合后覆盖正确结果。

**Architecture:** 新建 `useReviewHistoryList` 管理列表快照、当前查询、请求 epoch、分页状态与失败重试上下文；页面只处理详情选择和筛选事件。`ReviewHistoryTimeline` 接收 `disabled`，在第一页替换请求期间保留旧内容作为上下文但禁止打开详情或加载旧 cursor。

**Tech Stack:** Vue 3 Composition API、TypeScript、Vitest、Testing Library、Tauri 生成绑定 `ReviewHistoryInput`/`ReviewHistoryPage`。

## Global Constraints

- 不处理许可证、隐私、客服、账户删除、设备迁移、更新失败恢复或支持 SLA 等上线前事项。
- 不新增依赖，不修改生成绑定、Rust/OCR 文件，不暂存、不提交。
- 使用 TDD；只有最新请求可提交列表状态，应用错误或异常不得清空最后一次成功快照。
- 替换加载期间旧时间线只用于视觉上下文，所有旧详情和旧 cursor 操作必须禁用。

---

### Task 1: 列表控制器失败测试

**Files:**
- Create: `src/modules/review-history/composables/useReviewHistoryList.test.ts`
- Create: `src/modules/review-history/composables/useReviewHistoryList.ts`

**Interfaces:**
- Consumes: `AppResult<ReviewHistoryPage>`、`Omit<ReviewHistoryInput, 'cursor' | 'limit'>`。
- Produces: `useReviewHistoryList(options)` 的请求与状态契约。

- [x] **Step 1: 建立测试 harness**

```ts
const listPage = vi.fn(async (_input: ReviewHistoryInput) => success(emptyPage))
const controller = useReviewHistoryList({ listPage, initialQuery, pageSize: 20 })
```

测试夹具包含第一页 `event-1`、第二页重复的 `event-1` 与新记录 `event-2`，以及两个互斥筛选查询。

- [x] **Step 2: 写最新筛选请求获胜测试**

启动两个 deferred `replace()`；后发请求先成功后，让旧请求成功或抛错。断言只有后发查询的 items、subjects、nextCursor、totalCount 可提交，旧响应不能修改 error/loading。

- [x] **Step 3: 写替换期间分页阻断测试**

先成功加载带 `cursor-old` 的第一页，再启动新筛选 deferred；此时调用 `loadMore()`，断言返回 false 且 `listPage` 没有收到第三次请求。新筛选完成后只能使用新响应的 cursor。

- [x] **Step 4: 写分页去重与精确重试测试**

追加页包含重复 event id 时只保留首个现有项并追加新项。应用错误和异常保留列表、设置 stale，并保存完整 query/cursor/append；`retry()` 必须用完全相同的 `ReviewHistoryInput` 重试。

- [x] **Step 5: 写初始失败与空结果测试**

当前应用错误和异常返回 false、暴露稳定错误；成功空页设置 loaded 状态、totalCount 0、stale false。无 cursor 或已有分页请求时 `loadMore()` 返回 false。

- [x] **Step 6: 运行测试确认失败**

Run: `pnpm exec vitest run src/modules/review-history/composables/useReviewHistoryList.test.ts`

Expected: FAIL，因为控制器文件尚不存在。

---

### Task 2: 实现复习历史列表事务

**Files:**
- Create: `src/modules/review-history/composables/useReviewHistoryList.ts`
- Test: `src/modules/review-history/composables/useReviewHistoryList.test.ts`

**Interfaces:**
- Consumes:

```ts
interface ReviewHistoryListOptions {
  listPage: (input: ReviewHistoryInput) => Promise<AppResult<ReviewHistoryPage>>
  initialQuery: ReviewHistoryQuery
  pageSize: number
}
type ReviewHistoryQuery = Omit<ReviewHistoryInput, 'cursor' | 'limit'>
```

- Produces:

```ts
items: ShallowReadonly<ShallowRef<ReviewHistoryItem[]>>
subjects: ShallowReadonly<ShallowRef<string[]>>
nextCursor: Readonly<Ref<string | null>>
totalCount: Readonly<Ref<number>>
loading: Readonly<Ref<boolean>>
loadingMore: Readonly<Ref<boolean>>
loaded: Readonly<Ref<boolean>>
errorMessage: Readonly<Ref<string>>
stale: Readonly<Ref<boolean>>
replace(query: ReviewHistoryQuery): Promise<boolean>
loadMore(): Promise<boolean>
retry(): Promise<boolean>
```

- [x] **Step 1: 实现请求快照和 latest-start-wins**

每个请求构造不可变 `ReviewHistoryInput`，递增 epoch；只有 epoch 等于当前值的响应、错误和 finally 可提交。`replace()` 更新当前 query、使用 cursor null，并在请求开始时清除旧失败上下文。

- [x] **Step 2: 实现替换和分页互斥**

`loadMore()` 在 `loading || loadingMore || !nextCursor` 时返回 false。替换成功原子替换列表；追加成功用 eventId Set 去重，现有项保持顺序，新项按服务端顺序追加。

- [x] **Step 3: 实现失败和重试语义**

应用错误使用服务端 userMessage；异常使用 `复习历史暂时无法读取，请稍后重试。`。失败保留 items，并把 stale 设置为 items 非空。失败上下文保存完整 input 与 append；`retry()` 原样重发该 input，没有失败上下文时重载当前查询第一页。

- [x] **Step 4: 暴露只读状态**

items 和 subjects 数组 ref 使用 `shallowReadonly`，其他状态使用 `readonly`；外部不得直接写 items、cursor 或 busy 状态。

- [x] **Step 5: 运行测试与定向覆盖率**

Run: `pnpm exec vitest run src/modules/review-history/composables/useReviewHistoryList.test.ts`

Run: `pnpm exec vitest run src/modules/review-history/composables/useReviewHistoryList.test.ts --coverage --coverage.include=src/modules/review-history/composables/useReviewHistoryList.ts`

Expected: PASS，statements、branches、functions、lines 均为 100%。

---

### Task 3: 页面和时间线接入

**Files:**
- Modify: `src/app/views/ReviewHistoryView.vue`
- Modify: `src/app/views/ReviewHistoryView.resilience.test.ts`
- Modify: `src/modules/review-history/components/ReviewHistoryTimeline.vue`
- Create: `src/modules/review-history/components/ReviewHistoryTimeline.test.ts`

**Interfaces:**
- Consumes: Task 2 的只读状态和 `replace/loadMore/retry`。
- Produces: `ReviewHistoryTimeline.disabled: boolean` 原生禁用契约。

- [x] **Step 1: 写筛选期间旧 cursor 禁用回归**

在 `ReviewHistoryView.resilience.test.ts` 先返回含 next cursor 的旧列表，再让新筛选第一页 deferred。提交筛选后断言旧记录按钮和“加载更多”均为 disabled；点击不会请求旧 cursor。新请求完成后只显示新筛选记录。

- [x] **Step 2: 写时间线组件禁用测试**

渲染 `ReviewHistoryTimeline` 并传 `disabled: true`，断言所有历史行和加载更多按钮均有原生 `disabled`，点击不发出 select/more；`aria-busy` 为 true。传 false 后事件恢复。

- [x] **Step 3: 接入列表控制器**

从 `ReviewHistoryView.vue` 删除 items/subjects/cursor/count/list loading/error/stale/failedRequest/listEpoch 和 `requestPage()`。控制器的 `listPage` 在 preview 模式返回 `previewPage()`，桌面模式规范化 `commands.reviewHistoryList(input)`。

- [x] **Step 4: 收紧页面交互**

`applyFilters/resetFilters` 先关闭详情再调用 `replace()`；`selectDetail` 在 loading 时直接返回；时间线传 `:disabled="loading"`。重试按钮调用 `retry()`，加载更多事件直接调用控制器 `loadMore()`。

- [x] **Step 5: 运行页面、组件和类型检查**

Run: `pnpm exec vitest run src/modules/review-history/composables/useReviewHistoryList.test.ts src/modules/review-history/components/ReviewHistoryTimeline.test.ts src/app/views/ReviewHistoryView.test.ts src/app/views/ReviewHistoryView.resilience.test.ts`

Run: `pnpm typecheck`

Expected: PASS，现有初始错误、精确分页重试、详情竞态和焦点恢复均不回退。

---

### Task 4: 全量门禁与复核

**Files:**
- Verify: `docs/superpowers/plans/2026-07-30-review-history-list-transaction.md`

- [x] **Step 1: 运行完整门禁**

Run: `pnpm exec vitest run --coverage`

Run: `pnpm lint`

Run: `pnpm typecheck`

Run: `pnpm build`

Expected: 全部退出码 0，无新增 bundle warning。

- [x] **Step 2: 本地代码复核**

按计划逐项检查 query/cursor 是否来自同一快照、替换与分页是否互斥、append 是否去重、失败是否保留旧数据、旧时间线是否真正原生禁用、详情 latest-start-wins 是否保留。

- [x] **Step 3: 修复并复验**

只修改本批文件；运行受影响聚焦测试、`pnpm typecheck` 和 `git diff --check -- src/app/views/ReviewHistoryView.vue src/app/views/ReviewHistoryView.resilience.test.ts src/modules/review-history/components/ReviewHistoryTimeline.vue`，无重要发现后勾选计划。
