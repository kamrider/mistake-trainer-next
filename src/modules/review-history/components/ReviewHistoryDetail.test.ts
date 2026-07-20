import { render, screen } from '@testing-library/vue'
import userEvent from '@testing-library/user-event'
import { describe, expect, it } from 'vitest'
import ReviewHistoryDetail from './ReviewHistoryDetail.vue'

const detail = {
  eventId: 'secret-event-id', subject: '物理', note: '完整笔记', problemStatus: 'archived', rating: 'again' as const,
  durationMs: 42_000, occurredAtUtcMs: 1_750_000_000_000, algorithmVersion: 'fsrs-5', parameterVersion: 'legacy-1',
  algorithmIsCurrent: false, parametersAreCurrent: false, isCurrentDevice: false, reviewOrdinal: 2, problemReviewCount: 5,
  currentSchedule: { dueAtUtcMs: 1_750_086_400_000, stability: 4.25, difficulty: 3.5, lastReviewedAtUtcMs: 1_750_000_000_000, algorithmVersion: 'fsrs-6.6.1', parameterVersion: 'default-6.6.1' },
}

describe('ReviewHistoryDetail', () => {
  it('separates immutable facts from the current projection without exposing raw ids', async () => {
    const user = userEvent.setup()
    const view = render(ReviewHistoryDetail, { props: { detail, loading: false, error: '' } })

    expect(screen.getByText('不可变事件事实')).toBeVisible()
    expect(screen.getByText('当前排程投影')).toBeVisible()
    expect(screen.getByText('其他设备')).toBeVisible()
    expect(screen.getAllByText('历史')).toHaveLength(2)
    expect(screen.getByText(/不是当时的历史快照/)).toBeVisible()
    expect(screen.queryByText('secret-event-id')).not.toBeInTheDocument()
    await user.click(screen.getByRole('button', { name: '关闭复习详情' }))
    expect(view.emitted().close).toHaveLength(1)
  })
})
