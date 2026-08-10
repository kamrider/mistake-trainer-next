import { computed, readonly, ref } from 'vue'
import type { AppResult } from '../../shared/api/app-result'
import type {
  WindowsUpdateCheckReport,
  WindowsUpdateInstallReceipt,
  WindowsUpdateStatus,
} from '../../shared/api/bindings'

export const STARTUP_UPDATE_CHECK_INTERVAL_MS = 24 * 60 * 60 * 1_000
export const STARTUP_UPDATE_DELAY_MS = 1_500
export const STARTUP_UPDATE_STORAGE_KEY = 'mistake-trainer.update.lastAutomaticCheckUtcMs.v1'

type UpdateOperations = {
  status: () => Promise<AppResult<WindowsUpdateStatus>>
  check: () => Promise<AppResult<WindowsUpdateCheckReport>>
  install: (expectedVersion: string) => Promise<AppResult<WindowsUpdateInstallReceipt>>
}

type StartupUpdateOptions = {
  desktopRuntime: boolean
  operations: UpdateOperations
  storage?: Pick<Storage, 'getItem' | 'setItem'>
  online?: () => boolean
  now?: () => number
  delayMs?: number
  schedule?: (callback: () => void, delayMs: number) => ReturnType<typeof setTimeout>
  cancelScheduled?: (handle: ReturnType<typeof setTimeout>) => void
}

export function useStartupUpdate(options: StartupUpdateOptions) {
  const report = ref<WindowsUpdateCheckReport>()
  const installing = ref(false)
  const message = ref('')
  const available = computed(() => Boolean(report.value?.available && report.value.version))
  const now = options.now ?? Date.now
  const storage = options.storage ?? defaultStorage()
  const online = options.online ?? (() => typeof navigator === 'undefined' || navigator.onLine !== false)
  const schedule = options.schedule ?? ((callback, delayMs) => setTimeout(callback, delayMs))
  const cancelScheduled = options.cancelScheduled ?? clearTimeout
  let scheduledCheck: ReturnType<typeof setTimeout> | undefined
  let checkingTask: Promise<void> | undefined
  let installTask: Promise<void> | undefined
  let started = false
  let disposed = false

  function start() {
    if (started || disposed || !options.desktopRuntime) return
    started = true
    scheduledCheck = schedule(() => {
      scheduledCheck = undefined
      void checkForUpdate(false)
    }, options.delayMs ?? STARTUP_UPDATE_DELAY_MS)
  }

  function dispose() {
    disposed = true
    if (scheduledCheck !== undefined) {
      cancelScheduled(scheduledCheck)
      scheduledCheck = undefined
    }
  }

  function checkedRecently(atUtcMs: number): boolean {
    if (!storage) return false
    try {
      const raw = storage.getItem(STARTUP_UPDATE_STORAGE_KEY)
      if (raw === null || raw.trim() === '') return false
      const lastCheckAtUtcMs = Number(raw)
      const elapsedMs = atUtcMs - lastCheckAtUtcMs
      return Number.isFinite(lastCheckAtUtcMs)
        && lastCheckAtUtcMs >= 0
        && elapsedMs >= 0
        && elapsedMs < STARTUP_UPDATE_CHECK_INTERVAL_MS
    }
    catch {
      return false
    }
  }

  function recordCheckAttempt(atUtcMs: number): boolean {
    if (!storage) return false
    try {
      storage.setItem(STARTUP_UPDATE_STORAGE_KEY, String(atUtcMs))
      return true
    }
    catch {
      return false
    }
  }

  function checkForUpdate(ignoreThrottle: boolean): Promise<void> {
    if (checkingTask || disposed || !options.desktopRuntime || !online())
      return checkingTask ?? Promise.resolve()

    const task = (async () => {
      try {
        const status = await options.operations.status()
        if (disposed || !status.ok || !status.data.enabled) return

        const attemptAtUtcMs = now()
        if (!ignoreThrottle && checkedRecently(attemptAtUtcMs)) return
        if (!recordCheckAttempt(attemptAtUtcMs)) return

        const result = await options.operations.check()
        if (disposed || !result.ok) return
        report.value = result.data.available && result.data.version
          ? result.data
          : undefined
        message.value = ''
      }
      catch {
        // Automatic update failures stay silent; Settings retains a manual retry path.
      }
    })().finally(() => {
      if (checkingTask === task) checkingTask = undefined
    })
    checkingTask = task
    return task
  }

  function dismiss() {
    if (installing.value) return
    report.value = undefined
    message.value = ''
  }

  function install(): Promise<void> {
    if (installTask) return installTask
    const expectedVersion = report.value?.available ? report.value.version : undefined
    if (!expectedVersion || disposed) return Promise.resolve()

    const task = (async () => {
      installing.value = true
      message.value = '正在下载并验证更新；安装开始时应用会关闭。'
      try {
        const result = await options.operations.install(expectedVersion)
        if (disposed) return
        if (!result.ok) {
          if (result.error.code === 'update_version_changed') {
            report.value = undefined
            message.value = ''
            await checkForUpdate(true)
            return
          }
          message.value = result.error.userMessage
          return
        }
        message.value = '安装程序已启动；请按系统提示完成更新。'
      }
      catch {
        if (!disposed)
          message.value = '更新没有安装，当前版本保持不变；请稍后重试。'
      }
      finally {
        installing.value = false
      }
    })().finally(() => {
      if (installTask === task) installTask = undefined
    })
    installTask = task
    return task
  }

  return {
    report: readonly(report),
    available: readonly(available),
    installing: readonly(installing),
    message: readonly(message),
    start,
    dispose,
    dismiss,
    install,
  }
}

function defaultStorage(): Pick<Storage, 'getItem' | 'setItem'> | undefined {
  if (typeof window === 'undefined') return undefined
  try {
    return window.localStorage
  }
  catch {
    return undefined
  }
}
