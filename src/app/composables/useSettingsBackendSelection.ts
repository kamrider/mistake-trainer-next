import { readonly, ref } from 'vue'
import { failure, type AppResult } from '../../shared/api/app-result'
import type { CloudBackendKind, CloudBackendStatus } from '../../shared/api/bindings'

interface SettingsBackendSelectionOptions {
  load: () => Promise<AppResult<CloudBackendStatus>>
  select: (kind: CloudBackendKind) => Promise<AppResult<CloudBackendStatus>>
  label: (kind: CloudBackendKind) => string
}

const selectionFailureMessage = '同步后端设置暂时不可用，本地数据不会受到影响。'

export function useSettingsBackendSelection(options: SettingsBackendSelectionOptions) {
  const status = ref<AppResult<CloudBackendStatus>>()
  const busy = ref(false)
  const message = ref('')
  let latestLoadRevision = 0
  let selectionRevision = 0

  async function loadStatus(): Promise<boolean> {
    if (busy.value) return false
    const loadRevision = ++latestLoadRevision
    const selectionRevisionAtStart = selectionRevision
    let result: AppResult<CloudBackendStatus>
    try {
      result = await options.load()
    }
    catch {
      result = failure(
        'SYNC_STATUS_UNAVAILABLE',
        '暂时无法读取同步设置，本地数据仍可正常使用',
        true,
        'settings-sync-status-unavailable',
      )
    }
    if (
      loadRevision !== latestLoadRevision
      || selectionRevisionAtStart !== selectionRevision
      || busy.value
    ) return false
    status.value = result
    return true
  }

  async function choose(kind: CloudBackendKind): Promise<boolean> {
    const current = status.value
    if (
      busy.value
      || kind === 'tencent'
      || (current?.ok && current.data.kind === kind)
    ) return false

    selectionRevision += 1
    latestLoadRevision += 1
    busy.value = true
    message.value = ''
    try {
      const result = await options.select(kind)
      if (!result.ok) {
        message.value = result.error.userMessage
        return false
      }
      status.value = result
      message.value = `已选择 ${options.label(result.data.kind)}`
      return true
    }
    catch {
      message.value = selectionFailureMessage
      return false
    }
    finally {
      busy.value = false
    }
  }

  return {
    status: readonly(status),
    busy: readonly(busy),
    message: readonly(message),
    loadStatus,
    choose,
  }
}
