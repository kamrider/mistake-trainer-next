import { describe, expect, it, vi } from 'vitest'
import type {
  DeletedExportSnapshotSummary,
  ExportCreateInput,
  ExportSnapshotSummary,
  GeneratedExportSummary,
} from '../../../shared/api/bindings'
import { failure, success } from '../../../shared/api/app-result'
import { useExportSnapshotMutations } from './useExportSnapshotMutations'

const snapshots: ExportSnapshotSummary[] = [
  { id: 'snapshot-1', title: '本周复盘', problemCount: 12, layout: 'question_answer_alternating', createdAtUtcMs: 1 },
  { id: 'snapshot-2', title: '期中题册', problemCount: 20, layout: 'questions_then_answers', createdAtUtcMs: 2 },
]
const deleted: DeletedExportSnapshotSummary = {
  snapshot: { id: 'deleted-1', title: '旧题册', problemCount: 4, layout: 'original_image_folder', createdAtUtcMs: 0 },
  deletedAtUtcMs: 3,
  purgeAfterUtcMs: 4,
}
const createdSnapshot: ExportSnapshotSummary = {
  id: 'snapshot-created',
  title: '新复盘',
  problemCount: 2,
  layout: 'question_answer_alternating',
  createdAtUtcMs: 5,
}
const createInput: ExportCreateInput = {
  title: '新复盘',
  problemIds: ['problem-1', 'problem-2'],
  layout: 'question_answer_alternating',
}
const generatedExport: GeneratedExportSummary = {
  snapshotId: 'snapshot-1',
  outputName: '本周复盘.docx',
  problemCount: 12,
  layout: 'question_answer_alternating',
}

function deferred<T>() {
  let resolve!: (value: T) => void
  let reject!: (reason?: unknown) => void
  const promise = new Promise<T>((finish, fail) => { resolve = finish; reject = fail })
  return { promise, resolve, reject }
}

function harness() {
  let currentSnapshots = [...snapshots]
  let currentDeleted = [deleted]
  let trashLoaded = false
  const confirmDelete = vi.fn(async () => true)
  const createOperation = vi.fn(async () => success(createdSnapshot))
  const generateOperation = vi.fn(async () => success<GeneratedExportSummary | null>(generatedExport))
  const deleteOperation = vi.fn(async () => success(true))
  const restoreOperation = vi.fn(async () => success(true))
  const listTrash = vi.fn(async () => success(currentDeleted))
  const onSnapshotsChange = vi.fn((items: ExportSnapshotSummary[]) => { currentSnapshots = items })
  const onDeletedSnapshotsChange = vi.fn((items: DeletedExportSnapshotSummary[]) => { currentDeleted = items })
  const onTrashLoadedChange = vi.fn((loaded: boolean) => { trashLoaded = loaded })
  const onError = vi.fn()
  const onStatus = vi.fn()
  const scheduleSync = vi.fn()
  const blocked = vi.fn(() => false)
  const controller = useExportSnapshotMutations({
    snapshots: () => currentSnapshots,
    deletedSnapshots: () => currentDeleted,
    onSnapshotsChange,
    onDeletedSnapshotsChange,
    onTrashLoadedChange,
    onError,
    onStatus,
    scheduleSync,
    blocked,
    confirmDelete,
    createOperation,
    generateOperation,
    deleteOperation,
    restoreOperation,
    listTrash,
  })
  return {
    controller,
    confirmDelete,
    createOperation,
    generateOperation,
    deleteOperation,
    restoreOperation,
    listTrash,
    onSnapshotsChange,
    onDeletedSnapshotsChange,
    onTrashLoadedChange,
    onError,
    onStatus,
    scheduleSync,
    blocked,
    snapshots: () => currentSnapshots,
    deletedSnapshots: () => currentDeleted,
    trashLoaded: () => trashLoaded,
  }
}

