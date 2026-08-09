import { describe, expect, it, vi } from 'vitest'
import type { SyncConflictSummary } from '../../../shared/api/bindings'
import { failure, success } from '../../../shared/api/app-result'
import { useSyncConflictOperations } from './useSyncConflictOperations'

const noteConflict = {
  id: 'conflict-note',
  entityType: 'problem',
  entityId: 'problem-1',
  entityLabel: '数学',
  fieldName: 'note',
  localValue: { kind: 'string', value: '先检查定义域' },
  remoteValue: { kind: 'string', value: '先通分' },
  createdAtUtcMs: 1_725_000_000_000,
} as SyncConflictSummary

const tagConflict = {
  ...noteConflict,
  id: 'conflict-tags',
  fieldName: 'tags',
} as SyncConflictSummary

function deferred<T>() {
  let resolve!: (value: T) => void
  let reject!: (reason?: unknown) => void
  const promise = new Promise<T>((finish, fail) => { resolve = finish; reject = fail })
  return { promise, resolve, reject }
}

function harness() {
  const listConflicts = vi.fn(async () => success<SyncConflictSummary[]>([]))
  const scheduleSync = vi.fn()
  const onChanged = vi.fn()
  const controller = useSyncConflictOperations({ listConflicts, scheduleSync, onChanged })
  return { controller, listConflicts, scheduleSync, onChanged }
}

