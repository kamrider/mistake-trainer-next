import { describe, expect, it, vi } from 'vitest'
import type { CaptureBatchDetail, CaptureBatchSummary } from '../../../shared/api/bindings'
import { failure, success, type AppResult } from '../../../shared/api/app-result'
import { useCaptureBatchData } from './useCaptureBatchData'

const batch = {
  id: 'batch-1', subject: '数学', state: 'collecting', itemCount: 1,
  draftCount: 0, readyCount: 0, updatedAtUtcMs: 1, revision: 1,
} as CaptureBatchSummary
const detail = { batch, items: [], drafts: [], unassignedItemIds: [], pairSuggestions: [] } as CaptureBatchDetail

function deferred<T>() {
  let resolve!: (value: T) => void
  const promise = new Promise<T>(finish => { resolve = finish })
  return { promise, resolve }
}

function harness() {
  let routeBatchId: string | undefined
  const list = vi.fn(async () => success([batch]))
  const load = vi.fn(async () => success(detail))
  const onError = vi.fn()
  const onDetailRequested = vi.fn()
  const replaceRouteBatchId = vi.fn((value: string) => { routeBatchId = value })
  const controller = useCaptureBatchData({
    desktopAvailable: true,
    list,
    detail: load,
    onError,
    routeBatchId: () => routeBatchId,
    replaceRouteBatchId,
  })
  controller.setDetailRequestedHandler(onDetailRequested)
  return { controller, list, load, onError, onDetailRequested, replaceRouteBatchId }
}

describe('useCaptureBatchData', () => {
  it('loads list and detail while projecting request and route identity', async () => {
    const h = harness()
    await h.controller.loadBatches()
    await h.controller.loadDetail('batch-1')
    expect(h.controller.batches.value).toEqual([batch])
    expect(h.controller.detail.value).toEqual(detail)
    expect(h.controller.requestedBatchId.value).toBe('batch-1')
    expect(h.onDetailRequested).toHaveBeenCalledWith('batch-1')
    expect(h.replaceRouteBatchId).toHaveBeenCalledWith('batch-1')
  })

  it('drops stale detail completion after a newer request or clear', async () => {
    const first = deferred<AppResult<CaptureBatchDetail>>()
    const second = deferred<AppResult<CaptureBatchDetail>>()
    const h = harness()
    h.load.mockReturnValueOnce(first.promise).mockReturnValueOnce(second.promise)
    const oldRequest = h.controller.loadDetail('batch-1')
    const newRequest = h.controller.loadDetail('batch-2')
    first.resolve(success(detail))
    await oldRequest
    expect(h.controller.detail.value).toBeUndefined()
    h.controller.clearDetail()
    second.resolve(success({ ...detail, batch: { ...batch, id: 'batch-2' } }))
    await newRequest
    expect(h.controller.detail.value).toBeUndefined()
  })

  it('owns command and transport failure copy', async () => {
    const h = harness()
    h.list.mockResolvedValue(failure('list_failed', '列表失败', true, 'diag-list'))
    h.load.mockRejectedValue(new Error('offline'))
    await h.controller.loadBatches()
    await h.controller.loadDetail('batch-1')
    expect(h.onError).toHaveBeenNthCalledWith(1, '列表失败')
    expect(h.onError).toHaveBeenNthCalledWith(2, '没有读取到这个采集批次，请返回后重试。')
  })

  it('supports explicit external replacement and development hydration', () => {
    const h = harness()
    h.controller.replaceDetail(detail)
    expect(h.controller.detail.value).toEqual(detail)
    h.controller.hydrateDevelopment({ batches: [batch], detail: undefined })
    expect(h.controller.batches.value).toEqual([batch])
    expect(h.controller.detail.value).toBeUndefined()
  })
})