describe('useExportSnapshotMutations', () => {
  it('is single-flight from confirmation and releases state after cancellation', async () => {
    const gate = deferred<boolean>()
    const current = harness()
    current.confirmDelete.mockReturnValue(gate.promise)

    const deleting = current.controller.deleteSnapshot('snapshot-1')
    await vi.waitFor(() => expect(current.confirmDelete).toHaveBeenCalledWith(snapshots[0]))
    expect(current.controller.deletingId.value).toBe('snapshot-1')
    expect(current.controller.operationBusy.value).toBe(true)

    await current.controller.deleteSnapshot('snapshot-2')
    await current.controller.restoreSnapshot(deleted)
    await current.controller.createSnapshot(createInput)
    await current.controller.generateSnapshot(snapshots[1]!)
    expect(current.confirmDelete).toHaveBeenCalledOnce()
    expect(current.restoreOperation).not.toHaveBeenCalled()
    expect(current.createOperation).not.toHaveBeenCalled()
    expect(current.generateOperation).not.toHaveBeenCalled()

    gate.resolve(false)
    await deleting
    expect(current.deleteOperation).not.toHaveBeenCalled()
    expect(current.onError).not.toHaveBeenCalled()
    expect(current.controller.deletingId.value).toBe('')
    expect(current.controller.operationBusy.value).toBe(false)
  })

  it('deletes one snapshot, schedules sync, and replaces the refreshed recycle bin', async () => {
    const current = harness()
    const refreshed = [{ ...deleted, snapshot: { ...deleted.snapshot, id: 'snapshot-1', title: '本周复盘' } }]
    current.listTrash.mockResolvedValue(success(refreshed))

    await current.controller.deleteSnapshot('snapshot-1')

    expect(current.deleteOperation).toHaveBeenCalledWith('snapshot-1')
    expect(current.snapshots()).toEqual([snapshots[1]])
    expect(current.deletedSnapshots()).toEqual(refreshed)
    expect(current.scheduleSync).toHaveBeenCalledOnce()
    expect(current.onTrashLoadedChange).toHaveBeenCalledWith(true)
    expect(current.trashLoaded()).toBe(true)
  })

  it('preserves lists for delete failures and reports stable fallback copy', async () => {
    const rejected = harness()
    rejected.deleteOperation.mockResolvedValue(failure(
      'export_delete_failed', '快照没有删除。', true, 'diag-delete',
    ))
    await rejected.controller.deleteSnapshot('snapshot-1')
    expect(rejected.onError).toHaveBeenLastCalledWith('快照没有删除。')
    expect(rejected.snapshots()).toEqual(snapshots)
    expect(rejected.scheduleSync).not.toHaveBeenCalled()

    const thrown = harness()
    thrown.deleteOperation.mockRejectedValue(new Error('offline'))
    await thrown.controller.deleteSnapshot('snapshot-1')
    expect(thrown.onError).toHaveBeenLastCalledWith('导出快照没有删除，请稍后重试。')
    expect(thrown.controller.operationBusy.value).toBe(false)

    const confirmationFailed = harness()
    confirmationFailed.confirmDelete.mockRejectedValue(new Error('dialog failed'))
    await expect(confirmationFailed.controller.deleteSnapshot('snapshot-1')).resolves.toBe(false)
    expect(confirmationFailed.deleteOperation).not.toHaveBeenCalled()
    expect(confirmationFailed.onError).toHaveBeenLastCalledWith('导出快照没有删除，请稍后重试。')
    expect(confirmationFailed.controller.operationBusy.value).toBe(false)
  })

  it('keeps a durable deletion when the recycle-bin refresh fails', async () => {
    const current = harness()
    current.listTrash.mockResolvedValue(failure(
      'export_trash_failed', '回收区读取失败。', true, 'diag-trash',
    ))

    await current.controller.deleteSnapshot('snapshot-1')

    expect(current.snapshots()).toEqual([snapshots[1]])
    expect(current.deletedSnapshots()).toEqual([deleted])
    expect(current.scheduleSync).toHaveBeenCalledOnce()
    expect(current.onError).toHaveBeenLastCalledWith('快照已删除，但回收区暂时没有刷新成功。')

    const thrown = harness()
    thrown.listTrash.mockRejectedValue(new Error('offline'))
    await thrown.controller.deleteSnapshot('snapshot-1')
    expect(thrown.snapshots()).toEqual([snapshots[1]])
    expect(thrown.scheduleSync).toHaveBeenCalledOnce()
    expect(thrown.onError).toHaveBeenLastCalledWith('快照已删除，但回收区暂时没有刷新成功。')
  })

  it('restores one current deleted snapshot and ignores stale targets', async () => {
    const current = harness()

    await current.controller.restoreSnapshot(deleted)

    expect(current.restoreOperation).toHaveBeenCalledWith('deleted-1')
    expect(current.snapshots()).toEqual([deleted.snapshot, ...snapshots])
    expect(current.deletedSnapshots()).toEqual([])
    expect(current.scheduleSync).toHaveBeenCalledOnce()
    expect(current.controller.restoringId.value).toBe('')

    await current.controller.restoreSnapshot(deleted)
    await current.controller.deleteSnapshot('missing')
    expect(current.restoreOperation).toHaveBeenCalledOnce()
    expect(current.deleteOperation).not.toHaveBeenCalled()
  })

  it('preserves lists and error semantics for restore failures', async () => {
    const rejected = harness()
    rejected.restoreOperation.mockResolvedValue(failure(
      'export_restore_failed', '快照没有恢复。', true, 'diag-restore',
    ))
    await rejected.controller.restoreSnapshot(deleted)
    expect(rejected.onError).toHaveBeenLastCalledWith('快照没有恢复。')
    expect(rejected.snapshots()).toEqual(snapshots)

    const thrown = harness()
    thrown.restoreOperation.mockRejectedValue(new Error('offline'))
    await thrown.controller.restoreSnapshot(deleted)
    expect(thrown.onError).toHaveBeenLastCalledWith('导出快照没有恢复，请稍后重试。')
    expect(thrown.controller.operationBusy.value).toBe(false)
  })

  it('creates one deduplicated snapshot and isolates a failed sync notification', async () => {
    const current = harness()
    current.createOperation.mockResolvedValue(success({ ...snapshots[0]!, title: '更新后的复盘' }))
    current.scheduleSync.mockImplementation(() => { throw new Error('timer unavailable') })

    await expect(current.controller.createSnapshot(createInput)).resolves.toBe(true)

    expect(current.createOperation).toHaveBeenCalledWith(createInput)
    expect(current.snapshots()).toEqual([{ ...snapshots[0]!, title: '更新后的复盘' }, snapshots[1]])
    expect(current.onStatus).toHaveBeenLastCalledWith('已保存“更新后的复盘”，随时可以从下方重新生成。')
    expect(current.onError).toHaveBeenLastCalledWith('')
    expect(current.controller.saving.value).toBe(false)
  })

  it('preserves snapshots for create application and transport failures', async () => {
    const rejected = harness()
    rejected.createOperation.mockResolvedValue(failure(
      'export_create_failed', '快照没有保存。', true, 'diag-create',
    ))
    await expect(rejected.controller.createSnapshot(createInput)).resolves.toBe(false)
    expect(rejected.snapshots()).toEqual(snapshots)
    expect(rejected.onError).toHaveBeenLastCalledWith('快照没有保存。')

    const thrown = harness()
    thrown.createOperation.mockRejectedValue(new Error('offline'))
    await expect(thrown.controller.createSnapshot(createInput)).resolves.toBe(false)
    expect(thrown.snapshots()).toEqual(snapshots)
    expect(thrown.onError).toHaveBeenLastCalledWith('导出快照没有保存，请稍后重试。')

    const invalid = harness()
    await expect(invalid.controller.createSnapshot({ ...createInput, title: ' ' })).resolves.toBe(false)
    await expect(invalid.controller.createSnapshot({ ...createInput, problemIds: [] })).resolves.toBe(false)
    expect(invalid.createOperation).not.toHaveBeenCalled()
  })

  it('generates only a current snapshot and treats picker cancellation as neutral', async () => {
    const current = harness()

    await expect(current.controller.generateSnapshot(snapshots[0]!)).resolves.toBe(true)
    expect(current.generateOperation).toHaveBeenCalledWith('snapshot-1')
    expect(current.onStatus).toHaveBeenLastCalledWith('已生成 本周复盘.docx，共 12 题。')
    expect(current.controller.generatingId.value).toBe('')

    current.generateOperation.mockResolvedValueOnce(success(null))
    await expect(current.controller.generateSnapshot(snapshots[1]!)).resolves.toBe(false)
    expect(current.onStatus).toHaveBeenLastCalledWith('')

    await expect(current.controller.generateSnapshot({ ...snapshots[0]!, id: 'stale' })).resolves.toBe(false)
    expect(current.generateOperation).toHaveBeenCalledTimes(2)
  })

  it('reports exact generation failures without changing snapshot state', async () => {
    const rejected = harness()
    rejected.generateOperation.mockResolvedValue(failure(
      'export_generate_failed', '文件没有生成。', true, 'diag-generate',
    ))
    await expect(rejected.controller.generateSnapshot(snapshots[0]!)).resolves.toBe(false)
    expect(rejected.onError).toHaveBeenLastCalledWith('文件没有生成。')
    expect(rejected.snapshots()).toEqual(snapshots)

    const thrown = harness()
    thrown.generateOperation.mockRejectedValue(new Error('folder unavailable'))
    await expect(thrown.controller.generateSnapshot(snapshots[0]!)).resolves.toBe(false)
    expect(thrown.onError).toHaveBeenLastCalledWith('文件没有生成，请检查目标目录空间与权限后重试。')
    expect(thrown.controller.operationBusy.value).toBe(false)
  })

  it('blocks every operation while the page owns an incompatible load', async () => {
    const current = harness()
    current.blocked.mockReturnValue(true)

    await current.controller.createSnapshot(createInput)
    await current.controller.generateSnapshot(snapshots[0]!)
    await current.controller.deleteSnapshot('snapshot-1')
    await current.controller.restoreSnapshot(deleted)

    expect(current.createOperation).not.toHaveBeenCalled()
    expect(current.generateOperation).not.toHaveBeenCalled()
    expect(current.confirmDelete).not.toHaveBeenCalled()
    expect(current.restoreOperation).not.toHaveBeenCalled()
  })

  it('does not turn durable delete and restore results into failures when sync scheduling throws', async () => {
    const deleting = harness()
    deleting.scheduleSync.mockImplementation(() => { throw new Error('timer unavailable') })
    await deleting.controller.deleteSnapshot('snapshot-1')
    expect(deleting.snapshots()).toEqual([snapshots[1]])
    expect(deleting.listTrash).toHaveBeenCalledOnce()
    expect(deleting.onError).not.toHaveBeenCalledWith('导出快照没有删除，请稍后重试。')

    const restoring = harness()
    restoring.scheduleSync.mockImplementation(() => { throw new Error('timer unavailable') })
    await restoring.controller.restoreSnapshot(deleted)
    expect(restoring.snapshots()).toEqual([deleted.snapshot, ...snapshots])
    expect(restoring.deletedSnapshots()).toEqual([])
    expect(restoring.onError).not.toHaveBeenCalledWith('导出快照没有恢复，请稍后重试。')
  })
})
