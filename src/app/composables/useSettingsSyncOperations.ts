import { readonly, ref } from 'vue'
import type { AppResult } from '../../shared/api/app-result'
import type {
  SettingsOverview,
  SyncNowReport,
} from '../../shared/api/bindings'

interface SettingsSyncOperationsOptions {
  sync: () => Promise<AppResult<SyncNowReport>>
  refreshOverview: () => Promise<AppResult<SettingsOverview>>
  applyOverview: (overview: SettingsOverview) => void
  refreshConflicts: () => Promise<boolean>
}

const primaryFailureMessage = '同步请求没有完成，待同步变更会保留并等待下次重试。'

export function useSettingsSyncOperations(options: SettingsSyncOperationsOptions) {
  const busy = ref(false)
  const message = ref('')

  async function syncNow(): Promise<boolean> {
    if (busy.value) return false
    busy.value = true
    message.value = ''
    try {
      let result: AppResult<SyncNowReport>
      try {
        result = await options.sync()
      }
      catch {
        message.value = primaryFailureMessage
        return false
      }
      if (!result.ok) {
        message.value = result.error.userMessage
        return false
      }

      const staleSurfaces: string[] = []
      try {
        const overviewResult = await options.refreshOverview()
        if (overviewResult.ok) options.applyOverview(overviewResult.data)
        else staleSurfaces.push('顶部资料库统计')
      }
      catch {
        staleSurfaces.push('顶部资料库统计')
      }

      try {
        if (!await options.refreshConflicts()) staleSurfaces.push('同步冲突列表')
      }
      catch {
        staleSurfaces.push('同步冲突列表')
      }

      const counts = `同步完成：上传 ${result.data.pushedOperationCount} 项，拉取 ${result.data.pulledChangeCount} 项`
      message.value = staleSurfaces.length
        ? `${counts}；${staleSurfaces.join('、')}暂时没有刷新。`
        : `${counts}。`
      return true
    }
    finally {
      busy.value = false
    }
  }

  return {
    busy: readonly(busy),
    message: readonly(message),
    syncNow,
  }
}
