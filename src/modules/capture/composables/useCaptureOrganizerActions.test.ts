import { describe, expect, it, vi } from 'vitest'
import type { CaptureBatchDetail } from '../../../shared/api/bindings'
import { failure, success, type AppResult } from '../../../shared/api/app-result'
import { useCaptureOrganizerActions } from './useCaptureOrganizerActions'

const detail = {
  batch: { id: 'batch-1', subject: '数学', state: 'organizing', itemCount: 1, draftCount: 1, readyCount: 0, updatedAtUtcMs: 1, revision: 7 },
  items: [], drafts: [], unassignedItemIds: [], pairSuggestions: [],
} as CaptureBatchDetail
const updated = { ...detail, batch: { ...detail.batch, revision: 8 } }

function deferred<T>() {
  let resolve!: (value: T) => void
  let reject!: (reason?: unknown) => void
  const promise = new Promise<T>((finish, fail) => { resolve = finish; reject = fail })
  return { promise, resolve, reject }
}

function harness() {
  let current: CaptureBatchDetail | undefined = detail
  let blocked = false
  const operations = {
    applyLayout: vi.fn().mockResolvedValue(success(updated)),
    assignSubject: vi.fn().mockResolvedValue(success(updated)),
    moveItem: vi.fn().mockResolvedValue(success(updated)),
    stageRole: vi.fn().mockResolvedValue(success(updated)),
    mergeCard: vi.fn().mockResolvedValue(success(updated)),
    deleteDraft: vi.fn().mockResolvedValue(success(updated)),
    applyPairSuggestions: vi.fn().mockResolvedValue(success(updated)),
  }
  const onDetailChange = vi.fn((value: CaptureBatchDetail) => { current = value })
  const onBusyChange = vi.fn((value: boolean) => { blocked = value })
  const onSaveStateChange = vi.fn()
  const onError = vi.fn()
  const onNotice = vi.fn()
  const reloadDetail = vi.fn().mockResolvedValue(undefined)
  const controller = useCaptureOrganizerActions({
    desktopAvailable: true,
    activeDetail: () => current,
    isBlocked: () => blocked,
    onBusyChange,
    onSaveStateChange,
    onDetailChange,
    onError,
    onNotice,
    reloadDetail,
    operations,
  })
  return { controller, operations, onDetailChange, onBusyChange, onSaveStateChange, onError, onNotice, reloadDetail, setCurrent: (value?: CaptureBatchDetail) => { current = value }, setBlocked: (value: boolean) => { blocked = value } }
}

