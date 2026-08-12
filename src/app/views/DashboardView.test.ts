import { render, screen, waitFor } from '@testing-library/vue'
import userEvent from '@testing-library/user-event'
import { createMemoryHistory } from 'vue-router'
import { beforeEach, describe, expect, it, vi } from 'vitest'
import { createAppRouter } from '../router'
import DashboardView from './DashboardView.vue'

const api = vi.hoisted(() => ({ dashboardOverview: vi.fn(), reviewQuickStart: vi.fn(), learningGoalSave: vi.fn() }))

vi.mock('@tauri-apps/api/core', () => ({ isTauri: () => true }))
vi.mock('../../shared/api/bindings', () => ({ commands: api }))

const dashboardData = {
  profileName: '小树',
  activeProblemCount: 20,
  dueProblemCount: 6,
  reviewedTodayCount: 2,
  rememberedRate30Days: 0.8,
  currentStreakDays: 4,
  pendingCaptureBatchCount: 1,
  pendingCaptureItemCount: 9,
  dailyPlan: {
    reviewTarget: 20,
    minutesTarget: 20,
    completedReviews: 2,
    remainingReviews: 18,
    dueReviews: 6,
    suggestedReviews: 18,
    estimatedMinutes: 18,
  },
}

async function renderWithRouter() {
  const router = createAppRouter(createMemoryHistory())
  await router.push('/')
  await router.isReady()
  render(DashboardView, { global: { plugins: [router] } })
  return router
}

describe('DashboardView', () => {
  beforeEach(() => {
    vi.clearAllMocks()
    api.dashboardOverview.mockResolvedValue({ ok: true, data: dashboardData })
    api.reviewQuickStart.mockResolvedValue({
      ok: true,
      data: { sessionId: 'quick-1', mode: 'manual', resumed: false, completedCount: 0, totalCount: 8, items: [] },
    })
    api.learningGoalSave.mockResolvedValue({
      ok: true,
      data: { dailyReviewTarget: 30, dailyMinutesTarget: 25 },
    })
  })

  it('loads the typed local overview with the browser timezone offset', async () => {
    await renderWithRouter()

    expect(await screen.findByText('小树，今天从 6 道到期题开始。')).toBeVisible()
    expect(api.dashboardOverview).toHaveBeenCalledWith(-new Date().getTimezoneOffset())
    expect(screen.getByText('1 个批次 · 9 张图片待整理')).toBeVisible()
  })

  it('shows a truthful failure and retries without displaying stale numbers', async () => {
    const user = userEvent.setup()
    api.dashboardOverview
      .mockResolvedValueOnce({ ok: false, error: { code: 'dashboard_overview_failed', userMessage: '资料库忙碌。', retryable: true, diagnosticId: 'diag-1' } })
      .mockResolvedValueOnce({ ok: true, data: dashboardData })
    await renderWithRouter()

    expect(await screen.findByRole('alert')).toHaveTextContent('资料库忙碌。')
    expect(screen.queryByText('80%')).not.toBeInTheDocument()
    await user.click(screen.getByRole('button', { name: '重新读取' }))
    expect(await screen.findByText('小树，今天从 6 道到期题开始。')).toBeVisible()
    expect(api.dashboardOverview).toHaveBeenCalledTimes(2)
  })

  it('routes an all-clear primary action to the real library', async () => {
    const user = userEvent.setup()
    api.dashboardOverview.mockResolvedValue({ ok: true, data: { ...dashboardData, dueProblemCount: 0 } })
    const router = await renderWithRouter()

    await user.click(await screen.findByRole('button', { name: '查看题库' }))
    await waitFor(() => expect(router.currentRoute.value.name).toBe('library'))
  })

  it('persists a quick session before navigating to review', async () => {
    const user = userEvent.setup()
    const router = await renderWithRouter()
    await user.click(await screen.findByRole('button', { name: '快速训练' }))
    await user.click(screen.getByRole('radio', { name: /十道题专注/ }))
    await user.click(screen.getByRole('button', { name: '开始这轮训练' }))

    await waitFor(() => expect(api.reviewQuickStart).toHaveBeenCalledWith({
      preset: 'ten_problems', subject: null, tag: null,
    }))
    await waitFor(() => expect(router.currentRoute.value.name).toBe('review'))
  })

  it('keeps the user on the dashboard when quick selection is empty', async () => {
    const user = userEvent.setup()
    api.reviewQuickStart.mockResolvedValue({
      ok: false,
      error: {
        code: 'review_quick_empty',
        userMessage: '当前没有符合条件的题目，可以调整科目或标签后再试。',
        retryable: false,
        diagnosticId: 'quick-empty',
      },
    })
    const router = await renderWithRouter()
    await user.click(await screen.findByRole('button', { name: '快速训练' }))
    await user.click(screen.getByRole('button', { name: '开始这轮训练' }))

    expect(await screen.findByRole('alert')).toHaveTextContent('调整科目或标签')
    expect(router.currentRoute.value.name).toBe('dashboard')
    expect(screen.getByRole('button', { name: '开始这轮训练' })).toBeEnabled()
  })

  it('saves a learning goal and refreshes the daily plan', async () => {
    const user = userEvent.setup()
    await renderWithRouter()
    await user.click(await screen.findByRole('button', { name: '调整学习目标' }))
    await user.clear(screen.getByLabelText('每日复习题数'))
    await user.type(screen.getByLabelText('每日复习题数'), '30')
    await user.clear(screen.getByLabelText('每日学习时间（分钟）'))
    await user.type(screen.getByLabelText('每日学习时间（分钟）'), '25')
    await user.click(screen.getByRole('button', { name: '保存目标' }))

    await waitFor(() => expect(api.learningGoalSave).toHaveBeenCalledWith({
      dailyReviewTarget: 30,
      dailyMinutesTarget: 25,
    }))
    await waitFor(() => expect(api.dashboardOverview).toHaveBeenCalledTimes(2))
  })
})