describe('useSyncConflictOperations', () => {
  it('lets only the latest overlapping list request commit state', async () => {
    const current = harness()
    const first = deferred<ReturnType<typeof success<SyncConflictSummary[]>>>()
    const second = deferred<ReturnType<typeof success<SyncConflictSummary[]>>>()
    current.listConflicts
      .mockReturnValueOnce(first.promise)
      .mockReturnValueOnce(second.promise)

    const older = current.controller.reload()
    const newer = current.controller.reload()
    expect(current.controller.loading.value).toBe(true)

    second.resolve(success([tagConflict]))
    await expect(newer).resolves.toBe(true)
    expect(current.controller.conflicts.value).toEqual([tagConflict])
    expect(current.controller.loading.value).toBe(false)

    first.resolve(success([noteConflict]))
    await expect(older).resolves.toBe(false)
    expect(current.controller.conflicts.value).toEqual([tagConflict])
  })

  it('ignores a stale thrown list request and preserves the latest success', async () => {
    const current = harness()
    const first = deferred<ReturnType<typeof success<SyncConflictSummary[]>>>()
    current.listConflicts
      .mockReturnValueOnce(first.promise)
      .mockResolvedValueOnce(success([tagConflict]))

    const older = current.controller.reload()
    await expect(current.controller.reload()).resolves.toBe(true)
    first.reject(new Error('stale offline result'))

    await expect(older).resolves.toBe(false)
    expect(current.controller.conflicts.value).toEqual([tagConflict])
    expect(current.controller.errorMessage.value).toBe('')
  })

  it('keeps the last conflict snapshot for current list failures', async () => {
    const current = harness()
    current.listConflicts
      .mockResolvedValueOnce(success([noteConflict]))
      .mockResolvedValueOnce(failure('sync_list_failed', '暂时读不到冲突。', true, 'diag-list'))
      .mockRejectedValueOnce(new Error('offline'))

    await expect(current.controller.reload()).resolves.toBe(true)
    await expect(current.controller.reload()).resolves.toBe(false)
    expect(current.controller.conflicts.value).toEqual([noteConflict])
    expect(current.controller.errorMessage.value).toBe('暂时读不到冲突。')

    await expect(current.controller.reload()).resolves.toBe(false)
    expect(current.controller.conflicts.value).toEqual([noteConflict])
    expect(current.controller.errorMessage.value).toBe(
      '同步冲突暂时无法读取；本机与云端内容都没有被修改。',
    )
    expect(current.controller.loading.value).toBe(false)
  })

  it('rejects resolutions while a list or another resolution is in flight', async () => {
    const current = harness()
    const listGate = deferred<ReturnType<typeof success<SyncConflictSummary[]>>>()
    current.listConflicts.mockReturnValueOnce(listGate.promise)
    const listRequest = current.controller.reload()
    const blockedByList = vi.fn(async () => success<SyncConflictSummary[]>([]))

    await expect(current.controller.resolve('field:note', blockedByList, '已保存')).resolves.toBe(false)
    expect(blockedByList).not.toHaveBeenCalled()
    listGate.resolve(success([noteConflict]))
    await listRequest

    const resolveGate = deferred<ReturnType<typeof success<SyncConflictSummary[]>>>()
    const firstOperation = vi.fn(() => resolveGate.promise)
    const first = current.controller.resolve('field:note', firstOperation, '笔记已保存。')
    const blockedByMutation = vi.fn(async () => success<SyncConflictSummary[]>([]))

    await expect(current.controller.resolve('field:tags', blockedByMutation, '标签已保存。')).resolves.toBe(false)
    expect(blockedByMutation).not.toHaveBeenCalled()
    expect(current.controller.busyKey.value).toBe('field:note')
    resolveGate.resolve(success([]))
    await expect(first).resolves.toBe(true)
    expect(current.controller.busyKey.value).toBe('')
  })

  it('preserves conflicts and exact error semantics for resolution failures', async () => {
    const current = harness()
    current.listConflicts.mockResolvedValueOnce(success([noteConflict]))
    await current.controller.reload()

    await expect(current.controller.resolve(
      'field:note',
      async () => failure('sync_resolve_failed', '冲突没有处理。', false, 'diag-resolve'),
      '不应显示',
    )).resolves.toBe(false)
    expect(current.controller.conflicts.value).toEqual([noteConflict])
    expect(current.controller.errorMessage.value).toBe('冲突没有处理。')
    expect(current.scheduleSync).not.toHaveBeenCalled()

    await expect(current.controller.resolve(
      'field:note',
      async () => { throw new Error('offline') },
      '不应显示',
    )).resolves.toBe(false)
    expect(current.controller.conflicts.value).toEqual([noteConflict])
    expect(current.controller.errorMessage.value).toBe('这次选择没有保存，本机与云端内容都保持不变。')
    expect(current.controller.busyKey.value).toBe('')
  })

  it('commits a durable resolution even when notification side effects throw', async () => {
    const current = harness()
    current.scheduleSync.mockImplementation(() => { throw new Error('timer unavailable') })
    current.onChanged.mockImplementation(() => { throw new Error('listener failed') })

    await expect(current.controller.resolve(
      'field:note',
      async () => success([tagConflict]),
      '笔记已采用云端版本。',
    )).resolves.toBe(true)

    expect(current.controller.conflicts.value).toEqual([tagConflict])
    expect(current.controller.statusMessage.value).toBe('笔记已采用云端版本。')
    expect(current.controller.errorMessage.value).toBe('')
    expect(current.scheduleSync).toHaveBeenCalledOnce()
    expect(current.onChanged).toHaveBeenCalledOnce()
    expect(current.controller.busyKey.value).toBe('')
  })

  it('coalesces reloads requested during a resolution and runs one afterward', async () => {
    const current = harness()
    const resolveGate = deferred<ReturnType<typeof success<SyncConflictSummary[]>>>()
    const operation = current.controller.resolve(
      'field:note',
      () => resolveGate.promise,
      '笔记已采用云端版本。',
    )

    await expect(current.controller.reload()).resolves.toBe(false)
    await expect(current.controller.reload()).resolves.toBe(false)
    expect(current.listConflicts).not.toHaveBeenCalled()

    current.listConflicts.mockResolvedValueOnce(success([tagConflict]))
    resolveGate.resolve(success([]))
    await expect(operation).resolves.toBe(true)
    await vi.waitFor(() => expect(current.listConflicts).toHaveBeenCalledOnce())
    await vi.waitFor(() => expect(current.controller.conflicts.value).toEqual([tagConflict]))
  })
})
