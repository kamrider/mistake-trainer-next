import { render, screen } from '@testing-library/vue'
import userEvent from '@testing-library/user-event'
import { describe, expect, it } from 'vitest'
import ReviewHistoryTimeline from './ReviewHistoryTimeline.vue'

const items = [
  { eventId: 'event-b', subject: '数学', notePreview: '焦点弦', problemStatus: 'active', rating: 'good' as const, durationMs: 65_000, occurredAtUtcMs: new Date(2026, 6, 20, 9).getTime(), algorithmVersion: 'fsrs-6.6.1', parameterVersion: 'default', algorithmIsCurrent: true, parametersAreCurrent: true },
  { eventId: 'event-a', subject: '物理', notePreview: '磁场方向', problemStatus: 'archived', rating: 'again' as const, durationMs: 20_000, occurredAtUtcMs: new Date(2026, 6, 19, 18).getTime(), algorithmVersion: 'fsrs-5', parameterVersion: 'legacy', algorithmIsCurrent: false, parametersAreCurrent: false },
]

describe('ReviewHistoryTimeline', () => {
  it('groups by local date, activates rows by keyboard, and requests explicit pagination', async () => {
    const user = userEvent.setup()
    const view = render(ReviewHistoryTimeline, { props: { items, selectedId: '', nextCursor: 'opaque', loadingMore: false } })

    expect(screen.getByRole('heading', { name: /2026年7月20日/ })).toBeVisible()
    expect(screen.getByRole('heading', { name: /2026年7月19日/ })).toBeVisible()
    const row = screen.getByRole('button', { name: /数学.*记住.*查看审计详情/ })
    row.focus()
    await user.keyboard('{Enter}')
    const selections = view.emitted().select as unknown[][]
    expect(selections[0]?.[0]).toBe('event-b')
    await user.click(screen.getByRole('button', { name: '加载更多' }))
    expect(view.emitted().more).toHaveLength(1)
  })
})
