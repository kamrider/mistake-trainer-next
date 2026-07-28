import { render, screen, waitFor } from '@testing-library/vue'
import userEvent from '@testing-library/user-event'
import { createMemoryHistory } from 'vue-router'
import { beforeEach, describe, expect, it, vi } from 'vitest'
import { createAppRouter } from '../router'
import ReviewHistoryView from './ReviewHistoryView.vue'

const api = vi.hoisted(() => ({ reviewHistoryList: vi.fn(), reviewHistoryDetail: vi.fn() }))
vi.mock('@tauri-apps/api/core', () => ({ isTauri: () => true }))
vi.mock('../../shared/api/bindings', () => ({ commands: api }))

const row = { eventId: 'event-1', subject: '数学', notePreview: '圆锥曲线', problemStatus: 'active', rating: 'good' as const, durationMs: 50_000, occurredAtUtcMs: 1_750_000_000_000, algorithmVersion: 'fsrs-6.6.1', parameterVersion: 'default-6.6.1', algorithmIsCurrent: true, parametersAreCurrent: true }
const detail = { ...row, note: '完整圆锥曲线笔记', isCurrentDevice: true, reviewOrdinal: 1, problemReviewCount: 1, currentSchedule: null }

async function renderView() {
  const router = createAppRouter(createMemoryHistory())
  await router.push('/report/history')
  await router.isReady()
  render(ReviewHistoryView, { global: { plugins: [router] } })
}

describe('ReviewHistoryView', () => {
  beforeEach(() => {
    vi.clearAllMocks()
    api.reviewHistoryList.mockResolvedValue({ ok: true, data: { items: [row], nextCursor: 'next', totalCount: 2, availableSubjects: ['数学'] } })
    api.reviewHistoryDetail.mockResolvedValue({ ok: true, data: detail })
  })

  it('loads real history, opens detail, and never places the event id in the route', async () => {
    const user = userEvent.setup()
    await renderView()
    const historyRow = await screen.findByRole('button', { name: /数学.*记住.*查看审计详情/ })
    expect(api.reviewHistoryList).toHaveBeenCalledWith({ range: '30_days', rating: null, subject: null, search: '', cursor: null, limit: 20 })
    await user.click(historyRow)
    expect(await screen.findByText('完整圆锥曲线笔记')).toBeVisible()
    expect(window.location.hash).not.toContain('event-1')
    expect(screen.getByText('本机设备')).toBeVisible()
  })

  it('preserves existing rows when loading another page fails', async () => {
    const user = userEvent.setup()
    api.reviewHistoryList
      .mockResolvedValueOnce({ ok: true, data: { items: [row], nextCursor: 'next', totalCount: 2, availableSubjects: ['数学'] } })
      .mockResolvedValueOnce({ ok: false, error: { code: 'read', userMessage: '下一页读取失败。', retryable: true, diagnosticId: 'diag' } })
    await renderView()
    await user.click(await screen.findByRole('button', { name: '加载更多' }))
    expect(await screen.findByRole('alert')).toHaveTextContent('下一页读取失败。')
    expect(screen.getByRole('button', { name: /数学.*记住.*查看审计详情/ })).toBeVisible()
    expect(screen.getByText('当前仍显示上一次成功读取的记录。')).toBeVisible()
    await waitFor(() => expect(api.reviewHistoryList).toHaveBeenLastCalledWith(expect.objectContaining({ cursor: 'next' })))
  })
})
