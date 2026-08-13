import { shallowReadonly, ref } from 'vue'
import type { CaptureBatchDetail, CaptureBatchSummary } from '../../../shared/api/bindings'
import type { AppResult } from '../../../shared/api/app-result'

export interface CaptureBatchDataOptions {
  desktopAvailable: boolean
  list: () => Promise<AppResult<CaptureBatchSummary[]>>
  detail: (batchId: string) => Promise<AppResult<CaptureBatchDetail>>
  onError: (message: string) => void
  routeBatchId: () => string | undefined
  replaceRouteBatchId: (batchId: string) => void
}

export function useCaptureBatchData(options: CaptureBatchDataOptions) {
  const batches = ref<CaptureBatchSummary[]>([])
  const detail = ref<CaptureBatchDetail>()
  const requestedBatchId = ref('')
  let onDetailRequested: (batchId: string) => void = () => undefined

  function setDetailRequestedHandler(handler: (batchId: string) => void) {
    onDetailRequested = handler
  }

  async function loadBatches() {
    if (!options.desktopAvailable) return
    try {
      const result = await options.list()
      if (result.ok) batches.value = result.data
      else options.onError(result.error.userMessage)
    }
    catch {
      options.onError('采集箱连接中断，请重新打开应用后重试。')
    }
  }

  async function loadDetail(batchId: string) {
    if (!options.desktopAvailable) return
    requestedBatchId.value = batchId
    try {
      const detailRequest = options.detail(batchId)
      onDetailRequested(batchId)
      const result = await detailRequest
      if (requestedBatchId.value !== batchId) return
      if (result.ok) {
        detail.value = result.data
        if (options.routeBatchId() !== batchId) options.replaceRouteBatchId(batchId)
      }
      else options.onError(result.error.userMessage)
    }
    catch {
      if (requestedBatchId.value === batchId) {
        options.onError('没有读取到这个采集批次，请返回后重试。')
      }
    }
  }

  function replaceDetail(value: CaptureBatchDetail | undefined) {
    detail.value = value
  }

  function clearDetail() {
    requestedBatchId.value = ''
    detail.value = undefined
  }

  function hydrateDevelopment(value: {
    batches: CaptureBatchSummary[]
    detail: CaptureBatchDetail | undefined
  }) {
    batches.value = value.batches
    detail.value = value.detail
  }

  return {
    batches: shallowReadonly(batches),
    detail: shallowReadonly(detail),
    requestedBatchId: shallowReadonly(requestedBatchId),
    setDetailRequestedHandler,
    loadBatches,
    loadDetail,
    replaceDetail,
    clearDetail,
    hydrateDevelopment,
  }
}
