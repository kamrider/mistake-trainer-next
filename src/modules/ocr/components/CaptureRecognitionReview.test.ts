import { fireEvent, render, screen, waitFor, within } from '@testing-library/vue'
import userEvent from '@testing-library/user-event'
import { describe, expect, it } from 'vitest'
import type {
  CaptureRecognitionJob,
  CaptureRecognitionSuggestion,
} from '../../../shared/api/bindings'
import CaptureRecognitionReview from './CaptureRecognitionReview.vue'

function suggestion(
  id: string,
  reviewBand: CaptureRecognitionSuggestion['reviewBand'],
  state: CaptureRecognitionSuggestion['state'] = 'proposed',
): CaptureRecognitionSuggestion {
  return {
    id,
    itemId: `item-${id}`,
    confidenceBasisPoints: reviewBand === 'high' ? 9300 : reviewBand === 'review' ? 7600 : 5200,
    reviewBand,
    state,
    reasonCodes: reviewBand === 'high' ? ['clear_question_anchor'] : ['weak_anchor'],
    regions: [{
      rect: { x: 0.1, y: 0.1, width: 0.8, height: 0.35 },
      role: 'question',
      groupSlot: 0,
      confidenceBasisPoints: reviewBand === 'high' ? 9300 : 7600,
    }],
  }
}

function job(): CaptureRecognitionJob {
  return {
    id: 'job-1',
    batchId: 'batch-1',
    state: 'review',
    totalItems: 4,
    processedItems: 4,
    suggestions: [
      suggestion('high', 'high'),
      suggestion('review', 'review'),
      suggestion('low', 'low'),
      suggestion('stale', 'high', 'stale'),
    ],
    createdAtUtcMs: 1,
    updatedAtUtcMs: 2,
  }
}

