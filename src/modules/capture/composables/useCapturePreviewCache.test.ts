import { describe, expect, it, vi } from 'vitest'
import type { CaptureItemPreview } from '../../../shared/api/bindings'
import {
  failure,
  success,
  type AppResult,
} from '../../../shared/api/app-result'
import { useCapturePreviewCache } from './useCapturePreviewCache'

function deferred<T>() {
  let resolve!: (value: T) => void
  const promise = new Promise<T>((finish) => {
    resolve = finish
  })
  return { promise, resolve }
}

function preview(itemId: string): CaptureItemPreview {
  return {
    itemId,
    mediaType: 'image/png',
    dataUrl: `data:image/png;base64,${itemId}`,
  }
}

function createHarness(
  fetchPreview = vi.fn(
    async (_batchId: string, itemId: string) => success(preview(itemId)),
  ),
  maxEntries = 2,
) {
  let activeBatchId: string | undefined = 'batch-1'
  const cache = useCapturePreviewCache({
    activeBatchId: () => activeBatchId,
    fetchPreview,
    maxEntries,
  })
  return {
    cache,
    fetchPreview,
    setActiveBatchId: (value: string | undefined) => { activeBatchId = value },
  }
}

describe('useCapturePreviewCache', () => {
  it('coalesces duplicate requests', async () => {
    const gate = deferred<AppResult<CaptureItemPreview>>()
    const harness = createHarness(vi.fn().mockReturnValue(gate.promise))

    const first = harness.cache.load('item-1')
    const second = harness.cache.load('item-1')
    expect(harness.fetchPreview).toHaveBeenCalledOnce()

    gate.resolve(success(preview('item-1')))
    await Promise.all([first, second])

    expect(harness.cache.previews['item-1']).toBe(preview('item-1').dataUrl)
  })

  it('uses true LRU order when a cached item is used again', async () => {
    const harness = createHarness()

    await harness.cache.load('item-1')
    await harness.cache.load('item-2')
    await harness.cache.load('item-1')
    await harness.cache.load('item-3')

    expect(harness.cache.previews['item-1']).toBeDefined()
    expect(harness.cache.previews['item-2']).toBeUndefined()
    expect(harness.cache.previews['item-3']).toBeDefined()
    expect(harness.fetchPreview).toHaveBeenCalledTimes(3)
  })

  it('does not reinsert an invalidated in-flight item', async () => {
    const gate = deferred<AppResult<CaptureItemPreview>>()
    const harness = createHarness(vi.fn().mockReturnValue(gate.promise))

    const loading = harness.cache.load('item-1')
    harness.cache.invalidate('item-1')
    gate.resolve(success(preview('item-1')))
    await loading

    expect(harness.cache.previews['item-1']).toBeUndefined()
  })

  it('does not let an older response overwrite a reload after invalidation', async () => {
    const oldGate = deferred<AppResult<CaptureItemPreview>>()
    const newGate = deferred<AppResult<CaptureItemPreview>>()
    const fetchPreview = vi.fn()
      .mockReturnValueOnce(oldGate.promise)
      .mockReturnValueOnce(newGate.promise)
    const harness = createHarness(fetchPreview)

    const oldLoading = harness.cache.load('item-1')
    harness.cache.invalidate('item-1')
    const newLoading = harness.cache.load('item-1')
    newGate.resolve(success({
      ...preview('item-1'),
      dataUrl: 'data:image/png;base64,new',
    }))
    await newLoading
    oldGate.resolve(success({
      ...preview('item-1'),
      dataUrl: 'data:image/png;base64,old',
    }))
    await oldLoading

    expect(harness.cache.previews['item-1']).toBe('data:image/png;base64,new')
  })

  it('does not apply an old response after the active batch is cleared', async () => {
    const gate = deferred<AppResult<CaptureItemPreview>>()
    const harness = createHarness(vi.fn().mockReturnValue(gate.promise))

    const loading = harness.cache.load('item-1')
    harness.setActiveBatchId('batch-2')
    harness.cache.clear()
    gate.resolve(success(preview('item-1')))
    await loading

    expect(harness.cache.previews).toEqual({})
  })

  it('rejects a response whose item id does not match the request', async () => {
    const harness = createHarness(vi.fn().mockResolvedValue(success(preview('item-2'))))

    await harness.cache.load('item-1')

    expect(harness.cache.previews).toEqual({})
  })

  it('keeps failures silent and permits a later retry', async () => {
    const fetchPreview = vi.fn()
      .mockResolvedValueOnce(failure('preview_failed', '缩略图失败', true, 'diag-1'))
      .mockResolvedValueOnce(success(preview('item-1')))
    const harness = createHarness(fetchPreview)

    await expect(harness.cache.load('item-1')).resolves.toBeUndefined()
    await harness.cache.load('item-1')

    expect(fetchPreview).toHaveBeenCalledTimes(2)
    expect(harness.cache.previews['item-1']).toBeDefined()
  })

  it('ignores pending and future work after disposal', async () => {
    const gate = deferred<AppResult<CaptureItemPreview>>()
    const harness = createHarness(vi.fn().mockReturnValue(gate.promise))

    const loading = harness.cache.load('item-1')
    harness.cache.dispose()
    gate.resolve(success(preview('item-1')))
    await loading
    await harness.cache.load('item-2')

    expect(harness.fetchPreview).toHaveBeenCalledOnce()
    expect(harness.cache.previews).toEqual({})
  })
})
