import { readonly, ref } from 'vue'
import type { AppResult } from '../../shared/api/app-result'
import type { LibraryAccessStatus } from '../../shared/api/bindings'

export type LibraryAccessPhase =
  | 'checking'
  | 'unlocked'
  | 'locked'
  | 'error'
  | 'unlocking'
  | 'restarting'

export type LibraryAccessErrorReason = 'credentials' | 'storage'

interface LibraryAccessLifecycleOptions {
  desktopRuntime: boolean
  checkAccess: () => Promise<AppResult<LibraryAccessStatus>>
  unlock: () => Promise<AppResult<LibraryAccessStatus>>
  initializeWorkspace: () => Promise<void>
}

export function useLibraryAccessLifecycle(options: LibraryAccessLifecycleOptions) {
  const phase = ref<LibraryAccessPhase>(options.desktopRuntime ? 'checking' : 'unlocked')
  const errorMessage = ref('')
  const errorReason = ref<LibraryAccessErrorReason>('credentials')
  const workspaceInitialized = ref(false)
  let accessTask: Promise<boolean> | undefined
  let initializationTask: Promise<boolean> | undefined
  let unlockTask: Promise<boolean> | undefined

  function ensureWorkspaceInitialized(): Promise<boolean> {
    if (workspaceInitialized.value) return Promise.resolve(true)
    if (initializationTask) return initializationTask

    const task = (async () => {
      try {
        await options.initializeWorkspace()
        workspaceInitialized.value = true
        return true
      }
      catch {
        phase.value = 'error'
        errorReason.value = 'credentials'
        errorMessage.value = '资料库已经解锁，但工作区没有完成初始化，请重新检查。'
        return false
      }
      finally {
        initializationTask = undefined
      }
    })()
    initializationTask = task
    return task
  }

  async function runAccessCheck(): Promise<boolean> {
    if (!options.desktopRuntime) {
      phase.value = 'unlocked'
      return ensureWorkspaceInitialized()
    }

    phase.value = 'checking'
    errorMessage.value = ''
    errorReason.value = 'credentials'
    try {
      const result = await options.checkAccess()
      if (!result.ok) {
        phase.value = 'error'
        errorMessage.value = result.error.userMessage
        errorReason.value = result.error.code === 'LIBRARY_STORAGE_UNAVAILABLE'
          ? 'storage'
          : 'credentials'
        return false
      }
      if (result.data.locked) {
        phase.value = 'locked'
        return false
      }

      phase.value = 'unlocked'
      return ensureWorkspaceInitialized()
    }
    catch {
      phase.value = 'error'
      errorMessage.value = 'Windows 凭据管理器没有响应，请重新检查或使用当前账户解锁。'
      errorReason.value = 'credentials'
      return false
    }
  }

  function checkLibraryAccess(): Promise<boolean> {
    if (accessTask) return accessTask
    if (unlockTask || phase.value === 'restarting') return Promise.resolve(false)
    const task = runAccessCheck().finally(() => {
      if (accessTask === task) accessTask = undefined
    })
    accessTask = task
    return task
  }

  async function runUnlock(): Promise<boolean> {
    phase.value = 'unlocking'
    errorMessage.value = ''
    errorReason.value = 'credentials'
    try {
      const result = await options.unlock()
      if (!result.ok) {
        phase.value = 'error'
        errorMessage.value = result.error.userMessage
        return false
      }
      phase.value = 'restarting'
      return true
    }
    catch {
      phase.value = 'error'
      errorMessage.value = '当前 Windows 账户未能完成解锁，请稍后再试。'
      return false
    }
  }

  function unlockLibrary(): Promise<boolean> {
    if (unlockTask) return unlockTask
    if (!options.desktopRuntime || accessTask || phase.value === 'restarting')
      return Promise.resolve(false)
    const task = runUnlock().finally(() => {
      if (unlockTask === task) unlockTask = undefined
    })
    unlockTask = task
    return task
  }

  function enterRestarting() {
    phase.value = 'restarting'
    errorMessage.value = ''
  }

  return {
    phase: readonly(phase),
    errorMessage: readonly(errorMessage),
    errorReason: readonly(errorReason),
    workspaceInitialized: readonly(workspaceInitialized),
    checkLibraryAccess,
    unlockLibrary,
    enterRestarting,
  }
}