describe('useCaptureOrganizerActions', () => {
  it('builds exact revision-aware inputs for all seven organizer actions', async () => {
    const cases = [
      async () => { const h = harness(); await h.controller.applyLayout('alternating', 2, 1, null); expect(h.operations.applyLayout).toHaveBeenCalledWith({ batchId: 'batch-1', expectedRevision: 7, mode: 'alternating', questionImagesPerDraft: 2, answerImagesPerDraft: 1, splitIndex: null }) },
      async () => { const h = harness(); await h.controller.assignBatchSubject('化学'); expect(h.operations.assignSubject).toHaveBeenCalledWith({ batchId: 'batch-1', expectedRevision: 7, subject: '化学' }) },
      async () => { const h = harness(); await h.controller.moveItem({ itemId: 'item-1', targetDraftId: 'draft-1', targetRole: 'answer', targetPosition: 0 }); expect(h.operations.moveItem).toHaveBeenCalledWith({ batchId: 'batch-1', expectedRevision: 7, itemId: 'item-1', targetDraftId: 'draft-1', targetRole: 'answer', targetPosition: 0 }) },
      async () => { const h = harness(); await h.controller.stageItemRole('item-1', 'question'); expect(h.operations.stageRole).toHaveBeenCalledWith({ batchId: 'batch-1', expectedRevision: 7, itemId: 'item-1', stagedRole: 'question' }) },
      async () => { const h = harness(); await h.controller.mergeCard(['item-1'], null, '物理'); expect(h.operations.mergeCard).toHaveBeenCalledWith({ batchId: 'batch-1', expectedRevision: 7, targetDraftId: null, itemIds: ['item-1'], newDraftSubject: '物理' }) },
      async () => { const h = harness(); await h.controller.deleteDraft('draft-1'); expect(h.operations.deleteDraft).toHaveBeenCalledWith('batch-1', 7, 'draft-1') },
      async () => { const h = harness(); await h.controller.applyPairSuggestions(['pair-1']); expect(h.operations.applyPairSuggestions).toHaveBeenCalledWith({ batchId: 'batch-1', expectedRevision: 7, pairIds: ['pair-1'] }) },
    ]
    for (const run of cases) await run()
  })

  it('owns pair-suggestion success notice and stale-input recovery policy', async () => {
    const successCase = harness()
    await successCase.controller.applyPairSuggestions(['pair-1', 'pair-2'])
    expect(successCase.onNotice).toHaveBeenCalledWith(
      '已把 2 组题面与答案生成采集草稿；确认科目后再保存到正式题库。',
    )

    const stale = harness()
    stale.operations.applyPairSuggestions.mockResolvedValue(
      failure('capture_input_invalid', 'raw invalid', true, 'diag-invalid'),
    )
    await stale.controller.applyPairSuggestions(['pair-1'])
    expect(stale.onError).toHaveBeenLastCalledWith(
      '这组题答素材刚刚被移动、改角色或已加入其他题卡，已刷新并保留你的现有整理。',
    )
    expect(stale.reloadDetail).toHaveBeenCalledWith('batch-1')
  })

  it('owns busy and save-state transitions for persisted organizer changes', async () => {
    const h = harness()
    await h.controller.moveItem({ itemId: 'item-1', targetDraftId: null, targetRole: null, targetPosition: 0 })
    expect(h.onBusyChange.mock.calls).toEqual([[true], [false]])
    expect(h.onSaveStateChange.mock.calls).toEqual([['saving'], ['saved']])
    expect(h.onDetailChange).toHaveBeenCalledWith(updated)
  })

  it('reports command errors and reloads only the still-active batch on revision conflict', async () => {
    const h = harness()
    h.operations.assignSubject.mockResolvedValue(
      failure('capture_revision_conflict', '批次已更新', true, 'diag-revision'),
    )
    await h.controller.assignBatchSubject('化学')
    expect(h.onSaveStateChange).toHaveBeenLastCalledWith('error')
    expect(h.onError).toHaveBeenCalledWith('批次已更新')
    expect(h.reloadDetail).toHaveBeenCalledWith('batch-1')
  })

  it('does not apply a late result after leaving the batch', async () => {
    const gate = deferred<AppResult<CaptureBatchDetail>>()
    const h = harness()
    h.operations.moveItem.mockReturnValue(gate.promise)
    const moving = h.controller.moveItem({ itemId: 'item-1', targetDraftId: null, targetRole: null, targetPosition: 0 })
    await vi.waitFor(() => expect(h.operations.moveItem).toHaveBeenCalledOnce())
    h.setCurrent(undefined)
    gate.resolve(success(updated))
    await moving
    expect(h.onDetailChange).not.toHaveBeenCalled()
    expect(h.onSaveStateChange).toHaveBeenLastCalledWith('idle')
  })

  it('ignores late errors and results after the active revision changes', async () => {
    const rejected = deferred<AppResult<CaptureBatchDetail>>()
    const failed = harness()
    failed.operations.assignSubject.mockReturnValue(rejected.promise)
    const assigning = failed.controller.assignBatchSubject('化学')
    await vi.waitFor(() => expect(failed.operations.assignSubject).toHaveBeenCalledOnce())
    failed.setCurrent(updated)
    rejected.resolve(failure('capture_revision_conflict', '过期冲突', true, 'diag-late'))
    await assigning
    expect(failed.onError).toHaveBeenCalledTimes(1)
    expect(failed.onError).toHaveBeenLastCalledWith('')
    expect(failed.reloadDetail).not.toHaveBeenCalled()
    expect(failed.onSaveStateChange).toHaveBeenLastCalledWith('idle')

    const thrown = deferred<AppResult<CaptureBatchDetail>>()
    const errored = harness()
    errored.operations.mergeCard.mockReturnValue(thrown.promise)
    const merging = errored.controller.mergeCard(['item-1'], null, null)
    await vi.waitFor(() => expect(errored.operations.mergeCard).toHaveBeenCalledOnce())
    errored.setCurrent(undefined)
    thrown.reject(new Error('late offline'))
    await merging
    expect(errored.onError).toHaveBeenCalledTimes(1)
    expect(errored.onError).toHaveBeenLastCalledWith('')
    expect(errored.onSaveStateChange).toHaveBeenLastCalledWith('idle')
  })

  it('uses action-specific fallback copy and ignores blocked or empty actions', async () => {
    const h = harness()
    h.operations.mergeCard.mockRejectedValue(new Error('offline'))
    await h.controller.mergeCard(['item-1'], null, null)
    expect(h.onError).toHaveBeenCalledWith('题卡没有保存成功，图片仍保留在原位置。')
    expect(h.onSaveStateChange).toHaveBeenLastCalledWith('error')

    const blocked = harness()
    blocked.setBlocked(true)
    await blocked.controller.assignBatchSubject('化学')
    await blocked.controller.mergeCard([], null, null)
    expect(blocked.operations.assignSubject).not.toHaveBeenCalled()
    expect(blocked.operations.mergeCard).not.toHaveBeenCalled()
  })
})
