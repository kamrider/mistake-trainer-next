import type { InjectionKey } from 'vue'
import type { AppResult } from '../shared/api/app-result'
import type { SyncNowReport } from '../shared/api/bindings'

export type SyncTrigger = 'startup' | 'online' | 'visible' | 'manual'
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
}

type PerformSync = (reason: SyncTrigger) => Promise<AppResult<SyncNowReport>>

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

export function createSyncController(perform: PerformSync): SyncController {
  let inFlight: Promise<AppResult<SyncNowReport>> | undefined

  return {
    run(reason) {
      if (inFlight) return inFlight

      const job = (async () => perform(reason))()
      const tracked = job.finally(() => {
        if (inFlight === tracked) inFlight = undefined
      })
      inFlight = tracked
      return tracked
    },
  }
}

export const syncControllerKey: InjectionKey<SyncController> = Symbol('sync-controller')
