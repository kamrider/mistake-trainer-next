import { describe, expect, it, vi } from 'vitest'
import type { LegacyImportSummary } from '../../../shared/api/bindings'
import { failure, success, type AppResult } from '../../../shared/api/app-result'
import { useLegacyImportHistory } from './useLegacyImportHistory'

const newest: LegacyImportSummary = {
  importId: 'import-new', memberCount: 2, problemCount: 8, assetCount: 10,
  reviewCount: 6, status: 'completed', createdAtUtcMs: 20, rolledBackAtUtcMs: null,
}
const older: LegacyImportSummary = {
  ...newest, importId: 'import-old', memberCount: 1, problemCount: 3, createdAtUtcMs: 10,
}

function deferred<T>() {
  let resolve!: (value: T) => void
  let reject!: (reason?: unknown) => void
  const promise = new Promise<T>((finish, fail) => { resolve = finish; reject = fail })
  return { promise, resolve, reject }
}

function harness() {
  const listImports = vi.fn(async () => success<LegacyImportSummary[]>([]))
  const controller = useLegacyImportHistory({ listImports })
  return { controller, listImports }
}

describe('useLegacyImportHistory', () => {
  it('lets only the latest overlapping request replace history state', async () => {
    const current = harness()
    const first = deferred<AppResult<LegacyImportSummary[]>>()
    const second = deferred<AppResult<LegacyImportSummary[]>>()
    current.listImports.mockReturnValueOnce(first.promise).mockReturnValueOnce(second.promise)

    const firstLoad = current.controller.reload()
    const secondLoad = current.controller.reload()
    expect(current.controller.loading.value).toBe(true)

    second.resolve(success([newest]))
    expect(await secondLoad).toBe(true)
    expect(current.controller.imports.value).toEqual([newest])
    expect(current.controller.loaded.value).toBe(true)
    expect(current.controller.loading.value).toBe(false)

    first.resolve(success([]))
    expect(await firstLoad).toBe(false)
    expect(current.controller.imports.value).toEqual([newest])
    expect(current.controller.loaded.value).toBe(true)

    const thrown = harness()
    const staleFailure = deferred<AppResult<LegacyImportSummary[]>>()
    const latest = deferred<AppResult<LegacyImportSummary[]>>()
    thrown.listImports.mockReturnValueOnce(staleFailure.promise).mockReturnValueOnce(latest.promise)
    const staleLoad = thrown.controller.reload()
    const latestLoad = thrown.controller.reload()
    latest.resolve(success([newest]))
    expect(await latestLoad).toBe(true)
    staleFailure.reject(new Error('late database error'))
    expect(await staleLoad).toBe(false)
    expect(thrown.controller.errorMessage.value).toBe('')
    expect(thrown.controller.imports.value).toEqual([newest])
  })

  it('distinguishes initial application and transport failures from empty history', async () => {
    const rejected = harness()
    rejected.listImports.mockResolvedValueOnce(failure(
      'legacy_list_failed', '迁移记录读取失败。', true, 'diag-list',
    ))
    expect(await rejected.controller.reload()).toBe(false)
    expect(rejected.controller.errorMessage.value).toBe('迁移记录读取失败。')
    expect(rejected.controller.loaded.value).toBe(false)
    expect(rejected.controller.imports.value).toEqual([])
    expect(rejected.controller.stale.value).toBe(false)

    const thrown = harness()
    thrown.listImports.mockRejectedValueOnce(new Error('database unavailable'))
    expect(await thrown.controller.reload()).toBe(false)
    expect(thrown.controller.errorMessage.value).toBe('迁移记录暂时无法读取，请稍后重试。')
    expect(thrown.controller.loaded.value).toBe(false)
    expect(thrown.controller.loading.value).toBe(false)
  })

  it('represents a successful empty history without an error', async () => {
    const current = harness()

    expect(await current.controller.reload()).toBe(true)
    expect(current.controller.loaded.value).toBe(true)
    expect(current.controller.imports.value).toEqual([])
    expect(current.controller.errorMessage.value).toBe('')
    expect(current.controller.stale.value).toBe(false)
  })

  it('preserves successful rows and marks them stale when refresh fails', async () => {
    const current = harness()
    current.listImports
      .mockResolvedValueOnce(success([newest, older]))
      .mockResolvedValueOnce(failure(
        'legacy_refresh_failed', '迁移记录没有刷新。', true, 'diag-refresh',
      ))

    expect(await current.controller.reload()).toBe(true)
    expect(await current.controller.reload()).toBe(false)

    expect(current.controller.imports.value).toEqual([newest, older])
    expect(current.controller.loaded.value).toBe(true)
    expect(current.controller.errorMessage.value).toBe('迁移记录没有刷新。')
    expect(current.controller.stale.value).toBe(true)
  })
})
