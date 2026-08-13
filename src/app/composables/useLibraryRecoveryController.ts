import { readonly, ref, type Ref } from 'vue'
import type { AppResult } from '../../shared/api/app-result'
import type { BackupRestoreCandidate } from '../../shared/api/bindings'
import { createRecoverySingleFlight } from '../recovery-single-flight'

export interface LibraryRecoveryControllerOptions {
  reconnect: () => Promise<AppResult<boolean>>
  prepareRestore: () => Promise<AppResult<BackupRestoreCandidate | null>>
  restore: (candidateId: string) => Promise<AppResult<boolean>>
  startFresh: (confirmation: string) => Promise<AppResult<boolean>>
  enterRestarting: () => void
}

export interface LibraryRecoveryController {
  busy: Readonly<Ref<boolean>>
  message: Readonly<Ref<string>>
  candidate: Readonly<Ref<BackupRestoreCandidate | undefined>>
  restoreDialogOpen: Readonly<Ref<boolean>>
  freshStartDialogOpen: Readonly<Ref<boolean>>
  openFreshStartDialog: () => void
  closeFreshStartDialog: () => void
  closeRestoreDialog: () => void
  reconnectLibrary: () => Promise<boolean>
  prepareRecoveryBackup: () => Promise<boolean>
  confirmRecoveryBackup: () => Promise<boolean>
  confirmFreshStart: (confirmation: string) => Promise<boolean>
}

export function useLibraryRecoveryController(
  options: LibraryRecoveryControllerOptions,
): LibraryRecoveryController {
  const busy = ref(false)
  const message = ref('')
  const candidate = ref<BackupRestoreCandidate>()
  const restoreDialogOpen = ref(false)
  const freshStartDialogOpen = ref(false)
  const runSingleFlight = createRecoverySingleFlight()

  function runRecovery(operation: () => Promise<boolean>): Promise<boolean> {
    return runSingleFlight(async () => {
      busy.value = true
      message.value = ''
      try {
        return await operation()
      }
      catch {
        message.value = '恢复操作没有完成，原资料库状态没有被覆盖，请稍后重试。'
        return false
      }
      finally {
        busy.value = false
      }
    })
  }

  function reconnectLibrary() {
    return runRecovery(async () => {
      const result = await options.reconnect()
      if (!result.ok) {
        message.value = result.error.userMessage
        return false
      }
      if (result.data) options.enterRestarting()
      return result.data
    })
  }

  function prepareRecoveryBackup() {
    return runRecovery(async () => {
      const result = await options.prepareRestore()
      if (!result.ok) {
        message.value = result.error.userMessage
        return false
      }
      if (!result.data) return false
      candidate.value = result.data
      restoreDialogOpen.value = true
      return true
    })
  }

  function confirmRecoveryBackup() {
    const selectedCandidate = candidate.value
    if (!selectedCandidate) return Promise.resolve(false)
    return runRecovery(async () => {
      const result = await options.restore(selectedCandidate.id)
      if (!result.ok) {
        message.value = result.error.userMessage
        return false
      }
      if (result.data) {
        restoreDialogOpen.value = false
        options.enterRestarting()
      }
      return result.data
    })
  }

  function confirmFreshStart(confirmation: string) {
    return runRecovery(async () => {
      const result = await options.startFresh(confirmation)
      if (!result.ok) {
        message.value = result.error.userMessage
        return false
      }
      if (result.data) {
        freshStartDialogOpen.value = false
        options.enterRestarting()
      }
      return result.data
    })
  }

  return {
    busy: readonly(busy),
    message: readonly(message),
    candidate: readonly(candidate),
    restoreDialogOpen: readonly(restoreDialogOpen),
    freshStartDialogOpen: readonly(freshStartDialogOpen),
    openFreshStartDialog() {
      message.value = ''
      freshStartDialogOpen.value = true
    },
    closeFreshStartDialog() {
      freshStartDialogOpen.value = false
    },
    closeRestoreDialog() {
      restoreDialogOpen.value = false
    },
    reconnectLibrary,
    prepareRecoveryBackup,
    confirmRecoveryBackup,
    confirmFreshStart,
  }
}
