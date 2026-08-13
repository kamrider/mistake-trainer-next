import { describe, expect, it, vi } from 'vitest'
import { failure, success, type AppResult } from '../../shared/api/app-result'
import type {
  StorageLocationStatus,
  StorageMigrationReceipt,
} from '../../shared/api/bindings'
import {
  useSettingsStorageLifecycle,
  type SettingsStorageLifecycleOptions,
} from './useSettingsStorageLifecycle'

const status: StorageLocationStatus = {
  kind: 'custom',
  locationLabel: '自定义位置 · StudyDisk',
  databaseBytes: 4096,
  assetBytes: 8192,
  migrationPending: false,
}

function receipt(outcome: StorageMigrationReceipt['outcome']): StorageMigrationReceipt {
  return {
    outcome,
    destinationLabel: '自定义位置 · StudyDisk',
    copiedAssetCount: 4,
    copiedBytes: 16_384,
  }
}

function deferred<T>() {
  let resolve!: (value: T) => void
  const promise = new Promise<T>((finish) => { resolve = finish })
  return { promise, resolve }
}

function createHarness(overrides: Partial<SettingsStorageLifecycleOptions> = {}) {
  const operations = {
    status: vi.fn(async () => success(status)),
    receipt: vi.fn(async () => success<StorageMigrationReceipt | null>(receipt('moved'))),
    migrate: vi.fn(async () => success<StorageMigrationReceipt | null>(null)),
  }
  const enterRestarting = vi.fn()
  const restoreMigrationFocus = vi.fn(async () => undefined)
  const controller = useSettingsStorageLifecycle({
    operations,
    enterRestarting,
    restoreMigrationFocus,
    ...overrides,
  })
  return { controller, operations, enterRestarting, restoreMigrationFocus }
}

