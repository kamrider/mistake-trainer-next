import { computed, readonly, ref } from 'vue'
import type { AppResult } from '../../shared/api/app-result'
import type {
  AutomaticBackupStatus,
  BackupRestoreCandidate,
  BackupSummary,
  PortableBackupReceipt,
} from '../../shared/api/bindings'

export type SettingsBackupPhase =
  | 'idle'
  | 'creating'
  | 'creating_portable'
  | 'preparing'
  | 'restoring'
  | 'automatic'

export interface SettingsBackupOperations {
  create: () => Promise<AppResult<BackupSummary | null>>
  createPortable?: () => Promise<AppResult<PortableBackupReceipt | null>>
  prepareRestore: () => Promise<AppResult<BackupRestoreCandidate | null>>
  preparePortableRestore?: (recoveryKey: string) => Promise<AppResult<BackupRestoreCandidate | null>>
  restore: (candidateId: string) => Promise<AppResult<boolean>>
}

export interface SettingsAutomaticBackupOperations {
  status: () => Promise<AppResult<AutomaticBackupStatus>>
  configure: (intervalDays: number, retentionCount: number) => Promise<AppResult<AutomaticBackupStatus | null>>
  disable: () => Promise<AppResult<AutomaticBackupStatus>>
}

export interface SettingsBackupControllerOptions {
  operations: SettingsBackupOperations
  automatic: SettingsAutomaticBackupOperations
  restoreFocus: () => Promise<unknown> | unknown
  onOperationStart: () => void
}

const createFailureMessage =
  '加密备份没有完成，现有资料库未被替换，请检查磁盘空间后重试。'
const prepareFailureMessage =
  '备份包没有验证成功；现有资料库未被修改，请稍后重试。'
const restoreFailureMessage =
  '恢复任务没有开始；当前资料库保持不变，请稍后重试。'

export function useSettingsBackupOperations(options: SettingsBackupControllerOptions) {
  const phase = ref<SettingsBackupPhase>('idle')
  const busy = computed(() => phase.value !== 'idle')
  const automaticBusy = computed(() => phase.value === 'automatic')
  const navigationBusy = busy
  const created = ref<BackupSummary>()
  const candidate = ref<BackupRestoreCandidate>()
  const portableReceipt = ref<PortableBackupReceipt>()
  const automaticStatus = ref<AutomaticBackupStatus>()
  const restoreDialogOpen = ref(false)
  const message = ref('')

  function begin(nextPhase: Exclude<SettingsBackupPhase, 'idle'>): boolean {
    if (phase.value !== 'idle') return false
    phase.value = nextPhase
    message.value = ''
    options.onOperationStart()
    return true
  }

  async function createBackup(): Promise<boolean> {
    if (!begin('creating')) return false
    try {
      const result = await options.operations.create()
      if (!result.ok) {
        message.value = result.error.userMessage
        return false
      }
      if (!result.data) return false
      created.value = result.data
      return true
    }
    catch {
      message.value = createFailureMessage
      return false
    }
    finally {
      phase.value = 'idle'
    }
  }

  async function prepareRestore(): Promise<boolean> {
    if (!begin('preparing')) return false
    try {
      const result = await options.operations.prepareRestore()
      if (!result.ok) {
        candidate.value = undefined
        message.value = result.error.userMessage
        return false
      }
      if (!result.data) return false
      candidate.value = result.data
      return true
    }
    catch {
      candidate.value = undefined
      message.value = prepareFailureMessage
      return false
    }
    finally {
      phase.value = 'idle'
    }
  }

  async function preparePortableRestore(recoveryKey: string): Promise<boolean> {
    if (!recoveryKey.trim() || !options.operations.preparePortableRestore || !begin('preparing')) {
      return false
    }
    try {
      const result = await options.operations.preparePortableRestore(recoveryKey.trim())
      if (!result.ok) {
        candidate.value = undefined
        message.value = result.error.userMessage
        return false
      }
      if (!result.data) return false
      candidate.value = result.data
      return true
    }
    catch {
      candidate.value = undefined
      message.value = prepareFailureMessage
      return false
    }
    finally {
      phase.value = 'idle'
    }
  }

  async function createPortableBackup(): Promise<boolean> {
    if (!options.operations.createPortable || !begin('creating_portable')) return false
    portableReceipt.value = undefined
    try {
      const result = await options.operations.createPortable()
      if (!result.ok) {
        message.value = result.error.userMessage
        return false
      }
      if (!result.data) return false
      portableReceipt.value = result.data
      created.value = result.data.summary
      return true
    }
    catch {
      message.value = createFailureMessage
      return false
    }
    finally {
      phase.value = 'idle'
    }
  }

  async function restoreBackup(): Promise<boolean> {
    const selectedCandidate = candidate.value
    if (!selectedCandidate || !begin('restoring')) return false
    try {
      const result = await options.operations.restore(selectedCandidate.id)
      if (!result.ok) {
        message.value = result.error.userMessage
        phase.value = 'idle'
        return false
      }
      if (!result.data) {
        message.value = restoreFailureMessage
        phase.value = 'idle'
        return false
      }
      return true
    }
    catch {
      message.value = restoreFailureMessage
      phase.value = 'idle'
      return false
    }
  }

  async function loadAutomaticStatus(): Promise<boolean> {
    try {
      const result = await options.automatic.status()
      if (!result.ok) return false
      if (result.data) automaticStatus.value = result.data
      return true
    }
    catch {
      return false
    }
  }

  async function configureAutomaticBackup(
    intervalDays: number,
    retentionCount: number,
  ): Promise<boolean> {
    if (!begin('automatic')) return false
    try {
      const result = await options.automatic.configure(intervalDays, retentionCount)
      if (!result.ok) {
        message.value = result.error.userMessage
        return false
      }
      if (result.data) automaticStatus.value = result.data
      return true
    }
    catch {
      message.value = '自动备份设置没有更新；现有备份和资料库保持不变。'
      return false
    }
    finally {
      phase.value = 'idle'
    }
  }

  async function disableAutomaticBackup(): Promise<boolean> {
    if (!begin('automatic')) return false
    try {
      const result = await options.automatic.disable()
      if (!result.ok) {
        message.value = result.error.userMessage
        return false
      }
      automaticStatus.value = result.data
      return true
    }
    catch {
      message.value = '自动备份没有停用；请稍后重试。'
      return false
    }
    finally {
      phase.value = 'idle'
    }
  }

  function openRestoreDialog() {
    if (busy.value || !candidate.value) return
    restoreDialogOpen.value = true
  }

  async function closeRestoreDialog(): Promise<boolean> {
    if (busy.value) return false
    restoreDialogOpen.value = false
    await options.restoreFocus()
    return true
  }

  async function confirmRestore(): Promise<boolean> {
    const started = await restoreBackup()
    if (!started) await closeRestoreDialog()
    return started
  }

  return {
    phase: readonly(phase),
    busy,
    automaticBusy,
    navigationBusy,
    created: readonly(created),
    candidate: readonly(candidate),
    portableReceipt: readonly(portableReceipt),
    automaticStatus: readonly(automaticStatus),
    restoreDialogOpen: readonly(restoreDialogOpen),
    message: readonly(message),
    clearMessage: () => { message.value = '' },
    createBackup,
    createPortableBackup,
    clearPortableReceipt: () => { portableReceipt.value = undefined },
    prepareRestore,
    preparePortableRestore,
    restoreBackup,
    loadAutomaticStatus,
    configureAutomaticBackup,
    disableAutomaticBackup,
    openRestoreDialog,
    closeRestoreDialog,
    confirmRestore,
  }
}
