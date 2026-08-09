import { readonly, ref, shallowReadonly, shallowRef } from 'vue'
import type { AppResult } from '../../../shared/api/app-result'
import type { SyncConflictSummary } from '../../../shared/api/bindings'

interface SyncConflictOperationsOptions {
  listConflicts: () => Promise<AppResult<SyncConflictSummary[]>>
  scheduleSync: () => void
  onChanged: () => void
}

type ConflictResolution = () => Promise<AppResult<SyncConflictSummary[]>>

const listFailureMessage = '同步冲突暂时无法读取；本机与云端内容都没有被修改。'
const resolutionFailureMessage = '这次选择没有保存，本机与云端内容都保持不变。'

function runNotification(effect: () => void) {
  try {
    effect()
  }
  catch {
    // The conflict command is already durable. Notification failures must not revoke it.
  }
}

export function useSyncConflictOperations(options: SyncConflictOperationsOptions) {
  const conflicts = shallowRef<SyncConflictSummary[]>([])
  const loading = ref(false)
  const busyKey = ref('')
  const errorMessage = ref('')
  const statusMessage = ref('')
  let listEpoch = 0
  let refreshQueued = false

  async function reload(): Promise<boolean> {
    if (busyKey.value) {
      refreshQueued = true
      return false
    }

    const epoch = ++listEpoch
    loading.value = true
    errorMessage.value = ''
    try {
      const result = await options.listConflicts()
      if (epoch !== listEpoch) return false
      if (!result.ok) {
        errorMessage.value = result.error.userMessage
        return false
      }

      conflicts.value = result.data
      statusMessage.value = ''
      return true
    }
    catch {
      if (epoch !== listEpoch) return false
      errorMessage.value = listFailureMessage
      return false
    }
    finally {
      if (epoch === listEpoch) loading.value = false
    }
  }

  async function resolve(
    operationKey: string,
    operation: ConflictResolution,
    successMessage: string,
  ): Promise<boolean> {
    if (loading.value || busyKey.value) return false

    busyKey.value = operationKey
    errorMessage.value = ''
    statusMessage.value = ''
    try {
      const result = await operation()
      if (!result.ok) {
        errorMessage.value = result.error.userMessage
        return false
      }

      conflicts.value = result.data
      statusMessage.value = successMessage
      runNotification(options.scheduleSync)
      runNotification(options.onChanged)
      return true
    }
    catch {
      errorMessage.value = resolutionFailureMessage
      return false
    }
    finally {
      busyKey.value = ''
      if (refreshQueued) {
        refreshQueued = false
        void reload()
      }
    }
  }

  return {
    conflicts: shallowReadonly(conflicts),
    loading: readonly(loading),
    busyKey: readonly(busyKey),
    errorMessage: readonly(errorMessage),
    statusMessage: readonly(statusMessage),
    reload,
    resolve,
  }
}
