import { describe, expect, it, vi } from 'vitest'
import type {
  CloudAuthState,
  LibraryAccessStatus,
} from '../../shared/api/bindings'
import { failure, success, type AppResult } from '../../shared/api/app-result'
import { useSettingsCloudSession } from './useSettingsCloudSession'

const signedOut: CloudAuthState = {
  configured: true,
  status: { kind: 'signed_out', emailHint: null },
}
const connected: CloudAuthState = {
  configured: true,
  status: { kind: 'connected', emailHint: 'u***@example.com' },
}
const verificationRequired: CloudAuthState = {
  configured: true,
  status: { kind: 'verification_required', emailHint: 'u***@example.com' },
}
const locked: LibraryAccessStatus = {
  state: 'locked',
  trustedWindowsAccount: true,
  recoveryReason: null,
}

function deferred<T>() {
  let resolve!: (value: T) => void
  let reject!: (reason?: unknown) => void
  const promise = new Promise<T>((finish, fail) => {
    resolve = finish
    reject = fail
  })
  return { promise, resolve, reject }
}

function harness() {
  const operations = {
    restore: vi.fn().mockResolvedValue(success(signedOut)),
    status: vi.fn().mockResolvedValue(success(signedOut)),
    signIn: vi.fn().mockResolvedValue(success(connected)),
    signUp: vi.fn().mockResolvedValue(success(verificationRequired)),
    disconnect: vi.fn().mockResolvedValue(success(signedOut)),
    lockLibrary: vi.fn().mockResolvedValue(success(locked)),
    loadDeviceAccess: vi.fn().mockResolvedValue(success(locked)),
  }
  const onConnected = vi.fn()
  const onRestarting = vi.fn()
  const restoreLockFocus = vi.fn(async () => undefined)
  const controller = useSettingsCloudSession({
    operations,
    onConnected,
    onRestarting,
    restoreLockFocus,
  })
  return { controller, operations, onConnected, onRestarting, restoreLockFocus }
}

