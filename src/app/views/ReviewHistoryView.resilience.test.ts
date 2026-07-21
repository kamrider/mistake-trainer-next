import { fireEvent, render, screen, waitFor } from '@testing-library/vue'
import userEvent from '@testing-library/user-event'
import { createMemoryHistory } from 'vue-router'
import { beforeEach, describe, expect, it, vi } from 'vitest'
import { createAppRouter } from '../router'
import ReviewHistoryView from './ReviewHistoryView.vue'

const api = vi.hoisted(() => ({ reviewHistoryList: vi.fn(), reviewHistoryDetail: vi.fn() }))
vi.mock('@tauri-apps/api/core', () => ({ isTauri: () => true }))
vi.mock('../../shared/api/bindings', () => ({ commands: api }))

const row = { eventId: 'event-1', subject: 'Math', notePreview: 'First note', problemStatus: 'active', rating: 'good' as const, durationMs: 50_000, occurredAtUtcMs: 1_750_000_000_000, algorithmVersion: 'fsrs-6.6.1', parameterVersion: 'default-6.6.1', algorithmIsCurrent: true, parametersAreCurrent: true }
const secondRow = { ...row, eventId: 'event-2', subject: 'Physics', notePreview: 'Second note' }
const detail = { ...row, note: 'First complete note', isCurrentDevice: true, reviewOrdinal: 1, problemReviewCount: 1, currentSchedule: null }
const secondDetail = { ...detail, ...secondRow, note: 'Second complete note' }

function deferred<T>() {
  let resolve!: (value: T) => void
  const promise = new Promise<T>((done) => { resolve = done })
  return { promise, resolve }
}

async function renderView() {
  const router = createAppRouter(createMemoryHistory())
  await router.push('/report/history')
  await router.isReady()
  return render(ReviewHistoryView, { global: { plugins: [router] } })
}

describe('ReviewHistoryView resilience', () => {
  beforeEach(() => vi.clearAllMocks())

  it('distinguishes an initial read failure from a genuine empty result', async () => {
    const user = userEvent.setup()
    api.reviewHistoryList
      .mockResolvedValueOnce({ ok: false, error: { code: 'read', userMessage: 'Read failed.', retryable: true, diagnosticId: 'diag' } })
      .mockResolvedValueOnce({ ok: true, data: { items: [], nextCursor: null, totalCount: 0, availableSubjects: [] } })
    const view = await renderView()
    await screen.findByText('Read failed.')
    expect(view.container.querySelector('.initial-error')).toBeVisible()
    expect(view.container.querySelectorAll('.empty-state')).toHaveLength(1)
    await user.click(view.container.querySelector<HTMLButtonElement>('.initial-error button')!)
    await waitFor(() => expect(view.container.querySelector('.initial-error')).not.toBeInTheDocument())
    expect(view.container.querySelector('.empty-state')).toBeVisible()
  })

  it('retries the exact failed append cursor', async () => {
    const user = userEvent.setup()
    api.reviewHistoryList
      .mockResolvedValueOnce({ ok: true, data: { items: [row], nextCursor: 'cursor-2', totalCount: 2, availableSubjects: [] } })
      .mockResolvedValueOnce({ ok: false, error: { code: 'read', userMessage: 'Append failed.', retryable: true, diagnosticId: 'diag' } })
      .mockResolvedValueOnce({ ok: true, data: { items: [secondRow], nextCursor: null, totalCount: 2, availableSubjects: [] } })
    const view = await renderView()
    await waitFor(() => expect(view.container.querySelector('.more-button')).toBeInTheDocument())
    await user.click(view.container.querySelector<HTMLButtonElement>('.more-button')!)
    await screen.findByText('Append failed.')
    await user.click(view.container.querySelector<HTMLButtonElement>('.error-banner button')!)
    await waitFor(() => expect(api.reviewHistoryList).toHaveBeenCalledTimes(3))
    expect(api.reviewHistoryList).toHaveBeenLastCalledWith(expect.objectContaining({ cursor: 'cursor-2' }))
  })

  it('ignores stale detail responses and restores focus to the closing row', async () => {
    const first = deferred<{ ok: true; data: typeof detail }>()
    const second = deferred<{ ok: true; data: typeof secondDetail }>()
    api.reviewHistoryList.mockResolvedValueOnce({ ok: true, data: { items: [row, secondRow], nextCursor: null, totalCount: 2, availableSubjects: [] } })
    api.reviewHistoryDetail.mockReturnValueOnce(first.promise).mockReturnValueOnce(second.promise)
    const view = await renderView()
    await waitFor(() => expect(view.container.querySelectorAll('.history-row')).toHaveLength(2))
    const rows = view.container.querySelectorAll<HTMLButtonElement>('.history-row')
    await fireEvent.click(rows[0]!)
    await fireEvent.click(rows[1]!)
    second.resolve({ ok: true, data: secondDetail })
    expect(await screen.findByText('Second complete note')).toBeVisible()
    first.resolve({ ok: true, data: detail })
    await waitFor(() => expect(screen.queryByText('First complete note')).not.toBeInTheDocument())
    await fireEvent.click(view.container.querySelector<HTMLButtonElement>('.history-detail header button')!)
    await waitFor(() => expect(rows[1]).toHaveFocus())
  })
})
