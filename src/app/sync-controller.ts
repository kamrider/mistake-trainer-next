import type { InjectionKey } from 'vue'
import type { AppResult } from '../shared/api/app-result'
import type { SyncNowReport } from '../shared/api/bindings'

export type SyncTrigger = 'startup' | 'online' | 'visible' | 'manual' | 'mutation'
export type SyncPhase =
  | 'local_only'
  | 'signed_out'
  | 'offline'
  | 'idle'
  | 'syncing'
  | 'synced'
  | 'deferred_capture'
  | 'retry_waiting'

export type SyncStatusTone = 'neutral' | 'active' | 'success' | 'waiting' | 'warning'

export interface SyncStatusCopy {
  label: string
  tone: SyncStatusTone
}

export interface SyncController {
  run: (reason: SyncTrigger) => Promise<AppResult<SyncNowReport>>
  scheduleMutation: () => void
  dispose: () => void
}

type PerformSync = (reason: SyncTrigger) => Promise<AppResult<SyncNowReport>>

export interface SyncControllerOptions {
  mutationDebounceMs?: number
  canScheduleMutation?: () => boolean
}

const statusCopy: Record<SyncPhase, SyncStatusCopy> = {
  local_only: { label: '本地资料库 · 未启用同步', tone: 'neutral' },
  signed_out: { label: '本地已保存 · 云端未登录', tone: 'neutral' },
  offline: { label: '本地已保存 · 当前离线', tone: 'waiting' },
  idle: { label: '本地已保存 · 等待同步', tone: 'neutral' },
  syncing: { label: '正在安全同步', tone: 'active' },
  synced: { label: '本地与云端已同步', tone: 'success' },
  deferred_capture: { label: '手机采集中 · 稍后同步', tone: 'waiting' },
  retry_waiting: { label: '本地已保存 · 等待重试', tone: 'warning' },
}

export function syncStatusCopy(phase: SyncPhase): SyncStatusCopy {
  return statusCopy[phase]
}

export function createSyncController(
  perform: PerformSync,
  options: SyncControllerOptions = {},
): SyncController {
  let inFlight: Promise<AppResult<SyncNowReport>> | undefined
  let mutationTimer: ReturnType<typeof globalThis.setTimeout> | undefined
  let mutationPending = false
  let flushingMutations = false
  let disposed = false
  const mutationDebounceMs = Math.min(10_000, Math.max(0, options.mutationDebounceMs ?? 1_200))
  const canScheduleMutation = options.canScheduleMutation ?? (() => true)

  function clearMutationTimer() {
    if (mutationTimer === undefined) return
    globalThis.clearTimeout(mutationTimer)
    mutationTimer = undefined
  }

  function armMutationTimer() {
    clearMutationTimer()
    mutationTimer = globalThis.setTimeout(() => {
      mutationTimer = undefined
      void flushMutationQueue()
    }, mutationDebounceMs)
  }

  async function flushMutationQueue() {
    if (disposed || flushingMutations || !mutationPending) return
    flushingMutations = true

    try {
      mutationPending = false
      const olderRequest = inFlight
      if (olderRequest) {
        try {
          await olderRequest
        }
        catch {
          mutationPending = false
          return
        }
      }
      if (disposed || !canScheduleMutation()) {
        mutationPending = false
        return
      }

      // Mutations committed while waiting for the older request are covered by the
      // fresh pass that starts below. Only mutations arriving during this pass remain
      // dirty and receive another debounced pass.
      mutationPending = false
      let result: AppResult<SyncNowReport>
      try {
        result = await run('mutation')
      }
      catch {
        mutationPending = false
        return
      }
      if (!result.ok) {
        mutationPending = false
        clearMutationTimer()
      }
    }
    finally {
      flushingMutations = false
      if (mutationPending && !disposed && canScheduleMutation()) armMutationTimer()
    }
  }

  function run(reason: SyncTrigger) {
    if (inFlight) return inFlight

    const job = (async () => perform(reason))()
    const tracked = job.finally(() => {
      if (inFlight === tracked) inFlight = undefined
    })
    inFlight = tracked
    return tracked
  }

  return {
    run,
    scheduleMutation() {
      if (disposed || !canScheduleMutation()) return
      mutationPending = true
      armMutationTimer()
    },
    dispose() {
      disposed = true
      mutationPending = false
      clearMutationTimer()
    },
  }
}

export const syncControllerKey: InjectionKey<SyncController> = Symbol('sync-controller')