describe('useSettingsCloudSession', () => {
  it('does not let a stale restore response overwrite a newer successful login', async () => {
    const restoreGate = deferred<AppResult<CloudAuthState>>()
    const h = harness()
    h.operations.restore.mockReturnValueOnce(restoreGate.promise)

    const restoring = h.controller.restoreSession()
    h.controller.email.value = ' user@example.com '
    h.controller.password.value = 'secret-123'
    const signingIn = h.controller.submit()

    await expect(signingIn).resolves.toBe(true)
    expect(h.controller.auth.value).toEqual(connected)
    expect(h.controller.password.value).toBe('')
    restoreGate.resolve(success(signedOut))
    await restoring

    expect(h.controller.auth.value).toEqual(connected)
    expect(h.onConnected).toHaveBeenCalledOnce()
  })

  it('admits only one authentication mutation while a request is pending', async () => {
    const signInGate = deferred<AppResult<CloudAuthState>>()
    const h = harness()
    h.operations.signIn.mockReturnValueOnce(signInGate.promise)
    h.controller.email.value = 'user@example.com'
    h.controller.password.value = 'secret-123'

    const first = h.controller.submit()
    await vi.waitFor(() => expect(h.operations.signIn).toHaveBeenCalledOnce())
    await expect(h.controller.submit()).resolves.toBe(false)
    await expect(h.controller.disconnectCloud()).resolves.toBe(false)

    expect(h.controller.authBusy.value).toBe(true)
    signInGate.resolve(success(connected))
    await expect(first).resolves.toBe(true)
    expect(h.controller.authBusy.value).toBe(false)
    expect(h.operations.signIn).toHaveBeenCalledOnce()
  })

  it('uses exact trimmed sign-up credentials and verification guidance', async () => {
    const h = harness()
    h.controller.mode.value = 'signUp'
    h.controller.email.value = ' new@example.com '
    h.controller.password.value = 'register-123'

    await expect(h.controller.submit()).resolves.toBe(true)

    expect(h.operations.signUp).toHaveBeenCalledWith({
      email: 'new@example.com',
      password: 'register-123',
    })
    expect(h.controller.password.value).toBe('')
    expect(h.controller.authMessage.value).toBe('注册成功，请先完成邮箱验证，再回来登录。')
    expect(h.onConnected).not.toHaveBeenCalled()
  })

  it('disconnects before locking and stays in the restart boundary after success', async () => {
    const h = harness()
    h.controller.auth.value = connected
    h.controller.openLibraryLock('sign-out')

    await expect(h.controller.confirmLibraryLock()).resolves.toBe(true)

    expect(h.operations.disconnect).toHaveBeenCalledOnce()
    expect(h.operations.lockLibrary).toHaveBeenCalledOnce()
    expect(h.operations.disconnect.mock.invocationCallOrder[0]!)
      .toBeLessThan(h.operations.lockLibrary.mock.invocationCallOrder[0]!)
    expect(h.onRestarting).toHaveBeenCalledOnce()
    expect(h.controller.lockingLibrary.value).toBe(true)
    expect(h.controller.lockDialogOpen.value).toBe(true)
  })

  it('keeps the lock decision retryable when cloud disconnect fails', async () => {
    const h = harness()
    h.controller.auth.value = connected
    h.operations.disconnect.mockResolvedValue(
      failure('auth_disconnect_failed', '云端会话没有退出。', true, 'diag-disconnect'),
    )
    h.controller.openLibraryLock('sign-out')

    await expect(h.controller.confirmLibraryLock()).resolves.toBe(false)

    expect(h.operations.lockLibrary).not.toHaveBeenCalled()
    expect(h.controller.lockErrorMessage.value).toBe('云端会话没有退出。')
    expect(h.controller.lockingLibrary.value).toBe(false)
    expect(h.controller.lockDialogOpen.value).toBe(true)
  })

  it('falls back to status when optional credential restoration is unavailable', async () => {
    const h = harness()
    h.operations.restore.mockRejectedValue(new Error('credential unavailable'))
    h.operations.status.mockResolvedValue(success(connected))

    await expect(h.controller.restoreSession()).resolves.toBe(true)

    expect(h.operations.status).toHaveBeenCalledOnce()
    expect(h.controller.auth.value).toEqual(connected)
    expect(h.controller.authMessage.value).toBe('')
  })

  it('loads device access status and clears stale state on safe failures', async () => {
    const h = harness()
    await expect(h.controller.loadDeviceAccessStatus()).resolves.toBe(true)
    expect(h.controller.deviceAccessStatus.value).toEqual(locked)

    h.operations.loadDeviceAccess.mockResolvedValueOnce(failure(
      'device_status_failed',
      '无法读取 Windows 资料库凭据，已保持锁定。',
      true,
      'private-device',
    ))
    await expect(h.controller.loadDeviceAccessStatus()).resolves.toBe(false)
    expect(h.controller.deviceAccessStatus.value).toBeUndefined()
    expect(h.controller.deviceAccessError.value).toBe('无法读取 Windows 资料库凭据，已保持锁定。')

    h.operations.loadDeviceAccess.mockRejectedValueOnce(new Error('credential service'))
    await expect(h.controller.loadDeviceAccessStatus()).resolves.toBe(false)
    expect(h.controller.deviceAccessError.value).toBe(
      '当前设备的离线解锁状态暂时无法读取；资料库仍保持本机加密。',
    )
  })

  it.each(['lock', 'sign-out'] as const)('restores %s trigger focus when closing', async (mode) => {
    const h = harness()
    h.controller.openLibraryLock(mode)
    await expect(h.controller.closeLibraryLock()).resolves.toBe(true)
    expect(h.restoreLockFocus).toHaveBeenCalledWith(mode)
    expect(h.controller.lockDialogOpen.value).toBe(false)
  })
})
