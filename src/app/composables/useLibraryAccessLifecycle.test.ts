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
  locked: false,
  trustedWindowsAccount: true,
}
const locked: LibraryAccessStatus = {
  locked: true,
  trustedWindowsAccount: true,
}

describe('useLibraryAccessLifecycle', () => {
  it('coalesces concurrent access checks and initializes the workspace once', async () => {
    const accessRequest = deferred<ReturnType<typeof success<LibraryAccessStatus>>>()
    const checkAccess = vi.fn().mockReturnValue(accessRequest.promise)
    const initializeWorkspace = vi.fn().mockResolvedValue(undefined)
    const lifecycle = useLibraryAccessLifecycle({
      desktopRuntime: true,
      checkAccess,
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
      unlock: vi.fn(),
      initializeWorkspace,
    })

    await lifecycle.checkLibraryAccess()

    expect(lifecycle.phase.value).toBe('locked')
    expect(initializeWorkspace).not.toHaveBeenCalled()
    expect(lifecycle.workspaceInitialized.value).toBe(false)
  })

  it('classifies a storage failure and permits a later successful retry', async () => {
    const checkAccess = vi.fn()
      .mockResolvedValueOnce(failure(
        'LIBRARY_STORAGE_UNAVAILABLE',
        '资料库磁盘尚未连接。',
        true,
        'diag-storage',
      ))
      .mockResolvedValueOnce(success(unlocked))
    const initializeWorkspace = vi.fn().mockResolvedValue(undefined)
    const lifecycle = useLibraryAccessLifecycle({
      desktopRuntime: true,
      checkAccess,
      unlock: vi.fn(),
      initializeWorkspace,
    })

    await lifecycle.checkLibraryAccess()
    expect(lifecycle.phase.value).toBe('error')
    expect(lifecycle.errorReason.value).toBe('storage')
    expect(lifecycle.errorMessage.value).toBe('资料库磁盘尚未连接。')
    expect(initializeWorkspace).not.toHaveBeenCalled()

    await lifecycle.checkLibraryAccess()
    expect(checkAccess).toHaveBeenCalledTimes(2)
    expect(initializeWorkspace).toHaveBeenCalledOnce()
    expect(lifecycle.phase.value).toBe('unlocked')
  })

  it('keeps unlock single-flight and stays restarting after native success', async () => {
    const unlockRequest = deferred<ReturnType<typeof success<LibraryAccessStatus>>>()
    const unlock = vi.fn().mockReturnValue(unlockRequest.promise)
    const initializeWorkspace = vi.fn().mockResolvedValue(undefined)
    const lifecycle = useLibraryAccessLifecycle({
      desktopRuntime: true,
      checkAccess: vi.fn().mockResolvedValue(success(locked)),
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

  it('does not start unlock while an access retry is in flight', async () => {
    const retryRequest = deferred<ReturnType<typeof success<LibraryAccessStatus>>>()
    const checkAccess = vi.fn()
      .mockResolvedValueOnce(failure(
        'LIBRARY_ACCESS_UNAVAILABLE',
        '暂时无法确认资料库状态。',
        true,
        'diag-access',
      ))
      .mockReturnValueOnce(retryRequest.promise)
    const unlock = vi.fn().mockResolvedValue(success(unlocked))
    const lifecycle = useLibraryAccessLifecycle({
      desktopRuntime: true,
      checkAccess,
      unlock,
      initializeWorkspace: vi.fn().mockResolvedValue(undefined),
    })
    await lifecycle.checkLibraryAccess()

    const retry = lifecycle.checkLibraryAccess()
    await expect(lifecycle.unlockLibrary()).resolves.toBe(false)

    expect(unlock).not.toHaveBeenCalled()
    retryRequest.resolve(success(unlocked))
    await expect(retry).resolves.toBe(true)
  })

  it('initializes browser preview once without invoking native access commands', async () => {
    const checkAccess = vi.fn()
    const unlock = vi.fn()
    const initializeWorkspace = vi.fn().mockResolvedValue(undefined)
    const lifecycle = useLibraryAccessLifecycle({
      desktopRuntime: false,
      checkAccess,
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
