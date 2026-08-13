import { computed, getCurrentScope, onScopeDispose, ref, shallowReadonly, watch } from 'vue'
import type { CaptureBatchDetail, CaptureDraftSummary, CaptureDraftUpdateInput } from '../../../shared/api/bindings'
import type { AppResult } from '../../../shared/api/app-result'
import {
  useCaptureDraftSaveQueue,
  type CaptureDraftSaveOutcome,
  type CaptureDraftSaveQueueState,
  type CaptureDraftSaveUpdate,
} from './useCaptureDraftSaveQueue'

export interface CaptureDraftPersistenceOptions {
  desktopAvailable: boolean
  activeDetail: () => CaptureBatchDetail | undefined
  isBlocked: () => boolean
  update: (input: CaptureDraftUpdateInput) => Promise<AppResult<CaptureBatchDetail>>
  refresh: (batchId: string) => Promise<void>
  onDetailChange: (detail: CaptureBatchDetail) => void
  onBusyChange: (busy: boolean) => void
  onSaveStateChange: (state: 'idle' | 'saving' | 'saved' | 'error') => void
  onError: (message: string) => void
}

const transportFailureMessage = '草稿文字保存没有完成；本次编辑仍保留在当前输入框中，请再次修改或重试。'

export function useCaptureDraftPersistence(options: CaptureDraftPersistenceOptions) {
  const state = ref<CaptureDraftSaveQueueState>({ pending: false, running: false, retryRequired: false })
  const unsaved = computed(() => state.value.pending || state.value.running || state.value.retryRequired)
  const retryAvailable = computed(() => state.value.retryRequired)
  const persistenceBusy = computed(() => state.value.pending || state.value.running)

  async function perform(update: CaptureDraftSaveUpdate): Promise<CaptureDraftSaveOutcome> {
    const current = options.activeDetail()
    if (!current || current.batch.id !== update.batchId) {
      return { kind: 'failed', message: '当前采集批次已经切换，本次草稿没有保存。' }
    }
    try {
      const result = await options.update({
        batchId: current.batch.id,
        expectedRevision: current.batch.revision,
        draftId: update.draftId,
        subject: update.subject,
        tags: update.tags,
        note: update.note,
      })
      if (result.ok) {
        if (options.activeDetail()?.batch.id === update.batchId) options.onDetailChange(result.data)
        return { kind: 'saved' }
      }
      return result.error.code === 'capture_revision_conflict'
        ? { kind: 'revision_conflict', message: result.error.userMessage }
        : { kind: 'failed', message: result.error.userMessage }
    }
    catch {
      return { kind: 'failed', message: transportFailureMessage }
    }
  }

  const queue = useCaptureDraftSaveQueue({
    activeBatchId: () => options.activeDetail()?.batch.id,
    isBlocked: options.isBlocked,
    perform,
    refresh: options.refresh,
    onSaving: () => options.onSaveStateChange('saving'),
    onSaved: () => options.onSaveStateChange('saved'),
    onFailed: (message) => {
      options.onSaveStateChange('error')
      options.onError(message)
    },
    onBusyChange: options.onBusyChange,
    onStateChange: (nextState) => {
      state.value = nextState
      if (nextState.retryRequired && !nextState.pending && !nextState.running) {
        options.onSaveStateChange('error')
      }
    },
  })

  function updateDraft(
    draft: Pick<CaptureDraftSummary, 'id'>,
    subject: string,
    tags: string[],
    note: string,
  ) {
    const current = options.activeDetail()
    if (!options.desktopAvailable || !current) return
    queue.enqueue({ batchId: current.batch.id, draftId: draft.id, subject, tags, note })
  }

  async function retry() {
    options.onError('')
    await queue.retry()
  }

  const stopBlockedWatch = watch(options.isBlocked, (blocked) => {
    if (!blocked) void queue.flush()
  })
  const stopBatchWatch = watch(
    () => options.activeDetail()?.batch.id,
    batchId => batchId ? queue.retainBatch(batchId) : queue.clear(),
    { flush: 'sync' },
  )

  function dispose() {
    stopBlockedWatch()
    stopBatchWatch()
    queue.dispose()
  }
  if (getCurrentScope()) onScopeDispose(dispose)

  return {
    state: shallowReadonly(state),
    unsaved,
    retryAvailable,
    persistenceBusy,
    updateDraft,
    retry,
    clear: queue.clear,
    dispose,
  }
}
