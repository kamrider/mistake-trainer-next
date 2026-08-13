import { computed, readonly, ref } from 'vue'
import type { AppResult } from '../../shared/api/app-result'
import type {
  WindowsCompatibilityStatus,
  WindowsUpdateCheckReport,
  WindowsUpdateInstallReceipt,
  WindowsUpdateStatus,
} from '../../shared/api/bindings'
import { formatSettingsTime } from '../settings-formatters'

export interface SettingsWindowsUpdateOperations {
  compatibility: () => Promise<AppResult<WindowsCompatibilityStatus>>
  status: () => Promise<AppResult<WindowsUpdateStatus>>
  check: () => Promise<AppResult<WindowsUpdateCheckReport>>
  install: (expectedVersion: string) => Promise<AppResult<WindowsUpdateInstallReceipt>>
}

export interface SettingsWindowsUpdateOptions {
  operations: SettingsWindowsUpdateOperations
  restoreFocus: () => Promise<unknown> | unknown
}

export function useSettingsWindowsUpdate(options: SettingsWindowsUpdateOptions) {
  const compatibility = ref<WindowsCompatibilityStatus>()
  const status = ref<WindowsUpdateStatus>()
  const report = ref<WindowsUpdateCheckReport>()
  const checking = ref(false)
  const installing = ref(false)
  const message = ref('')
  const publicationLabel = computed(() => formatPublication(report.value?.publishedAt ?? null))
  let checkTask: Promise<boolean> | undefined
  let installTask: Promise<boolean> | undefined

  function formatPublication(value: string | null): string {
    if (!value) return ''
    const timestamp = Date.parse(value)
    return Number.isFinite(timestamp) ? formatSettingsTime(timestamp) : ''
  }

  async function loadCompatibility(): Promise<boolean> {
    try {
      const result = await options.operations.compatibility()
      if (!result.ok) return false
      compatibility.value = result.data
      return true
    }
    catch {
      return false
    }
  }

  async function loadStatus(): Promise<boolean> {
    try {
      const result = await options.operations.status()
      if (!result.ok) return false
      status.value = result.data
      return true
    }
    catch {
      return false
    }
  }

  function check(): Promise<boolean> {
    if (checkTask) return checkTask
    if (installing.value || !status.value?.enabled) return Promise.resolve(false)

    const task = (async () => {
      checking.value = true
      report.value = undefined
      message.value = ''
      try {
        const result = await options.operations.check()
        if (!result.ok) {
          message.value = result.error.userMessage
          return false
        }
        report.value = result.data
        message.value = result.data.available && result.data.version
          ? `发现已签名版本 ${result.data.version}。下载后仍会再次核对版本和签名。`
          : '当前已经是最新版本。'
        return true
      }
      catch {
        message.value = '暂时无法检查更新，请确认网络连接后重试；当前版本可继续离线使用。'
        return false
      }
      finally {
        checking.value = false
        await options.restoreFocus()
      }
    })().finally(() => {
      if (checkTask === task) checkTask = undefined
    })
    checkTask = task
    return task
  }

  function install(): Promise<boolean> {
    if (installTask) return installTask
    const version = report.value?.available ? report.value.version : undefined
    if (checking.value || !version) return Promise.resolve(false)

    const task = (async () => {
      installing.value = true
      message.value = '正在下载并验证更新；安装开始时应用会关闭。'
      try {
        const result = await options.operations.install(version)
        if (!result.ok) {
          report.value = undefined
          message.value = result.error.userMessage
          return false
        }
        message.value = '安装程序已启动；请按系统提示完成更新。'
        return true
      }
      catch {
        report.value = undefined
        message.value = '更新没有安装，当前版本保持不变；请稍后重新检查。'
        return false
      }
      finally {
        installing.value = false
        await options.restoreFocus()
      }
    })().finally(() => {
      if (installTask === task) installTask = undefined
    })
    installTask = task
    return task
  }

  return {
    compatibility: readonly(compatibility),
    status: readonly(status),
    report: readonly(report),
    checking: readonly(checking),
    installing: readonly(installing),
    message: readonly(message),
    publicationLabel,
    loadCompatibility,
    loadStatus,
    check,
    install,
    formatPublication,
  }
}
