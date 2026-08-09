import type {
  CaptureBatchCreateInput,
  CaptureBatchDetail,
  CaptureBatchSummary,
  CaptureBatchUpdateInput,
  CaptureCommitReport,
} from '../../../shared/api/bindings'
import type { AppResult } from '../../../shared/api/app-result'

interface CaptureBatchLifecycleOperations {
  create: (input: CaptureBatchCreateInput) => Promise<AppResult<CaptureBatchSummary>>
  discard: (batchId: string) => Promise<AppResult<boolean>>
  update: (input: CaptureBatchUpdateInput) => Promise<AppResult<CaptureBatchSummary>>
  commit: (batchId: string, expectedRevision: number) => Promise<AppResult<CaptureCommitReport>>
}

interface CaptureBatchLifecycleOptions {
  desktopAvailable: boolean
  activeDetail: () => CaptureBatchDetail | undefined
  activeLanBatchId: () => string | undefined
  isBlocked: () => boolean
  onBusyChange: (busy: boolean) => void
  onError: (message: string) => void
  onCommitMessage: (message: string) => void
  onActiveBatchDiscarded: (batchId: string) => void
  loadBatches: () => Promise<void>
  loadDetail: (batchId: string) => Promise<void>
  stopMobileCapture: (silent?: boolean) => Promise<void>
  scheduleSyncMutation: () => void
  operations: CaptureBatchLifecycleOperations
}

interface BatchIdentity {
  id: string
  revision: number
}

export function useCaptureBatchLifecycle(options: CaptureBatchLifecycleOptions) {
  function activeIdentity(): BatchIdentity | undefined {
    const batch = options.activeDetail()?.batch
    return batch ? { id: batch.id, revision: batch.revision } : undefined
  }

  function isCurrent(identity: BatchIdentity) {
    const active = options.activeDetail()?.batch
    return active?.id === identity.id && active.revision === identity.revision
  }

  async function createBatch(subject: string) {
    if (!options.desktopAvailable || options.isBlocked()) return
    options.onBusyChange(true)
    options.onError('')
    try {
      const result = await options.operations.create({ subject })
      if (result.ok) {
        await options.loadBatches()
        await options.loadDetail(result.data.id)
      }
      else options.onError(result.error.userMessage)
    }
    catch {
      options.onError('新批次没有创建成功，请稍后重试。')
    }
    finally {
      options.onBusyChange(false)
    }
  }

  async function discardBatch(batchId: string) {
    if (!options.desktopAvailable || options.isBlocked()) return
    options.onBusyChange(true)
    try {
      if (options.activeLanBatchId() === batchId) {
        await options.stopMobileCapture(true)
      }
      const result = await options.operations.discard(batchId)
      if (result.ok) {
        if (options.activeDetail()?.batch.id === batchId) {
          options.onActiveBatchDiscarded(batchId)
        }
        await options.loadBatches()
      }
      else options.onError(result.error.userMessage)
    }
    catch {
      options.onError('批次没有删除成功，原有图片仍会保留。')
    }
    finally {
      options.onBusyChange(false)
    }
  }

  async function finishCollecting(subject: string) {
    const identity = activeIdentity()
    if (!options.desktopAvailable || !identity || options.isBlocked()) return
    options.onBusyChange(true)
    try {
      if (options.activeLanBatchId() === identity.id) {
        await options.stopMobileCapture(true)
      }
      const result = await options.operations.update({
        batchId: identity.id,
        expectedRevision: identity.revision,
        subject,
        finishCollecting: true,
      })
      if (result.ok) {
        if (isCurrent(identity)) await options.loadDetail(identity.id)
        else await options.loadBatches()
      }
      else if (isCurrent(identity)) options.onError(result.error.userMessage)
    }
    catch {
      if (isCurrent(identity)) options.onError('没有结束采集，请稍后重试。')
    }
    finally {
      options.onBusyChange(false)
    }
  }

  async function commitReady() {
    const identity = activeIdentity()
    if (!options.desktopAvailable || !identity || options.isBlocked()) return
    options.onBusyChange(true)
    options.onCommitMessage('')
    try {
      const result = await options.operations.commit(identity.id, identity.revision)
      if (result.ok) {
        if (result.data.committedCount > 0) options.scheduleSyncMutation()
        if (isCurrent(identity)) {
          options.onCommitMessage(result.data.committedCount
            ? `已将 ${result.data.committedCount} 道题加入题库。`
            : '没有可加入题库的完整题卡。')
          await options.loadDetail(identity.id)
        }
        await options.loadBatches()
      }
      else if (isCurrent(identity)) options.onError(result.error.userMessage)
    }
    catch {
      if (isCurrent(identity)) {
        options.onError('批量入库没有完成，所有草稿仍保持原样，可以直接重试。')
      }
    }
    finally {
      options.onBusyChange(false)
    }
  }

  return { createBatch, discardBatch, finishCollecting, commitReady }
}
