import { readonly, ref } from 'vue'
import type { AppResult } from '../../shared/api/app-result'
import type {
  AuthCredentials,
  CloudAuthState,
  LibraryAccessStatus,
} from '../../shared/api/bindings'

export interface SettingsCloudSessionOperations {
  restore?: () => Promise<AppResult<CloudAuthState>>
  status?: () => Promise<AppResult<CloudAuthState>>
  signIn: (credentials: AuthCredentials) => Promise<AppResult<CloudAuthState>>
  signUp: (credentials: AuthCredentials) => Promise<AppResult<CloudAuthState>>
  disconnect: () => Promise<AppResult<CloudAuthState>>
  lockLibrary: () => Promise<AppResult<LibraryAccessStatus>>
  loadDeviceAccess: () => Promise<AppResult<LibraryAccessStatus>>
}

export interface SettingsCloudSessionOptions {
  operations: SettingsCloudSessionOperations
  onConnected: () => unknown
  onRestarting: () => void
  restoreLockFocus: (mode: 'lock' | 'sign-out') => Promise<unknown> | unknown
}

export function useSettingsCloudSession(options: SettingsCloudSessionOptions) {
  const auth = ref<CloudAuthState>()
  const email = ref('')
  const password = ref('')
  const mode = ref<'signIn' | 'signUp'>('signIn')
  const authBusy = ref(false)
  const authMessage = ref('')
  const lockDialogOpen = ref(false)
  const lockDialogMode = ref<'lock' | 'sign-out'>('lock')
  const lockingLibrary = ref(false)
  const lockErrorMessage = ref('')
  const deviceAccessStatus = ref<LibraryAccessStatus>()
  const deviceAccessError = ref('')

  let authRevision = 0
  let loadRevision = 0

  function loadIsCurrent(authToken: number, loadToken: number) {
    return authToken === authRevision && loadToken === loadRevision && !authBusy.value
  }

  async function restoreSession(): Promise<boolean> {
    if (authBusy.value || lockingLibrary.value) return false
    const authToken = authRevision
    const loadToken = ++loadRevision
    if (options.operations.restore) {
      try {
        const result = await options.operations.restore()
        if (!loadIsCurrent(authToken, loadToken)) return false
        if (result.ok) {
          auth.value = result.data
          return true
        }
      }
      catch {
        // Credential restoration is optional; fall back to the explicit status command.
      }
    }
    if (!loadIsCurrent(authToken, loadToken) || !options.operations.status) return false
    try {
      const result = await options.operations.status()
      if (!loadIsCurrent(authToken, loadToken) || !result.ok) return false
      auth.value = result.data
      return true
    }
    catch {
      return false
    }
  }

  function scheduleConnectedSync() {
    try {
      void Promise.resolve(options.onConnected()).catch(() => undefined)
    }
    catch {
      // Authentication is already durable; first-sync scheduling remains best-effort.
    }
  }

  async function submit(): Promise<boolean> {
    const normalizedEmail = email.value.trim()
    if (authBusy.value || lockingLibrary.value || !normalizedEmail || !password.value) return false
    const credentials: AuthCredentials = {
      email: normalizedEmail,
      password: password.value,
    }
    authRevision += 1
    const token = authRevision
    authBusy.value = true
    authMessage.value = ''
    try {
      const result = mode.value === 'signUp'
        ? await options.operations.signUp(credentials)
        : await options.operations.signIn(credentials)
      if (token !== authRevision) return false
      if (!result.ok) {
        authMessage.value = result.error.userMessage
        return false
      }
      auth.value = result.data
      password.value = ''
      authMessage.value = result.data.status.kind === 'verification_required'
        ? '注册成功，请先完成邮箱验证，再回来登录。'
        : result.data.status.kind === 'connected'
          ? '已连接，正在开始第一次安全同步。'
          : '登录状态已更新。'
      if (result.data.status.kind === 'connected') scheduleConnectedSync()
      return true
    }
    catch {
      if (token === authRevision) authMessage.value = '登录请求没有完成，本地数据不受影响。'
      return false
    }
    finally {
      if (token === authRevision) authBusy.value = false
    }
  }

  async function disconnectCloud(): Promise<boolean> {
    if (authBusy.value) return false
    authRevision += 1
    const token = authRevision
    authBusy.value = true
    authMessage.value = ''
    try {
      const result = await options.operations.disconnect()
      if (token !== authRevision) return false
      if (!result.ok) {
        authMessage.value = result.error.userMessage
        return false
      }
      auth.value = result.data
      return true
    }
    catch {
      if (token === authRevision) authMessage.value = '退出云端账户没有完成。'
      return false
    }
    finally {
      if (token === authRevision) authBusy.value = false
    }
  }

  function openLibraryLock(nextMode: 'lock' | 'sign-out') {
    if (lockingLibrary.value) return
    lockDialogMode.value = nextMode
    lockErrorMessage.value = ''
    lockDialogOpen.value = true
  }

  async function closeLibraryLock(): Promise<boolean> {
    if (lockingLibrary.value || !lockDialogOpen.value) return false
    const closedMode = lockDialogMode.value
    lockDialogOpen.value = false
    await options.restoreLockFocus(closedMode)
    return true
  }

  async function loadDeviceAccessStatus(): Promise<boolean> {
    deviceAccessError.value = ''
    try {
      const result = await options.operations.loadDeviceAccess()
      if (!result.ok) {
        deviceAccessStatus.value = undefined
        deviceAccessError.value = result.error.userMessage
        return false
      }
      deviceAccessStatus.value = result.data
      return true
    }
    catch {
      deviceAccessStatus.value = undefined
      deviceAccessError.value = '当前设备的离线解锁状态暂时无法读取；资料库仍保持本机加密。'
      return false
    }
  }

  async function confirmLibraryLock(): Promise<boolean> {
    if (lockingLibrary.value || !lockDialogOpen.value) return false
    lockingLibrary.value = true
    lockErrorMessage.value = ''
    try {
      if (lockDialogMode.value === 'sign-out' && !await disconnectCloud()) {
        lockErrorMessage.value = authMessage.value || '云端账户没有退出，本地资料库保持打开。'
        lockingLibrary.value = false
        return false
      }
      const result = await options.operations.lockLibrary()
      if (!result.ok) {
        lockErrorMessage.value = result.error.userMessage
        lockingLibrary.value = false
        return false
      }
      try {
        options.onRestarting()
      }
      catch {
        // Native locking already succeeded; remain inside the restart boundary.
      }
      return true
    }
    catch {
      lockErrorMessage.value = '本地资料库没有完成锁定，当前资料保持打开，请稍后再试。'
      lockingLibrary.value = false
      return false
    }
  }

  return {
    auth,
    email,
    password,
    mode,
    authBusy,
    authMessage,
    lockDialogOpen,
    lockDialogMode,
    lockingLibrary,
    lockErrorMessage,
    deviceAccessStatus: readonly(deviceAccessStatus),
    deviceAccessError: readonly(deviceAccessError),
    restoreSession,
    submit,
    disconnectCloud,
    openLibraryLock,
    closeLibraryLock,
    confirmLibraryLock,
    loadDeviceAccessStatus,
  }
}
