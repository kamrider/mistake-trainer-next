import type { InjectionKey } from 'vue'
import type { AppResult } from '../api/app-result'
import type { SyncNowReport } from '../api/bindings'

export type SyncTrigger = 'startup' | 'online' | 'visible' | 'manual' | 'mutation'

export interface SyncController {
  run: (reason: SyncTrigger) => Promise<AppResult<SyncNowReport>>
  scheduleMutation: () => void
  dispose: () => void
}

export const syncControllerKey: InjectionKey<SyncController> = Symbol('sync-controller')
