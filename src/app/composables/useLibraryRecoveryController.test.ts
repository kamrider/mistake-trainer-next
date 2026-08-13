import { describe, expect, it, vi } from 'vitest'
import { failure, success, type AppResult } from '../../shared/api/app-result'
import type { BackupRestoreCandidate } from '../../shared/api/bindings'
import {
  useLibraryRecoveryController,
  type LibraryRecoveryControllerOptions,
} from './useLibraryRecoveryController'

const candidate: BackupRestoreCandidate = {
  id: 'candidate-1',
  summary: {
    formatVersion: 1,
    createdAtUtcMs: 1,
    assetCount: 2,
    encryptedBytes: 3,
    label: '恢复备份',
    readyForRestore: true,
  },
  expiresAtUtcMs: 100,
}

function deferred<T>() {
  let resolve!: (value: T) => void
  let reject!: (reason?: unknown) => void
  const promise = new Promise<T>((resolvePromise, rejectPromise) => {
    resolve = resolvePromise
    reject = rejectPromise
  })
  return { promise, resolve, reject }
}

function createOptions(
  overrides: Partial<LibraryRecoveryControllerOptions> = {},
): LibraryRecoveryControllerOptions {
  return {
    reconnect: vi.fn().mockResolvedValue(success(true)),
    prepareRestore: vi.fn().mockResolvedValue(success(candidate)),
    restore: vi.fn().mockResolvedValue(success(true)),
    startFresh: vi.fn().mockResolvedValue(success(true)),
    enterRestarting: vi.fn(),
    ...overrides,
  }
}

describe('useLibraryRecoveryController', () => {
  it('keeps fresh-start open on backend rejection and clears stale messages on reopen', async () => {
    const backendMessage = '资料库状态已经变化；没有删除任何凭据。'
    const options = createOptions({
      startFresh: vi.fn().mockResolvedValue(failure(
        'LIBRARY_CHANGED',
        backendMessage,
        false,
        'changed',
      )),
    })
    const controller = useLibraryRecoveryController(options)

    controller.openFreshStartDialog()
    expect(controller.freshStartDialogOpen.value).toBe(true)
    await expect(controller.confirmFreshStart('永久放弃原资料库')).resolves.toBe(false)

    expect(controller.message.value).toBe(backendMessage)
    expect(controller.freshStartDialogOpen.value).toBe(true)
    expect(controller.busy.value).toBe(false)
    expect(options.enterRestarting).not.toHaveBeenCalled()

    controller.closeFreshStartDialog()
    controller.openFreshStartDialog()
    expect(controller.message.value).toBe('')
  })

  it('prepares and confirms one recovery candidate before restarting', async () => {
    const options = createOptions()
    const controller = useLibraryRecoveryController(options)

    await expect(controller.prepareRecoveryBackup()).resolves.toBe(true)
    expect(controller.candidate.value).toEqual(candidate)
    expect(controller.restoreDialogOpen.value).toBe(true)

    await expect(controller.confirmRecoveryBackup()).resolves.toBe(true)
    expect(options.restore).toHaveBeenCalledWith('candidate-1')
    expect(controller.restoreDialogOpen.value).toBe(false)
    expect(options.enterRestarting).toHaveBeenCalledOnce()
  })

  it('restarts only after successful reconnect and fresh-start operations', async () => {
    const options = createOptions()
    const controller = useLibraryRecoveryController(options)

    await expect(controller.reconnectLibrary()).resolves.toBe(true)
    expect(options.enterRestarting).toHaveBeenCalledOnce()

    controller.openFreshStartDialog()
    await expect(controller.confirmFreshStart('永久放弃原资料库')).resolves.toBe(true)
    expect(options.startFresh).toHaveBeenCalledWith('永久放弃原资料库')
    expect(controller.freshStartDialogOpen.value).toBe(false)
    expect(options.enterRestarting).toHaveBeenCalledTimes(2)
  })

  it('coalesces competing recovery actions until the active operation settles', async () => {
    const pending = deferred<AppResult<boolean>>()
    const reconnect = vi.fn().mockReturnValue(pending.promise)
    const prepareRestore = vi.fn().mockResolvedValue(success(candidate))
    const options = createOptions({ reconnect, prepareRestore })
    const controller = useLibraryRecoveryController(options)

    const first = controller.reconnectLibrary()
    const competing = controller.prepareRecoveryBackup()

    expect(competing).toBe(first)
    expect(reconnect).toHaveBeenCalledOnce()
    expect(prepareRestore).not.toHaveBeenCalled()
    expect(controller.busy.value).toBe(true)

    pending.resolve(success(true))
    await expect(first).resolves.toBe(true)
    expect(controller.busy.value).toBe(false)

    await expect(controller.prepareRecoveryBackup()).resolves.toBe(true)
    expect(prepareRestore).toHaveBeenCalledOnce()
  })

  it('reports a stable generic error, releases busy state, and permits retry', async () => {
    const pending = deferred<AppResult<boolean>>()
    const reconnect = vi.fn()
      .mockReturnValueOnce(pending.promise)
      .mockResolvedValueOnce(success(false))
    const options = createOptions({ reconnect })
    const controller = useLibraryRecoveryController(options)

    const first = controller.reconnectLibrary()
    pending.reject(new Error('native command rejected'))
    await expect(first).resolves.toBe(false)

    expect(controller.message.value).toBe(
      '恢复操作没有完成，原资料库状态没有被覆盖，请稍后重试。',
    )
    expect(controller.busy.value).toBe(false)
    expect(options.enterRestarting).not.toHaveBeenCalled()

    await expect(controller.reconnectLibrary()).resolves.toBe(false)
    expect(reconnect).toHaveBeenCalledTimes(2)
  })
})
