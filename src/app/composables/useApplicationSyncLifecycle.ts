import { readonly, ref, type Ref } from 'vue'
import { failure, type AppResult } from '../../shared/api/app-result'
import type { CloudAuthState, SyncNowReport } from '../../shared/api/bindings'
import type { SyncController, SyncTrigger } from '../../shared/contracts/sync-controller'
import type { AppPage } from '../AppShell.vue'
import { createSyncController, type SyncPhase } from '../sync-controller'
import type { LibraryAccessPhase } from './useLibraryAccessLifecycle'

const automaticSyncCooldownMs = 15_000
const networkFailureCodes = new Set([
  'AUTH_NETWORK',
  'AUTH_TIMEOUT',
  'cloud_network',
  'cloud_timeout',
  'cloud_unavailable',
])
const mutationSyncPhases = new Set<SyncPhase>([
  'idle',
  'syncing',
  'synced',
  'deferred_capture',
  'retry_waiting',
])

export interface ApplicationSyncLifecycleOptions {
  desktopRuntime: boolean
  libraryAccessPhase: Readonly<Ref<LibraryAccessPhase>>
  workspaceInitialized: Readonly<Ref<boolean>>
  activePage: Readonly<Ref<AppPage>>
  restoreSession: () => Promise<AppResult<CloudAuthState>>
  syncNow: () => Promise<AppResult<SyncNowReport>>
  onSyncSuccess: (report: SyncNowReport, reason: SyncTrigger) => Promise<void>
  now?: () => number
}

export interface ApplicationSyncLifecycle {
  phase: Readonly<Ref<SyncPhase>>
  controller: SyncController
  restoreCloudAndSync: (reason: SyncTrigger) => Promise<void>
  start: () => void
  dispose: () => void
}

export function useApplicationSyncLifecycle(
  options: ApplicationSyncLifecycleOptions,
): ApplicationSyncLifecycle {
  const phase = ref<SyncPhase>('local_only')
  const now = options.now ?? Date.now
  let lastSuccessfulSyncAtUtcMs = 0
  let restoreTask: Promise<void> | undefined
  let started = false
  let disposed = false

  async function performSync(reason: SyncTrigger): Promise<AppResult<SyncNowReport>> {
    phase.value = 'syncing'
    try {
      const result = await options.syncNow()
      if (!result.ok) {
        if (result.error.code === 'SYNC_CAPTURE_ACTIVE') phase.value = 'deferred_capture'
        else if (result.error.code === 'SYNC_ALREADY_RUNNING') phase.value = 'syncing'
        else if (networkFailureCodes.has(result.error.code)) phase.value = 'offline'
        else phase.value = 'retry_waiting'
        return result
      }

      phase.value = 'synced'
      lastSuccessfulSyncAtUtcMs = now()
      await options.onSyncSuccess(result.data, reason)
      return result
    }
    catch {
      phase.value = 'offline'
      return failure(
        'SYNC_REQUEST_FAILED',
        '暂时无法连接云端，本地内容已经保存并会等待重试。',
        true,
        'sync-request-failed',
      )
    }
  }

  const controller = createSyncController(performSync, {
    canScheduleMutation: () => mutationSyncPhases.has(phase.value),
  })

  async function runCloudRestoreAndSync(reason: SyncTrigger) {
    try {
      const result = await options.restoreSession()
      if (!result.ok) {
        phase.value = networkFailureCodes.has(result.error.code) ? 'offline' : 'retry_waiting'
        return
      }

      switch (result.data.status.kind) {
        case 'connected':
          phase.value = 'idle'
          await controller.run(reason)
          return
        case 'offline':
          phase.value = 'offline'
          return
        case 'unconfigured':
          phase.value = 'local_only'
          return
        case 'signed_out':
        case 'verification_required':
          phase.value = 'signed_out'
          return
      }
    }
    catch {
      phase.value = 'offline'
    }
  }

  function restoreCloudAndSync(reason: SyncTrigger): Promise<void> {
    if (restoreTask) return restoreTask
    if (!options.desktopRuntime) {
      phase.value = 'local_only'
      return Promise.resolve()
    }
    if (
      reason === 'visible'
      && lastSuccessfulSyncAtUtcMs > 0
      && now() - lastSuccessfulSyncAtUtcMs < automaticSyncCooldownMs
    ) {
      return Promise.resolve()
    }
    if (typeof navigator !== 'undefined' && navigator.onLine === false) {
      phase.value = 'offline'
      return Promise.resolve()
    }

    const job = runCloudRestoreAndSync(reason)
    const tracked = job.finally(() => {
      if (restoreTask === tracked) restoreTask = undefined
    })
    restoreTask = tracked
    return tracked
  }

  function handleOnline() {
    if (
      options.libraryAccessPhase.value !== 'unlocked'
      || !options.workspaceInitialized.value
    ) return
    void restoreCloudAndSync('online')
  }

  function handleVisibilityChange() {
    if (
      document.visibilityState !== 'visible'
      || options.libraryAccessPhase.value !== 'unlocked'
      || !options.workspaceInitialized.value
    ) return
    void restoreCloudAndSync('visible')
  }

  function start() {
    if (started || disposed) return
    started = true
    window.addEventListener('online', handleOnline)
    document.addEventListener('visibilitychange', handleVisibilityChange)
  }

  function dispose() {
    if (disposed) return
    disposed = true
    if (started) {
      window.removeEventListener('online', handleOnline)
      document.removeEventListener('visibilitychange', handleVisibilityChange)
    }
    controller.dispose()
  }

  return {
    phase: readonly(phase),
    controller,
    restoreCloudAndSync,
    start,
    dispose,
  }
}
