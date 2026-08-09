import { describe, expect, it, vi } from 'vitest'
import type { BackupRestoreCandidate, BackupSummary } from '../../shared/api/bindings'
import { failure, success, type AppResult } from '../../shared/api/app-result'
import { useSettingsBackupOperations } from './useSettingsBackupOperations'

const createdBackup: BackupSummary = {
  formatVersion: 1,
  createdAtUtcMs: 1_725_000_000_000,
  assetCount: 4,
  encryptedBytes: 2_097_152,
  label: 'backup-safe',
  readyForRestore: false,
}

const restoreCandidate: BackupRestoreCandidate = {
  id: 'candidate-1',
  expiresAtUtcMs: 1_725_086_400_000,
  summary: { ...createdBackup, readyForRestore: true },
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
    create: vi.fn(async () => success<BackupSummary | null>(createdBackup)),
    prepareRestore: vi.fn(async () => success<BackupRestoreCandidate | null>(restoreCandidate)),
    restore: vi.fn(async () => success(true)),
  }
  return {
    operations,
    controller: useSettingsBackupOperations(operations),
  }
}

describe('useSettingsBackupOperations', () => {
  it('allows only one backup operation in flight', async () => {
    const gate = deferred<AppResult<BackupSummary | null>>()
    const current = harness()
    current.operations.create.mockReturnValue(gate.promise)

    const creating = current.controller.createBackup()
    await vi.waitFor(() => expect(current.operations.create).toHaveBeenCalledOnce())
    await current.controller.createBackup()
    await current.controller.prepareRestore()
    await current.controller.restoreBackup()

    expect(current.controller.phase.value).toBe('creating')
    expect(current.controller.busy.value).toBe(true)
    expect(current.operations.create).toHaveBeenCalledOnce()
    expect(current.operations.prepareRestore).not.toHaveBeenCalled()
    expect(current.operations.restore).not.toHaveBeenCalled()

    gate.resolve(success(createdBackup))
    await creating
    expect(current.controller.phase.value).toBe('idle')
  })

  it('blocks creation and restore while package preparation is pending', async () => {
    const gate = deferred<AppResult<BackupRestoreCandidate | null>>()
    const current = harness()
    current.operations.prepareRestore.mockReturnValue(gate.promise)

    const preparing = current.controller.prepareRestore()
    await vi.waitFor(() => expect(current.operations.prepareRestore).toHaveBeenCalledOnce())
    await current.controller.prepareRestore()
    await current.controller.createBackup()
    await current.controller.restoreBackup()

    expect(current.operations.prepareRestore).toHaveBeenCalledOnce()
    expect(current.operations.create).not.toHaveBeenCalled()
    expect(current.operations.restore).not.toHaveBeenCalled()

    gate.resolve(success(restoreCandidate))
    await preparing
    expect(current.controller.candidate.value).toEqual(restoreCandidate)
  })

  it('keeps successful restore startup busy and rejects every later action', async () => {
    const current = harness()
    await current.controller.prepareRestore()
    await expect(current.controller.restoreBackup()).resolves.toBe(true)

    expect(current.controller.phase.value).toBe('restoring')
    expect(current.controller.busy.value).toBe(true)
    await current.controller.restoreBackup()
    await current.controller.createBackup()
    await current.controller.prepareRestore()

    expect(current.operations.restore).toHaveBeenCalledOnce()
    expect(current.operations.create).not.toHaveBeenCalled()
    expect(current.operations.prepareRestore).toHaveBeenCalledOnce()
  })

  it('treats native picker cancellation as neutral', async () => {
    const current = harness()
    current.operations.create.mockResolvedValue(success(null))
    current.operations.prepareRestore.mockResolvedValue(success(null))

    await expect(current.controller.createBackup()).resolves.toBe(false)
    await expect(current.controller.prepareRestore()).resolves.toBe(false)

    expect(current.controller.phase.value).toBe('idle')
    expect(current.controller.message.value).toBe('')
  })

  it('keeps a prior creation receipt when a later creation fails', async () => {
    const current = harness()
    await current.controller.createBackup()
    current.operations.create.mockResolvedValue(
      failure('backup_create_failed', '磁盘空间不足。', true, 'diag-create'),
    )

    await expect(current.controller.createBackup()).resolves.toBe(false)

    expect(current.controller.created.value).toEqual(createdBackup)
    expect(current.controller.message.value).toBe('磁盘空间不足。')
    expect(current.controller.phase.value).toBe('idle')
  })

  it('invalidates a stale candidate when later validation fails', async () => {
    const current = harness()
    await current.controller.prepareRestore()
    current.operations.prepareRestore.mockResolvedValue(
      failure('backup_invalid', '备份包校验失败。', false, 'diag-prepare'),
    )

    await expect(current.controller.prepareRestore()).resolves.toBe(false)

    expect(current.controller.candidate.value).toBeUndefined()
    expect(current.controller.message.value).toBe('备份包校验失败。')
  })

  it('keeps the candidate retryable when restore startup fails', async () => {
    const current = harness()
    await current.controller.prepareRestore()
    current.operations.restore.mockResolvedValue(
      failure('backup_restore_failed', '恢复任务没有开始。', true, 'diag-restore'),
    )

    await expect(current.controller.restoreBackup()).resolves.toBe(false)

    expect(current.controller.candidate.value).toEqual(restoreCandidate)
    expect(current.controller.phase.value).toBe('idle')
    expect(current.controller.message.value).toBe('恢复任务没有开始。')
  })

  it('uses stable fallback copy for thrown command failures', async () => {
    const creation = harness()
    creation.operations.create.mockRejectedValue(new Error('offline'))
    await creation.controller.createBackup()
    expect(creation.controller.message.value).toBe(
      '加密备份没有完成，现有资料库未被替换，请检查磁盘空间后重试。',
    )

    const preparation = harness()
    preparation.operations.prepareRestore.mockRejectedValue(new Error('offline'))
    await preparation.controller.prepareRestore()
    expect(preparation.controller.message.value).toBe(
      '备份包没有验证成功；现有资料库未被修改，请稍后重试。',
    )

    const restore = harness()
    restore.controller.candidate.value = restoreCandidate
    restore.operations.restore.mockRejectedValue(new Error('offline'))
    await restore.controller.restoreBackup()
    expect(restore.controller.message.value).toBe(
      '恢复任务没有开始；当前资料库保持不变，请稍后重试。',
    )
    expect(restore.controller.phase.value).toBe('idle')
  })

  it('does not stay busy when a restore command reports false success data', async () => {
    const current = harness()
    current.controller.candidate.value = restoreCandidate
    current.operations.restore.mockResolvedValue(success(false))

    await expect(current.controller.restoreBackup()).resolves.toBe(false)

    expect(current.controller.phase.value).toBe('idle')
    expect(current.controller.message.value).toBe(
      '恢复任务没有开始；当前资料库保持不变，请稍后重试。',
    )
  })
})
