import { render, screen } from '@testing-library/vue'
import userEvent from '@testing-library/user-event'
import { describe, expect, it } from 'vitest'
import type { ReviewHistoryItem } from '../../../shared/api/bindings'
import ReviewHistoryTimeline from './ReviewHistoryTimeline.vue'

const item: ReviewHistoryItem = {
  eventId: 'event-1',
  subject: '数学',
  notePreview: '圆锥曲线',
  problemStatus: 'active',
  rating: 'good',
  durationMs: 50_000,
  occurredAtUtcMs: 1_750_000_000_000,
  algorithmVersion: 'fsrs-6.6.1',
  parameterVersion: 'default-6.6.1',
  algorithmIsCurrent: true,
  parametersAreCurrent: true,
}

describe('ReviewHistoryTimeline', () => {
  it('uses native disabled controls while a replacement list is pending', async () => {
    const view = render(ReviewHistoryTimeline, {
      props: {
        items: [item],
        selectedId: '',
        nextCursor: 'cursor-2',
        loadingMore: false,
        disabled: true,
      },
    })

    const timeline = screen.getByRole('region', { name: '复习记录时间线' })
    const row = screen.getByRole('button', { name: /数学.*记住.*查看审计详情/ })
    const more = screen.getByRole('button', { name: '加载更多' })
    expect(timeline).toHaveAttribute('aria-busy', 'true')
    expect(row).toBeDisabled()
    expect(more).toBeDisabled()
    await userEvent.click(row)
    await userEvent.click(more)
    expect(view.emitted('select')).toBeUndefined()
    expect(view.emitted('more')).toBeUndefined()

    await view.rerender({ disabled: false })
    expect(timeline).toHaveAttribute('aria-busy', 'false')
    await userEvent.click(row)
    await userEvent.click(more)
    expect(view.emitted('select')).toHaveLength(1)
    expect(view.emitted('more')).toHaveLength(1)
  })
})
