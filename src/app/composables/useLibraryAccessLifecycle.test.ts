import { describe, expect, it, vi } from 'vitest'
import { failure, success } from '../../shared/api/app-result'
import type { LibraryAccessStatus } from '../../shared/api/bindings'
import { useLibraryAccessLifecycle } from './useLibraryAccessLifecycle'

function deferred<T>() {
  let resolve!: (value: T) => void
  const promise = new Promise<T>((finish) => { resolve = finish })
  return { promise, resolve }
}

const unlocked: LibraryAccessStatus = {
  state: 'unlocked',
  trustedWindowsAccount: true,
  recoveryReason: null,
}
const locked: LibraryAccessStatus = {
  state: 'locked',
  trustedWindowsAccount: true,
  recoveryReason: null,
}

describe('useLibraryAccessLifecycle', () => {
  it('coalesces concurrent access checks and initializes the workspace once', async () => {
    const accessRequest = deferred<ReturnType<typeof success<LibraryAccessStatus>>>()
    const checkAccess = vi.fn().mockReturnValue(accessRequest.promise)
    const initializeWorkspace = vi.fn().mockResolvedValue(undefined)
    const lifecycle = useLibraryAccessLifecycle({
      desktopRuntime: true,
      checkAccess,
      retry: vi.fn(),
      unlock: vi.fn(),
      initializeWorkspace,
    })

    const first = lifecycle.checkLibraryAccess()
    const second = lifecycle.checkLibraryAccess()

    expect(second).toBe(first)
    expect(checkAccess).toHaveBeenCalledOnce()
    accessRequest.resolve(success(unlocked))
    await Promise.all([first, second])

    expect(initializeWorkspace).toHaveBeenCalledOnce()
    expect(lifecycle.workspaceInitialized.value).toBe(true)
    expect(lifecycle.phase.value).toBe('unlocked')
  })

  it('does not initialize a locked workspace', async () => {
    const initializeWorkspace = vi.fn().mockResolvedValue(undefined)
    const lifecycle = useLibraryAccessLifecycle({
      desktopRuntime: true,
      checkAccess: vi.fn().mockResolvedValue(success(locked)),
      retry: vi.fn(),
      unlock: vi.fn(),
      initializeWorkspace,
    })

    await lifecycle.checkLibraryAccess()

    expect(lifecycle.phase.value).toBe('locked')
    expect(initializeWorkspace).not.toHaveBeenCalled()
    expect(lifecycle.workspaceInitialized.value).toBe(false)
  })

  it('keeps recovery fail-closed and restarts for a fresh startup probe', async () => {
    const checkAccess = vi.fn().mockResolvedValue(success({
      state: 'recovery_required',
      trustedWindowsAccount: true,
      recoveryReason: 'storage_disconnected',
    }))
    const retry = vi.fn().mockResolvedValue(success(true))
    const initializeWorkspace = vi.fn().mockResolvedValue(undefined)
    const lifecycle = useLibraryAccessLifecycle({
      desktopRuntime: true,
      checkAccess,
      retry,
      unlock: vi.fn(),
      initializeWorkspace,
    })

    await lifecycle.checkLibraryAccess()
    expect(lifecycle.phase.value).toBe('recovery')
    expect(lifecycle.recoveryReason.value).toBe('storage_disconnected')
    expect(initializeWorkspace).not.toHaveBeenCalled()

    await lifecycle.retryLibraryAccess()
    expect(checkAccess).toHaveBeenCalledOnce()
    expect(retry).toHaveBeenCalledOnce()
    expect(initializeWorkspace).not.toHaveBeenCalled()
    expect(lifecycle.phase.value).toBe('restarting')
  })

  it('keeps unlock single-flight and stays restarting after native success', async () => {
    const unlockRequest = deferred<ReturnType<typeof success<LibraryAccessStatus>>>()
    const unlock = vi.fn().mockReturnValue(unlockRequest.promise)
    const initializeWorkspace = vi.fn().mockResolvedValue(undefined)
    const lifecycle = useLibraryAccessLifecycle({
      desktopRuntime: true,
      checkAccess: vi.fn().mockResolvedValue(success(locked)),
      retry: vi.fn(),
      unlock,
      initializeWorkspace,
    })
    await lifecycle.checkLibraryAccess()

    const first = lifecycle.unlockLibrary()
    const second = lifecycle.unlockLibrary()

    expect(second).toBe(first)
    expect(unlock).toHaveBeenCalledOnce()
    expect(lifecycle.phase.value).toBe('unlocking')
    unlockRequest.resolve(success(unlocked))
    await Promise.all([first, second])

    expect(lifecycle.phase.value).toBe('restarting')
    expect(initializeWorkspace).not.toHaveBeenCalled()
  })

  it('does not start unlock while a restart retry is in flight', async () => {
    const retryRequest = deferred<ReturnType<typeof success<boolean>>>()
    const checkAccess = vi.fn().mockResolvedValue(failure(
        'LIBRARY_ACCESS_UNAVAILABLE',
        '暂时无法确认资料库状态。',
        true,
        'diag-access',
      ))
    const retryAccess = vi.fn().mockReturnValue(retryRequest.promise)
    const unlock = vi.fn().mockResolvedValue(success(unlocked))
    const lifecycle = useLibraryAccessLifecycle({
      desktopRuntime: true,
      checkAccess,
      retry: retryAccess,
      unlock,
      initializeWorkspace: vi.fn().mockResolvedValue(undefined),
    })
    await lifecycle.checkLibraryAccess()

    const retry = lifecycle.retryLibraryAccess()
    await expect(lifecycle.unlockLibrary()).resolves.toBe(false)

    expect(unlock).not.toHaveBeenCalled()
    retryRequest.resolve(success(true))
    await expect(retry).resolves.toBe(true)
    expect(lifecycle.phase.value).toBe('restarting')
  })

  it('initializes browser preview once without invoking native access commands', async () => {
    const checkAccess = vi.fn()
    const unlock = vi.fn()
    const initializeWorkspace = vi.fn().mockResolvedValue(undefined)
    const lifecycle = useLibraryAccessLifecycle({
      desktopRuntime: false,
      checkAccess,
      retry: vi.fn(),
      unlock,
      initializeWorkspace,
    })

    await Promise.all([
      lifecycle.checkLibraryAccess(),
      lifecycle.checkLibraryAccess(),
    ])

    expect(checkAccess).not.toHaveBeenCalled()
    expect(unlock).not.toHaveBeenCalled()
    expect(initializeWorkspace).toHaveBeenCalledOnce()
    expect(lifecycle.workspaceInitialized.value).toBe(true)
    expect(lifecycle.phase.value).toBe('unlocked')
  })
})
