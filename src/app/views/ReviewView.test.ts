import { render, screen, waitFor } from '@testing-library/vue'
import userEvent from '@testing-library/user-event'
import { createMemoryHistory } from 'vue-router'
import { beforeEach, describe, expect, it, vi } from 'vitest'
import { createAppRouter } from '../router'
import ReviewView from './ReviewView.vue'

const api = vi.hoisted(() => ({
  reviewQueue: vi.fn(),
  problemDetail: vi.fn(),
  reviewSubmit: vi.fn(),
}))

vi.mock('@tauri-apps/api/core', () => ({ isTauri: () => true }))
vi.mock('../../shared/api/bindings', () => ({ commands: api }))

const queueOverview = {
  sessionId: 'session-1',
  mode: 'due',
  resumed: true,
  completedCount: 1,
  totalCount: 2,
  items: [{ problemId: 'problem-2', dueAtUtcMs: null, reviewCount: 0 }],
}

const problemDetail = {
  id: 'problem-2',
  subject: '数学',
  note: '先独立写出关键步骤。',
  status: 'active',
  timeLimitSeconds: 60,
  updatedAtUtcMs: 1,
  assets: [],
}

async function renderView() {
  const router = createAppRouter(createMemoryHistory())
  await router.push('/review')
  await router.isReady()
  render(ReviewView, { global: { plugins: [router] } })
  return router
}

describe('ReviewView', () => {
  beforeEach(() => {
    vi.clearAllMocks()
    api.reviewQueue.mockResolvedValue({ ok: true, data: queueOverview })
    api.problemDetail.mockResolvedValue({ ok: true, data: problemDetail })
    api.reviewSubmit.mockResolvedValue({ ok: true, data: {
      eventId: 'event-1', problemId: 'problem-2', rating: 'good', dueAtUtcMs: 2,
      stability: 1, difficulty: 5, algorithmVersion: 'fsrs-6.6.1', parameterVersion: 'default-6.6.1',
    } })
  })

  it('uses persisted progress and submits a bounded stopped duration exactly once', async () => {
    const user = userEvent.setup()
    await renderView()

    expect(await screen.findByText('2 / 2')).toBeVisible()
    expect(screen.getByText('已恢复上次进度')).toBeVisible()
    await user.click(screen.getByRole('button', { name: '显示答案' }))
    await user.click(screen.getByRole('button', { name: '记住了' }))

    await waitFor(() => expect(api.reviewSubmit).toHaveBeenCalledOnce())
    const [input] = api.reviewSubmit.mock.calls[0]!
    expect(input).toMatchObject({ problemId: 'problem-2', rating: 'good' })
    expect(input.durationMs).toBeGreaterThanOrEqual(0)
    expect(input.durationMs).toBeLessThanOrEqual(86_400_000)
    expect(await screen.findByRole('heading', { name: '把今天该记住的，认真看完了。' })).toBeVisible()
  })

  it('retries a failed queue read without showing fake review content', async () => {
    const user = userEvent.setup()
    api.reviewQueue
      .mockResolvedValueOnce({ ok: false, error: { code: 'review_queue_failed', userMessage: '资料库暂时忙碌。', retryable: true, diagnosticId: 'diag-1' } })
      .mockResolvedValueOnce({ ok: true, data: queueOverview })
    await renderView()

    expect(await screen.findByRole('alert')).toHaveTextContent('资料库暂时忙碌。')
    expect(api.problemDetail).not.toHaveBeenCalled()
    await user.click(screen.getByRole('button', { name: '重新读取训练' }))
    expect(await screen.findByText('2 / 2')).toBeVisible()
    expect(api.reviewQueue).toHaveBeenCalledTimes(2)
  })

  it('keeps the same card visible when rating persistence fails', async () => {
    const user = userEvent.setup()
    api.reviewSubmit.mockResolvedValue({ ok: false, error: {
      code: 'review_submit_failed', userMessage: '评分没有保存。', retryable: true, diagnosticId: 'diag-2',
    } })
    await renderView()

    await user.click(await screen.findByRole('button', { name: '显示答案' }))
    await user.click(screen.getByRole('button', { name: '记住了' }))
    expect(await screen.findByRole('alert')).toHaveTextContent('评分没有保存。')
    expect(screen.getByText('2 / 2')).toBeVisible()
    expect(screen.getByRole('button', { name: '记住了' })).toBeEnabled()
  })
})
