import { reactive } from 'vue'
import type { CaptureItemPreview } from '../../../shared/api/bindings'
import type { AppResult } from '../../../shared/api/app-result'

export interface CapturePreviewCache {
  previews: Record<string, string>
  load: (itemId: string) => Promise<void>
  invalidate: (itemId: string) => void
  clear: () => void
  dispose: () => void
}

interface CapturePreviewCacheOptions {
  activeBatchId: () => string | undefined
  fetchPreview: (
    batchId: string,
    itemId: string,
  ) => Promise<AppResult<CaptureItemPreview>>
  maxEntries?: number
}

interface PreviewRequest {
  batchId: string
  epoch: number
  promise: Promise<void>
}

export function useCapturePreviewCache(
  options: CapturePreviewCacheOptions,
): CapturePreviewCache {
  const previews = reactive<Record<string, string>>({})
  const order: string[] = []
  const requests = new Map<string, PreviewRequest>()
  const requestedMaxEntries = options.maxEntries ?? 40
  const maxEntries = Number.isFinite(requestedMaxEntries) && requestedMaxEntries >= 1
    ? Math.floor(requestedMaxEntries)
    : 40
  let epoch = 0
  let disposed = false

  function removeFromOrder(itemId: string) {
    for (let index = order.length - 1; index >= 0; index -= 1) {
      if (order[index] === itemId) order.splice(index, 1)
    }
  }

  function invalidateCachedValue(itemId: string) {
    delete previews[itemId]
    removeFromOrder(itemId)
  }

  function touch(itemId: string) {
    removeFromOrder(itemId)
    order.push(itemId)
  }

  function enforceLimit() {
    while (order.length > maxEntries) {
      const expired = order.shift()
      if (expired) delete previews[expired]
    }
  }

  async function runRequest(
    itemId: string,
    request: PreviewRequest,
  ) {
    try {
      const result = await options.fetchPreview(request.batchId, itemId)
      if (
        disposed
        || request.epoch !== epoch
        || requests.get(itemId) !== request
        || options.activeBatchId() !== request.batchId
        || !result.ok
        || result.data.itemId !== itemId
      ) {
        return
      }
      invalidateCachedValue(itemId)
      previews[itemId] = result.data.dataUrl
      touch(itemId)
      enforceLimit()
    }
    catch {
      // A failed thumbnail must not interrupt organizing the rest of the batch.
    }
    finally {
      if (requests.get(itemId) === request) requests.delete(itemId)
    }
  }

  async function load(itemId: string) {
    if (disposed) return
    const batchId = options.activeBatchId()
    if (!batchId) return
    if (Object.prototype.hasOwnProperty.call(previews, itemId)) {
      touch(itemId)
      return
    }
    const existing = requests.get(itemId)
    if (
      existing
      && existing.batchId === batchId
      && existing.epoch === epoch
    ) {
      return existing.promise
    }

    const request: PreviewRequest = {
      batchId,
      epoch,
      promise: Promise.resolve(),
    }
    requests.set(itemId, request)
    request.promise = runRequest(itemId, request)
    return request.promise
  }

  function invalidate(itemId: string) {
    requests.delete(itemId)
    invalidateCachedValue(itemId)
  }

  function clear() {
    epoch += 1
    requests.clear()
    order.splice(0)
    for (const itemId of Object.keys(previews)) delete previews[itemId]
  }

  function dispose() {
    if (disposed) return
    disposed = true
    clear()
  }

  return {
    previews,
    load,
    invalidate,
    clear,
    dispose,
  }
}
