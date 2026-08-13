import { computed, ref } from 'vue'
import type {
  BackupRestoreCandidate,
  BackupSummary,
  PortableBackupReceipt,
} from '../../shared/api/bindings'
import type { AppResult } from '../../shared/api/app-result'

export type SettingsBackupPhase = 'idle' | 'creating' | 'creating_portable' | 'preparing' | 'restoring'

interface SettingsBackupOperations {
  create: () => Promise<AppResult<BackupSummary | null>>
  createPortable?: () => Promise<AppResult<PortableBackupReceipt | null>>
  prepareRestore: () => Promise<AppResult<BackupRestoreCandidate | null>>
  preparePortableRestore?: (recoveryKey: string) => Promise<AppResult<BackupRestoreCandidate | null>>
  restore: (candidateId: string) => Promise<AppResult<boolean>>
}

const createFailureMessage =
  '加密备份没有完成，现有资料库未被替换，请检查磁盘空间后重试。'
const prepareFailureMessage =
  '备份包没有验证成功；现有资料库未被修改，请稍后重试。'
const restoreFailureMessage =
  '恢复任务没有开始；当前资料库保持不变，请稍后重试。'

export function useSettingsBackupOperations(operations: SettingsBackupOperations) {
  const phase = ref<SettingsBackupPhase>('idle')
  const busy = computed(() => phase.value !== 'idle')
  const created = ref<BackupSummary>()
  const candidate = ref<BackupRestoreCandidate>()
  const portableReceipt = ref<PortableBackupReceipt>()
  const message = ref('')

  async function createBackup(): Promise<boolean> {
    if (phase.value !== 'idle') return false
    phase.value = 'creating'
    message.value = ''
    try {
      const result = await operations.create()
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
    if (phase.value !== 'idle') return false
    phase.value = 'preparing'
    message.value = ''
    try {
      const result = await operations.prepareRestore()
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
    if (phase.value !== 'idle' || !operations.preparePortableRestore || !recoveryKey.trim()) {
      return false
    }
    phase.value = 'preparing'
    message.value = ''
    try {
      const result = await operations.preparePortableRestore(recoveryKey.trim())
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
    if (phase.value !== 'idle' || !operations.createPortable) return false
    phase.value = 'creating_portable'
    message.value = ''
    portableReceipt.value = undefined
    try {
      const result = await operations.createPortable()
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
    if (phase.value !== 'idle' || !selectedCandidate) return false
    phase.value = 'restoring'
    message.value = ''
    try {
      const result = await operations.restore(selectedCandidate.id)
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

  return {
    phase,
    busy,
    created,
    candidate,
    portableReceipt,
    message,
    clearMessage: () => { message.value = '' },
    createBackup,
    createPortableBackup,
    clearPortableReceipt: () => { portableReceipt.value = undefined },
    prepareRestore,
    preparePortableRestore,
    restoreBackup,
  }
}
