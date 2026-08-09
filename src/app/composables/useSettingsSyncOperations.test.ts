import { describe, expect, it, vi } from 'vitest'
import type {
  SettingsOverview,
  SyncNowReport,
} from '../../shared/api/bindings'
import { failure, success, type AppResult } from '../../shared/api/app-result'
import { useSettingsSyncOperations } from './useSettingsSyncOperations'

const report: SyncNowReport = {
  pushedOperationCount: 1,
  uploadedAssetCount: 0,
  pulledChangeCount: 2,
  downloadedAssetCount: 0,
  finalCursor: 3,
}
const overview: SettingsOverview = {
  activeProblemCount: 8,
  archivedProblemCount: 2,
  trashedProblemCount: 1,
  pendingOperationCount: 0,
  failedOperationCount: 0,
  unresolvedConflictCount: 0,
  localEncryptionReady: true,
  cloudSyncConfigured: true,
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
    sync: vi.fn().mockResolvedValue(success(report)),
    refreshOverview: vi.fn().mockResolvedValue(success(overview)),
    applyOverview: vi.fn(),
    refreshConflicts: vi.fn().mockResolvedValue(true),
  }
  return {
    operations,
    controller: useSettingsSyncOperations(operations),
  }
}

describe('useSettingsSyncOperations', () => {
  it('reports authoritative counts and refreshes both supplementary surfaces', async () => {
    const h = harness()

    await expect(h.controller.syncNow()).resolves.toBe(true)

    expect(h.controller.message.value).toBe('同步完成：上传 1 项，拉取 2 项。')
    expect(h.operations.applyOverview).toHaveBeenCalledWith(overview)
    expect(h.operations.refreshConflicts).toHaveBeenCalledOnce()
    expect(h.controller.busy.value).toBe(false)
  })

  it('keeps successful sync truthful when overview refresh fails', async () => {
    const rejected = harness()
    rejected.operations.refreshOverview.mockResolvedValue(
      failure('settings_failed', '统计读取失败。', true, 'diag-overview'),
    )
    await rejected.controller.syncNow()
    expect(rejected.controller.message.value).toBe(
      '同步完成：上传 1 项，拉取 2 项；顶部资料库统计暂时没有刷新。',
    )

    const thrown = harness()
    thrown.operations.refreshOverview.mockRejectedValue(new Error('offline'))
    await thrown.controller.syncNow()
    expect(thrown.controller.message.value).toContain('顶部资料库统计暂时没有刷新')
    expect(thrown.controller.message.value).not.toContain('同步请求没有完成')
  })

  it('keeps successful sync truthful when conflict refresh fails', async () => {
    const rejected = harness()
    rejected.operations.refreshConflicts.mockResolvedValue(false)
    await rejected.controller.syncNow()
    expect(rejected.controller.message.value).toBe(
      '同步完成：上传 1 项，拉取 2 项；同步冲突列表暂时没有刷新。',
    )

    const thrown = harness()
    thrown.operations.refreshConflicts.mockRejectedValue(new Error('offline'))
    await thrown.controller.syncNow()
    expect(thrown.controller.message.value).toContain('同步冲突列表暂时没有刷新')
    expect(thrown.controller.message.value).not.toContain('同步请求没有完成')
  })

  it('names both stale surfaces without revoking successful synchronization', async () => {
    const h = harness()
    h.operations.refreshOverview.mockRejectedValue(new Error('overview offline'))
    h.operations.refreshConflicts.mockResolvedValue(false)

    await expect(h.controller.syncNow()).resolves.toBe(true)

    expect(h.controller.message.value).toBe(
      '同步完成：上传 1 项，拉取 2 项；顶部资料库统计、同步冲突列表暂时没有刷新。',
    )
  })

  it('uses primary command failures and skips supplementary refreshes', async () => {
    const rejected = harness()
    rejected.operations.sync.mockResolvedValue(
      failure('sync_failed', '云端暂时不可用。', true, 'diag-sync'),
    )
    await expect(rejected.controller.syncNow()).resolves.toBe(false)
    expect(rejected.controller.message.value).toBe('云端暂时不可用。')
    expect(rejected.operations.refreshOverview).not.toHaveBeenCalled()
    expect(rejected.operations.refreshConflicts).not.toHaveBeenCalled()

    const thrown = harness()
    thrown.operations.sync.mockRejectedValue(new Error('offline'))
    await expect(thrown.controller.syncNow()).resolves.toBe(false)
    expect(thrown.controller.message.value).toBe(
      '同步请求没有完成，待同步变更会保留并等待下次重试。',
    )
    expect(thrown.operations.refreshOverview).not.toHaveBeenCalled()
  })

  it('admits only one page-level manual sync at a time', async () => {
    const gate = deferred<AppResult<SyncNowReport>>()
    const h = harness()
    h.operations.sync.mockReturnValueOnce(gate.promise)

    const first = h.controller.syncNow()
    await vi.waitFor(() => expect(h.operations.sync).toHaveBeenCalledOnce())
    await expect(h.controller.syncNow()).resolves.toBe(false)
    expect(h.controller.busy.value).toBe(true)

    gate.resolve(success(report))
    await expect(first).resolves.toBe(true)
    expect(h.operations.sync).toHaveBeenCalledOnce()
    expect(h.controller.busy.value).toBe(false)
  })
})
