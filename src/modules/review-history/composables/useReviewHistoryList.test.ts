import { describe, expect, it, vi } from 'vitest'
import type { ReviewHistoryItem, ReviewHistoryPage } from '../../../shared/api/bindings'
import { failure, success } from '../../../shared/api/app-result'
import { type ReviewHistoryQuery, useReviewHistoryList } from './useReviewHistoryList'

const firstItem: ReviewHistoryItem = {
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
const secondItem: ReviewHistoryItem = {
  ...firstItem,
  eventId: 'event-2',
  subject: '物理',
  notePreview: '受力分析',
}
const initialQuery: ReviewHistoryQuery = {
  range: '30_days',
  rating: null,
  subject: null,
  search: '',
}
const physicsQuery: ReviewHistoryQuery = {
  ...initialQuery,
  subject: '物理',
}

function page(
  items: ReviewHistoryItem[],
  nextCursor: string | null = null,
  availableSubjects = ['数学', '物理'],
): ReviewHistoryPage {
  return { items, nextCursor, totalCount: items.length, availableSubjects }
}

function deferred<T>() {
  let resolve!: (value: T) => void
  let reject!: (reason?: unknown) => void
  const promise = new Promise<T>((finish, fail) => { resolve = finish; reject = fail })
  return { promise, resolve, reject }
}

function harness() {
  const listPage = vi.fn(async () => success(page([])))
  const controller = useReviewHistoryList({ listPage, initialQuery, pageSize: 20 })
  return { controller, listPage }
}

describe('useReviewHistoryList', () => {
  it('lets only the latest replacement request commit state', async () => {
    const current = harness()
    const first = deferred<ReturnType<typeof success<ReviewHistoryPage>>>()
    const second = deferred<ReturnType<typeof success<ReviewHistoryPage>>>()
    current.listPage.mockReturnValueOnce(first.promise).mockReturnValueOnce(second.promise)

    const older = current.controller.replace(initialQuery)
    const newer = current.controller.replace(physicsQuery)
    second.resolve(success({ ...page([secondItem], 'physics-next', ['物理']), totalCount: 4 }))
    await expect(newer).resolves.toBe(true)

    first.resolve(success({ ...page([firstItem], 'math-next', ['数学']), totalCount: 8 }))
    await expect(older).resolves.toBe(false)
    expect(current.controller.items.value).toEqual([secondItem])
    expect(current.controller.subjects.value).toEqual(['物理'])
    expect(current.controller.nextCursor.value).toBe('physics-next')
    expect(current.controller.totalCount.value).toBe(4)
    expect(current.controller.loading.value).toBe(false)
  })

  it('ignores a stale thrown replacement after a newer success', async () => {
    const current = harness()
    const olderGate = deferred<ReturnType<typeof success<ReviewHistoryPage>>>()
    current.listPage
      .mockReturnValueOnce(olderGate.promise)
      .mockResolvedValueOnce(success(page([secondItem])))

    const older = current.controller.replace(initialQuery)
    await expect(current.controller.replace(physicsQuery)).resolves.toBe(true)
    olderGate.reject(new Error('stale database error'))

    await expect(older).resolves.toBe(false)
    expect(current.controller.items.value).toEqual([secondItem])
    expect(current.controller.errorMessage.value).toBe('')
  })

  it('blocks old-cursor pagination while a replacement is pending', async () => {
    const current = harness()
    current.listPage.mockResolvedValueOnce(success({ ...page([firstItem], 'cursor-old'), totalCount: 2 }))
    await current.controller.replace(initialQuery)

    const replacement = deferred<ReturnType<typeof success<ReviewHistoryPage>>>()
    current.listPage.mockReturnValueOnce(replacement.promise)
    const pending = current.controller.replace(physicsQuery)
    expect(current.controller.loading.value).toBe(true)
    expect(current.controller.nextCursor.value).toBe('cursor-old')

    await expect(current.controller.loadMore()).resolves.toBe(false)
    expect(current.listPage).toHaveBeenCalledTimes(2)

    replacement.resolve(success(page([secondItem], 'cursor-new')))
    await expect(pending).resolves.toBe(true)
    await current.controller.loadMore()
    expect(current.listPage).toHaveBeenLastCalledWith({ ...physicsQuery, cursor: 'cursor-new', limit: 20 })
  })

  it('deduplicates an appended page and retries the exact failed request', async () => {
    const current = harness()
    current.listPage
      .mockResolvedValueOnce(success({ ...page([firstItem], 'cursor-2'), totalCount: 2 }))
      .mockResolvedValueOnce(failure('history_append_failed', '下一页读取失败。', true, 'diag-append'))
      .mockResolvedValueOnce(success({ ...page([firstItem, secondItem]), totalCount: 2 }))

    await current.controller.replace(initialQuery)
    await expect(current.controller.loadMore()).resolves.toBe(false)
    expect(current.controller.items.value).toEqual([firstItem])
    expect(current.controller.errorMessage.value).toBe('下一页读取失败。')
    expect(current.controller.stale.value).toBe(true)
    expect(current.controller.nextCursor.value).toBeNull()

    await expect(current.controller.retry()).resolves.toBe(true)
    expect(current.listPage).toHaveBeenLastCalledWith({ ...initialQuery, cursor: 'cursor-2', limit: 20 })
    expect(current.controller.items.value).toEqual([firstItem, secondItem])
    expect(current.controller.stale.value).toBe(false)
  })

  it('preserves the snapshot for a thrown append and coalesces pagination flights', async () => {
    const current = harness()
    current.listPage.mockResolvedValueOnce(success({ ...page([firstItem], 'cursor-2'), totalCount: 2 }))
    await current.controller.replace(initialQuery)

    const append = deferred<ReturnType<typeof success<ReviewHistoryPage>>>()
    current.listPage.mockReturnValueOnce(append.promise)
    const pending = current.controller.loadMore()
    expect(current.controller.loadingMore.value).toBe(true)
    await expect(current.controller.loadMore()).resolves.toBe(false)
    await expect(current.controller.retry()).resolves.toBe(false)
    expect(current.listPage).toHaveBeenCalledTimes(2)

    append.reject(new Error('offline'))
    await expect(pending).resolves.toBe(false)
    expect(current.controller.items.value).toEqual([firstItem])
    expect(current.controller.errorMessage.value).toBe('复习历史暂时无法读取，请稍后重试。')
    expect(current.controller.loadingMore.value).toBe(false)
  })

  it('distinguishes current failures from a successful empty result', async () => {
    const current = harness()
    current.listPage
      .mockResolvedValueOnce(failure('history_failed', '历史读取失败。', true, 'diag-list'))
      .mockRejectedValueOnce(new Error('offline'))
      .mockResolvedValueOnce(success(page([])))

    await expect(current.controller.replace(initialQuery)).resolves.toBe(false)
    expect(current.controller.errorMessage.value).toBe('历史读取失败。')
    expect(current.controller.loaded.value).toBe(false)
    await expect(current.controller.retry()).resolves.toBe(false)
    expect(current.controller.errorMessage.value).toBe('复习历史暂时无法读取，请稍后重试。')

    await expect(current.controller.replace(initialQuery)).resolves.toBe(true)
    expect(current.controller.items.value).toEqual([])
    expect(current.controller.totalCount.value).toBe(0)
    expect(current.controller.loaded.value).toBe(true)
    expect(current.controller.stale.value).toBe(false)
    await expect(current.controller.loadMore()).resolves.toBe(false)
  })

  it('reloads the current query when retry has no failed request', async () => {
    const current = harness()
    await expect(current.controller.retry()).resolves.toBe(true)
    expect(current.listPage).toHaveBeenCalledWith({ ...initialQuery, cursor: null, limit: 20 })
  })
})
