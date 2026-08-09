export interface CaptureDraftSaveUpdate {
  batchId: string
  draftId: string
  subject: string
  tags: string[]
  note: string
}

export type CaptureDraftSaveOutcome =
  | { kind: 'saved' }
  | { kind: 'revision_conflict', message: string }
  | { kind: 'failed', message: string }

export interface CaptureDraftSaveQueueState {
  pending: boolean
  running: boolean
  retryRequired: boolean
}

export interface CaptureDraftSaveQueue {
  enqueue: (update: CaptureDraftSaveUpdate) => void
  flush: () => Promise<void>
  retry: () => Promise<void>
  retainBatch: (batchId: string) => void
  clear: () => void
  dispose: () => void
}

interface PendingDraftSave extends CaptureDraftSaveUpdate {
  attempts: number
  generation: number
}

interface CaptureDraftSaveQueueOptions {
  activeBatchId: () => string | undefined
  isBlocked: () => boolean
  perform: (update: CaptureDraftSaveUpdate) => Promise<CaptureDraftSaveOutcome>
  refresh: (batchId: string) => Promise<void>
  onSaving: () => void
  onSaved: () => void
  onFailed: (message: string) => void
  onBusyChange: (busy: boolean) => void
  onStateChange: (state: CaptureDraftSaveQueueState) => void
  unexpectedErrorMessage?: string
}

const defaultUnexpectedError = '草稿文字保存没有完成；本次编辑仍保留在当前输入框中，请再次修改或重试。'

function updateKey(update: Pick<CaptureDraftSaveUpdate, 'batchId' | 'draftId'>) {
  return `${update.batchId}:${update.draftId}`
}

export function useCaptureDraftSaveQueue(
  options: CaptureDraftSaveQueueOptions,
): CaptureDraftSaveQueue {
  const pending = new Map<string, PendingDraftSave>()
  const failed = new Map<string, PendingDraftSave>()
  let running = false
  let disposed = false
  let generation = 0

  function publishState() {
    options.onStateChange({
      pending: pending.size > 0,
      running,
      retryRequired: failed.size > 0,
    })
  }

  function retainBatch(batchId: string) {
    for (const [key, update] of pending) {
      if (update.batchId !== batchId) pending.delete(key)
    }
    for (const [key, update] of failed) {
      if (update.batchId !== batchId) failed.delete(key)
    }
    publishState()
  }

  function clear() {
    pending.clear()
    failed.clear()
    publishState()
  }

  function dispose() {
    disposed = true
    clear()
  }

  function enqueue(update: CaptureDraftSaveUpdate) {
    if (disposed) return
    const key = updateKey(update)
    failed.delete(key)
    pending.set(key, {
      ...update,
      tags: [...update.tags],
      attempts: 0,
      generation: ++generation,
    })
    publishState()
    void flush()
  }

  function rememberFailure(
    key: string,
    update: PendingDraftSave,
    message: string,
  ) {
    if (!pending.has(key)) failed.set(key, update)
    options.onFailed(message)
  }

  async function flush() {
    if (disposed || running || options.isBlocked()) return
    const activeBatchId = options.activeBatchId()
    if (!activeBatchId) return
    retainBatch(activeBatchId)
    const queued = [...pending.entries()].find(([, update]) => update.batchId === activeBatchId)
    if (!queued) return

    const [key, update] = queued
    pending.delete(key)
    running = true
    options.onBusyChange(true)
    options.onSaving()
    publishState()
    try {
      const outcome = await options.perform(update)
      if (disposed || options.activeBatchId() !== update.batchId) return
      if (outcome.kind === 'saved') {
        options.onSaved()
      }
      else if (outcome.kind === 'revision_conflict') {
        if (update.attempts < 1) {
          await options.refresh(update.batchId)
          if (
            !disposed
            && options.activeBatchId() === update.batchId
            && !pending.has(key)
          ) {
            pending.set(key, {
              ...update,
              attempts: update.attempts + 1,
            })
          }
        }
        else {
          rememberFailure(key, update, outcome.message)
        }
      }
      else if (outcome.kind === 'failed') {
        rememberFailure(key, update, outcome.message)
      }
    }
    catch {
      if (!disposed) {
        rememberFailure(
          key,
          update,
          options.unexpectedErrorMessage ?? defaultUnexpectedError,
        )
      }
    }
    finally {
      running = false
      options.onBusyChange(false)
      publishState()
    }

    if (!disposed && pending.size && !options.isBlocked()) void flush()
  }

  async function retry() {
    if (disposed) return
    const activeBatchId = options.activeBatchId()
    if (!activeBatchId) return
    let moved = false
    for (const [key, update] of failed) {
      if (update.batchId !== activeBatchId) continue
      failed.delete(key)
      moved = true
      pending.set(key, {
        ...update,
        attempts: 0,
        generation: ++generation,
      })
    }
    publishState()
    if (moved) await flush()
  }

  publishState()

  return {
    enqueue,
    flush,
    retry,
    retainBatch,
    clear,
    dispose,
  }
}
