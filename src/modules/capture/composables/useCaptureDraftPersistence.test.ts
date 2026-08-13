import { describe, expect, it, vi } from 'vitest'
import { nextTick, ref } from 'vue'
import type { CaptureBatchDetail } from '../../../shared/api/bindings'
import { failure, success } from '../../../shared/api/app-result'
import { useCaptureDraftPersistence } from './useCaptureDraftPersistence'

const detail = {
  batch: { id: 'batch-1', subject: '数学', state: 'organizing', itemCount: 0, draftCount: 1, readyCount: 0, updatedAtUtcMs: 1, revision: 7 },
  items: [], drafts: [], unassignedItemIds: [], pairSuggestions: [],
} as CaptureBatchDetail
const updated = { ...detail, batch: { ...detail.batch, revision: 8 } }

function harness() {
  const active = ref<CaptureBatchDetail | undefined>(detail)
  const blocked = ref(false)
  const update = vi.fn().mockResolvedValue(success(updated))
  const refresh = vi.fn(async () => { active.value = updated })
  const onDetailChange = vi.fn((value: CaptureBatchDetail) => { active.value = value })
  const onBusyChange = vi.fn()
  const onSaveStateChange = vi.fn()
  const onError = vi.fn()
  const controller = useCaptureDraftPersistence({
    desktopAvailable: true,
    activeDetail: () => active.value,
    isBlocked: () => blocked.value,
    update,
    refresh,
    onDetailChange,
    onBusyChange,
    onSaveStateChange,
    onError,
  })
  return { active, blocked, update, refresh, onDetailChange, onBusyChange, onSaveStateChange, onError, controller }
}

function edit(controller: ReturnType<typeof useCaptureDraftPersistence>, note = '备注') {
  controller.updateDraft({ id: 'draft-1' }, '化学', ['函数'], note)
}

describe('useCaptureDraftPersistence', () => {
  it('owns exact revision-aware update input and success projection', async () => {
    const h = harness()
    edit(h.controller)
    await vi.waitFor(() => expect(h.update).toHaveBeenCalledOnce())
    expect(h.update).toHaveBeenCalledWith({
      batchId: 'batch-1', expectedRevision: 7, draftId: 'draft-1',
      subject: '化学', tags: ['函数'], note: '备注',
    })
    await vi.waitFor(() => expect(h.onDetailChange).toHaveBeenCalledWith(updated))
    expect(h.onSaveStateChange).toHaveBeenLastCalledWith('saved')
    expect(h.controller.unsaved.value).toBe(false)
  })

  it('refreshes and retries one revision conflict with the refreshed revision', async () => {
    const h = harness()
    h.update
      .mockResolvedValueOnce(failure('capture_revision_conflict', '批次已更新', true, 'diag-conflict'))
      .mockResolvedValueOnce(success(updated))
    edit(h.controller)
    await vi.waitFor(() => expect(h.update).toHaveBeenCalledTimes(2))
    expect(h.refresh).toHaveBeenCalledWith('batch-1')
    expect(h.update).toHaveBeenNthCalledWith(2, expect.objectContaining({ expectedRevision: 8 }))
  })

  it('retains failed input for explicit retry and owns transport failure copy', async () => {
    const h = harness()
    h.update.mockRejectedValueOnce(new Error('offline')).mockResolvedValueOnce(success(updated))
    edit(h.controller, '保留输入')
    await vi.waitFor(() => expect(h.controller.retryAvailable.value).toBe(true))
    expect(h.onError).toHaveBeenLastCalledWith(
      '草稿文字保存没有完成；本次编辑仍保留在当前输入框中，请再次修改或重试。',
    )
    await h.controller.retry()
    expect(h.onError).toHaveBeenCalledWith('')
    expect(h.update).toHaveBeenLastCalledWith(expect.objectContaining({ note: '保留输入' }))
  })

  it('waits while blocked, flushes when unblocked, and clears other batches', async () => {
    const h = harness()
    h.blocked.value = true
    edit(h.controller)
    expect(h.controller.persistenceBusy.value).toBe(true)
    expect(h.update).not.toHaveBeenCalled()
    h.active.value = { ...detail, batch: { ...detail.batch, id: 'batch-2' } }
    await nextTick()
    h.blocked.value = false
    await nextTick()
    expect(h.update).not.toHaveBeenCalled()
    expect(h.controller.unsaved.value).toBe(false)
  })

  it('rejects inactive input and ignores work after disposal', async () => {
    const h = harness()
    h.active.value = undefined
    edit(h.controller)
    expect(h.update).not.toHaveBeenCalled()
    h.active.value = detail
    h.controller.dispose()
    edit(h.controller)
    await Promise.resolve()
    expect(h.update).not.toHaveBeenCalled()
  })
})
