import { readonly, ref } from 'vue'
import type { AppResult } from '../../shared/api/app-result'
import type { DiagnosticExportReceipt } from '../../shared/api/bindings'

export interface SettingsDiagnosticsExportOptions {
  available: boolean
  exportReport: () => Promise<AppResult<DiagnosticExportReceipt | null>>
  restoreFocus: () => Promise<unknown> | unknown
}

export function useSettingsDiagnosticsExport(options: SettingsDiagnosticsExportOptions) {
  const receipt = ref<DiagnosticExportReceipt>()
  const busy = ref(false)
  const message = ref('')
  let activeTask: Promise<boolean> | undefined

  function exportDiagnostics(): Promise<boolean> {
    if (activeTask) return activeTask
    if (!options.available) return Promise.resolve(false)

    const task = (async () => {
      busy.value = true
      message.value = ''
      receipt.value = undefined
      try {
        const result = await options.exportReport()
        if (!result.ok) {
          message.value = result.error.userMessage
          return false
        }
        if (!result.data) return false
        receipt.value = result.data
        return true
      }
      catch {
        message.value = '诊断报告没有生成，现有资料不会受到影响；请检查磁盘空间和保存位置后重试。'
        return false
      }
      finally {
        busy.value = false
        await options.restoreFocus()
      }
    })().finally(() => {
      if (activeTask === task) activeTask = undefined
    })
    activeTask = task
    return task
  }

  return {
    receipt: readonly(receipt),
    busy: readonly(busy),
    message: readonly(message),
    exportDiagnostics,
  }
}