describe('useSettingsStorageLifecycle', () => {
  it('loads status and clears stale data on application and transport failures', async () => {
    const current = createHarness()
    await expect(current.controller.loadStatus()).resolves.toBe(true)
    expect(current.controller.status.value).toEqual(status)

    current.operations.status.mockResolvedValueOnce(failure(
      'storage_status_failed', '资料库容量读取失败。', true, 'storage-status',
    ))
    await expect(current.controller.loadStatus()).resolves.toBe(false)
    expect(current.controller.status.value).toBeUndefined()
    expect(current.controller.statusMessage.value).toBe('资料库容量读取失败。')

    current.operations.status.mockRejectedValueOnce(new Error('native unavailable'))
    await expect(current.controller.loadStatus()).resolves.toBe(false)
    expect(current.controller.statusMessage.value).toBe(
      '资料库容量暂时无法读取；迁移入口不会使用猜测数据。',
    )
  })

  it('exposes the explicit browser-preview message without probing native storage', () => {
    const current = createHarness()
    current.controller.showBrowserPreview()

    expect(current.controller.statusMessage.value).toBe(
      '容量和迁移会在 Windows 桌面应用中显示；浏览器预览不会读取本机资料。',
    )
    expect(current.operations.status).not.toHaveBeenCalled()
  })

  it.each([
    ['moved', 'success', '资料库已安全迁移', '4 个加密资源', '16.0 KB'],
    ['cleanup_required', 'warning', '新位置已启用，原副本需手动清理', '4 个加密资源', '16.0 KB'],
    ['rolled_back', 'warning', '迁移未生效，已自动回到原位置', '原资料库保持完整', null],
    ['scheduled', 'warning', '迁移等待安全重启', '4 个加密资源', '16.0 KB'],
  ] as const)('projects a privacy-safe %s migration receipt', async (
    outcome,
    kind,
    title,
    detail,
    bytes,
  ) => {
    const current = createHarness()
    current.operations.receipt.mockResolvedValueOnce(success(receipt(outcome)))

    await expect(current.controller.loadReceipt()).resolves.toBe(true)
    expect(current.controller.receiptCopy.value).toEqual(expect.objectContaining({ kind, title }))
    expect(current.controller.receiptCopy.value?.detail).toContain(detail)
    if (bytes) expect(current.controller.receiptCopy.value?.detail).toContain(bytes)
  })

  it('silently ignores a missing or failed supplementary receipt', async () => {
    const current = createHarness()
    current.operations.receipt.mockResolvedValueOnce(success(null))
    await expect(current.controller.loadReceipt()).resolves.toBe(false)
    expect(current.controller.receipt.value).toBeUndefined()

    current.operations.receipt.mockRejectedValueOnce(new Error('receipt unavailable'))
    await expect(current.controller.loadReceipt()).resolves.toBe(false)
    expect(current.controller.receipt.value).toBeUndefined()
  })

  it('closes and restores focus when native folder selection is cancelled', async () => {
    const current = createHarness()
    current.controller.openMigration()

    await expect(current.controller.confirmMigration()).resolves.toBe(false)
    expect(current.controller.dialogOpen.value).toBe(false)
    expect(current.controller.busy.value).toBe(false)
    expect(current.controller.migrationMessage.value).toBe('')
    expect(current.restoreMigrationFocus).toHaveBeenCalledOnce()
    expect(current.enterRestarting).not.toHaveBeenCalled()
  })

  it('keeps backend and transport failures open and retryable', async () => {
    const rejected = createHarness()
    rejected.operations.migrate.mockResolvedValueOnce(failure(
      'storage_migration_failed',
      '目标磁盘空间不足，原资料库保持不变。',
      true,
      'storage-migrate',
    ))
    rejected.controller.openMigration()
    await expect(rejected.controller.confirmMigration()).resolves.toBe(false)
    expect(rejected.controller.dialogOpen.value).toBe(true)
    expect(rejected.controller.busy.value).toBe(false)
    expect(rejected.controller.migrationMessage.value).toBe('目标磁盘空间不足，原资料库保持不变。')

    const thrown = createHarness()
    thrown.operations.migrate.mockRejectedValueOnce(new Error('native unavailable'))
    thrown.controller.openMigration()
    await expect(thrown.controller.confirmMigration()).resolves.toBe(false)
    expect(thrown.controller.migrationMessage.value).toBe(
      '迁移没有开始或没有完成，原资料库保持不变，请检查目标磁盘后重试。',
    )
    expect(thrown.controller.busy.value).toBe(false)
  })

  it('enters restart and deliberately stays busy after a scheduled migration', async () => {
    const current = createHarness()
    current.operations.migrate.mockResolvedValueOnce(success(receipt('scheduled')))
    current.controller.openMigration()

    await expect(current.controller.confirmMigration()).resolves.toBe(true)
    expect(current.enterRestarting).toHaveBeenCalledOnce()
    expect(current.controller.dialogOpen.value).toBe(true)
    expect(current.controller.busy.value).toBe(true)
    expect(current.restoreMigrationFocus).not.toHaveBeenCalled()
  })

  it('rejects competing confirms and close requests while migration is active', async () => {
    const pending = deferred<AppResult<StorageMigrationReceipt | null>>()
    const current = createHarness()
    current.operations.migrate.mockReturnValueOnce(pending.promise)
    current.controller.openMigration()

    const first = current.controller.confirmMigration()
    expect(current.controller.busy.value).toBe(true)
    await expect(current.controller.confirmMigration()).resolves.toBe(false)
    await expect(current.controller.closeMigration()).resolves.toBe(false)
    expect(current.operations.migrate).toHaveBeenCalledOnce()
    expect(current.controller.dialogOpen.value).toBe(true)

    pending.resolve(success(null))
    await expect(first).resolves.toBe(false)
    expect(current.controller.dialogOpen.value).toBe(false)
    expect(current.restoreMigrationFocus).toHaveBeenCalledOnce()
  })
})