describe('CaptureRecognitionReview', () => {
  it('starts with items needing review and exposes all confidence groups', () => {
    render(CaptureRecognitionReview, { props: { job: job(), previews: {} } })

    expect(screen.getByRole('button', { name: /需要检查 1/ })).toHaveClass('active')
    expect(screen.getByRole('button', { name: /可快速确认 1/ })).toBeVisible()
    expect(screen.getByRole('button', { name: /无法安全切分 1/ })).toBeVisible()
    expect(screen.getByRole('button', { name: /已过期 1/ })).toBeVisible()
    expect(screen.getByText('题号不够清晰')).toBeVisible()
  })

  it('requests the current source preview when review opens and navigation changes', async () => {
    const user = userEvent.setup()
    const view = render(CaptureRecognitionReview, { props: { job: job(), previews: {} } })

    await waitFor(() => expect(view.emitted('preview')).toEqual([['item-review']]))
    await user.click(screen.getByRole('button', { name: /可快速确认 1/ }))
    await waitFor(() => expect(view.emitted('preview')).toEqual([
      ['item-review'],
      ['item-high'],
    ]))
  })

  it('accepts only high-confidence items in bulk', async () => {
    const user = userEvent.setup()
    const view = render(CaptureRecognitionReview, { props: { job: job(), previews: {} } })

    await user.click(screen.getByRole('button', { name: '仅接受全部高可信建议' }))
    expect(view.emitted('reviewMany')).toEqual([[[
      {
        jobId: 'job-1',
        suggestionId: 'high',
        decision: 'accepted',
        editedRegions: null,
      },
    ]]])
    expect(view.emitted('reviewMany')).not.toContainEqual([
      expect.arrayContaining([
        expect.objectContaining({ suggestionId: 'low' }),
      ]),
    ])
    expect(view.emitted('reviewMany')).not.toContainEqual([
      expect.arrayContaining([
        expect.objectContaining({ suggestionId: 'stale' }),
      ]),
    ])
  })

  it('keeps low and stale suggestions unacceptably safe', async () => {
    const user = userEvent.setup()
    const view = render(CaptureRecognitionReview, { props: { job: job(), previews: {} } })

    await user.click(screen.getByRole('button', { name: /无法安全切分 1/ }))
    expect(screen.queryByRole('button', { name: '接受建议' })).not.toBeInTheDocument()
    expect(screen.queryByRole('button', { name: '调整边界' })).not.toBeInTheDocument()
    expect(screen.getByRole('button', { name: '保留原图' })).toBeVisible()
    await user.click(screen.getByRole('button', { name: '手工裁剪' }))
    expect(view.emitted('edit')).toEqual([[expect.objectContaining({ id: 'low' })]])
    expect(screen.getByText(/依据不足/)).toBeVisible()

    await user.click(screen.getByRole('button', { name: /已过期 1/ }))
    expect(screen.queryByRole('button', { name: '接受建议' })).not.toBeInTheDocument()
    expect(screen.getByRole('button', { name: '忽略已过期建议' })).toBeVisible()
    expect(screen.getByText(/不会再应用/)).toBeVisible()
  })

  it('supports keyboard review without stealing keys from inputs', async () => {
    const fixture = job()
    fixture.suggestions.splice(2, 0, suggestion('review-2', 'review'))
    const view = render(CaptureRecognitionReview, { props: { job: fixture, previews: {} } })
    const surface = screen.getByRole('heading', { name: '快速确认，不替你做决定' }).closest('section')!
    await fireEvent.keyDown(surface, { key: 'j' })
    expect(screen.getByText('2 / 2')).toBeVisible()
    await waitFor(() => expect(screen.getByRole('status')).toHaveTextContent('第 2 条，共 2 条'))

    await fireEvent.keyDown(surface, { key: 'k' })
    expect(screen.getByText('1 / 2')).toBeVisible()
    await fireEvent.keyDown(surface, { key: 'e' })
    expect(view.emitted('edit')).toEqual([[expect.objectContaining({ id: 'review' })]])

    await fireEvent.keyDown(surface, { key: 'Enter' })
    expect(view.emitted('review')).toContainEqual([{
      jobId: 'job-1',
      suggestionId: 'review',
      decision: 'accepted',
      editedRegions: null,
    }])
    await waitFor(() => {
      expect(screen.getByRole('status')).toHaveTextContent('已接受第 1 条建议，共 2 条')
    })

    await fireEvent.keyDown(surface, { key: 's' })
    expect(view.emitted('review')).toContainEqual([{
      jobId: 'job-1',
      suggestionId: 'review-2',
      decision: 'rejected',
      editedRegions: null,
    }])
    await waitFor(() => {
      expect(screen.getByRole('status')).toHaveTextContent('已跳过第 2 条建议，共 2 条')
    })

    const input = document.createElement('input')
    surface.append(input)
    await fireEvent.keyDown(input, { key: 's' })
    expect(view.emitted('review')).toHaveLength(2)
  })

  it('announces category changes and exposes the active category to assistive technology', async () => {
    const user = userEvent.setup()
    render(CaptureRecognitionReview, { props: { job: job(), previews: {} } })

    const high = screen.getByRole('button', { name: /可快速确认 1/ })
    expect(screen.getByRole('button', { name: /需要检查 1/ })).toHaveAttribute('aria-pressed', 'true')
    await user.click(high)
    expect(high).toHaveAttribute('aria-pressed', 'true')
    expect(screen.getByRole('status')).toHaveTextContent('已打开可快速确认，1 条')
  })

  it('shows exact impact before applying accepted suggestions', async () => {
    const user = userEvent.setup()
    const view = render(CaptureRecognitionReview, { props: { job: job(), previews: {} } })
    await user.click(screen.getByRole('button', { name: '接受建议' }))
    await user.click(screen.getByRole('button', { name: /把切图放入素材牌库（1）/ }))

    const dialog = screen.getByRole('alertdialog')
    expect(dialog).toHaveFocus()
    expect(within(dialog).getByText('1 张原图会保留')).toBeVisible()
    expect(within(dialog).getByText('1 张题图会进入素材牌库')).toBeVisible()
    expect(within(dialog).getByText('0 张答案图会进入素材牌库')).toBeVisible()
    expect(within(dialog).getByText('不会自动新建或改动题卡')).toBeVisible()
    await user.click(within(dialog).getByRole('button', { name: '放入素材牌库' }))
    expect(view.emitted('applyAccepted')).toEqual([[['review']]])
  })

  it('explains that a multi-question page becomes separate images while preserving the source', () => {
    const fixture = job()
    fixture.suggestions[1]!.regions.push({
      rect: { x: 0.1, y: 0.5, width: 0.8, height: 0.35 },
      role: 'question',
      groupSlot: 1,
      confidenceBasisPoints: 7600,
    })

    render(CaptureRecognitionReview, { props: { job: fixture, previews: {} } })

    expect(screen.getByRole('heading', { name: '这张原图将拆成 2 张题图' })).toBeVisible()
    expect(screen.getByText(/原图始终保留/)).toBeVisible()
  })

  it('returns focus to apply after cancelling the impact confirmation with Escape', async () => {
    const user = userEvent.setup()
    render(CaptureRecognitionReview, { props: { job: job(), previews: {} } })
    await user.click(screen.getByRole('button', { name: '接受建议' }))
    const apply = screen.getByRole('button', { name: /把切图放入素材牌库（1）/ })
    await user.click(apply)

    const dialog = screen.getByRole('alertdialog')
    expect(dialog).toHaveFocus()
    await fireEvent.keyDown(dialog, { key: 'Escape' })
    await waitFor(() => expect(apply).toHaveFocus())
    expect(screen.queryByRole('alertdialog')).not.toBeInTheDocument()
  })

  it('closes the modal review surface with Escape', async () => {
    const view = render(CaptureRecognitionReview, { props: { job: job(), previews: {} } })
    const surface = screen.getByRole('dialog', { name: '快速确认，不替你做决定' })

    await fireEvent.keyDown(surface, { key: 'Escape' })

    expect(view.emitted('close')).toHaveLength(1)
  })
})
