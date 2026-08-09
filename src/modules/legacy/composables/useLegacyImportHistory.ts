import { computed, readonly, ref } from 'vue'
import type { AppResult } from '../../../shared/api/app-result'
import type { LegacyImportSummary } from '../../../shared/api/bindings'

interface LegacyImportHistoryOptions {
  listImports: () => Promise<AppResult<LegacyImportSummary[]>>
}

export function useLegacyImportHistory(options: LegacyImportHistoryOptions) {
  const imports = ref<LegacyImportSummary[]>([])
  const loading = ref(false)
  const loaded = ref(false)
  const errorMessage = ref('')
  const stale = computed(() => (
    loaded.value && imports.value.length > 0 && Boolean(errorMessage.value)
  ))
  let requestEpoch = 0

  async function reload(): Promise<boolean> {
    const epoch = ++requestEpoch
    loading.value = true
    errorMessage.value = ''
    try {
      const result = await options.listImports()
      if (epoch !== requestEpoch) return false
      if (!result.ok) {
        errorMessage.value = result.error.userMessage
        return false
      }

      imports.value = result.data
      loaded.value = true
      return true
    }
    catch {
      if (epoch !== requestEpoch) return false
      errorMessage.value = '迁移记录暂时无法读取，请稍后重试。'
      return false
    }
    finally {
      if (epoch === requestEpoch) loading.value = false
    }
  }

  return {
    imports: readonly(imports),
    loading: readonly(loading),
    loaded: readonly(loaded),
    errorMessage: readonly(errorMessage),
    stale,
    reload,
  }
}
