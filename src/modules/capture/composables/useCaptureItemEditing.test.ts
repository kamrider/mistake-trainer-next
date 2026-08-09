import { describe, expect, it, vi } from 'vitest'
import type {
  CaptureBatchDetail,
  CaptureCropApplyReport,
  CaptureCropRecipe,
  CaptureItemPreview,
  CaptureItemSummary,
} from '../../../shared/api/bindings'
import { failure, success, type AppResult } from '../../../shared/api/app-result'
import { useCaptureItemEditing } from './useCaptureItemEditing'

const sourceItem = {
  id: 'item-1', sourceName: '题目.png', sourceSequence: 0, mediaType: 'image/png',
  byteLength: 100, width: 1200, height: 900, stagedRole: 'question', draftId: null,
  role: null, position: null, cropDerivationId: null, cropSourceItemId: null,
} satisfies CaptureItemSummary
const derivedItem = {
  ...sourceItem, id: 'derived-1', sourceName: '题目-裁剪.png', sourceSequence: 1,
  cropDerivationId: 'crop-1', cropSourceItemId: 'item-1',
} satisfies CaptureItemSummary
const detail = {
  batch: {
    id: 'batch-1', subject: '数学', state: 'organizing', itemCount: 2,
    draftCount: 1, readyCount: 0, updatedAtUtcMs: 1, revision: 7,
  },
  items: [sourceItem, derivedItem], drafts: [], unassignedItemIds: [], pairSuggestions: [],
} satisfies CaptureBatchDetail
const updated = {
  ...detail,
  batch: { ...detail.batch, revision: 8 },
} satisfies CaptureBatchDetail
const recipes = [{
  rect: { x: 0.1, y: 0.2, width: 0.7, height: 0.6 },
  rotationDegrees: 90,
  outputMediaType: 'image/png',
  maxEdge: 4096,
  jpegQuality: 90,
}] satisfies CaptureCropRecipe[]

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
    remove: vi.fn().mockResolvedValue(success(updated)),
    preview: vi.fn().mockResolvedValue(success({
      itemId: 'item-1', mediaType: 'image/png', dataUrl: 'data:image/png;base64,source',
    } satisfies CaptureItemPreview)),
    apply: vi.fn().mockResolvedValue(success({
      detail: updated,
      operationId: 'operation-1',
      sourceItemId: 'item-1',
      derivedItemIds: ['derived-2'],
      derivationIds: ['crop-2'],
    } satisfies CaptureCropApplyReport)),
    revert: vi.fn().mockResolvedValue(success(updated)),
  }
  const confirm = vi.fn().mockResolvedValue(true)
  const onBusyChange = vi.fn((value: boolean) => { blocked = value })
  const onSaveStateChange = vi.fn()
  const onDetailChange = vi.fn((value: CaptureBatchDetail) => { current = value })
  const onError = vi.fn()
  const invalidatePreview = vi.fn()
  const loadBatches = vi.fn().mockResolvedValue(undefined)
  const loadDetail = vi.fn().mockResolvedValue(undefined)
  const controller = useCaptureItemEditing({
    desktopAvailable: true,
    activeDetail: () => current,
    isBlocked: () => blocked,
    onBusyChange,
    onSaveStateChange,
    onDetailChange,
    onError,
    confirm,
    invalidatePreview,
    loadBatches,
    loadDetail,
    operations,
  })
  return {
    controller, operations, confirm, onBusyChange, onSaveStateChange, onDetailChange,
    onError, invalidatePreview, loadBatches, loadDetail,
    setCurrent: (value?: CaptureBatchDetail) => { current = value },
    setBlocked: (value: boolean) => { blocked = value },
  }
}

describe('useCaptureItemEditing', () => {
  it('builds exact confirmation requests and revision-aware command inputs', async () => {
    const remove = harness()
    await remove.controller.removeItem('item-1')
    expect(remove.confirm).toHaveBeenCalledWith({
      eyebrow: '采集图片 · 删除确认',
      title: '删除这张采集图片？',
      description: '图片会从当前批次移除；如果没有其他引用，对应的加密资产也会被清理。',
      confirmLabel: '删除图片',
      cancelLabel: '保留图片',
      tone: 'danger',
    })
    expect(remove.operations.remove).toHaveBeenCalledWith('batch-1', 7, 'item-1')

    const crop = harness()
    await crop.controller.openCropEditor('item-1')
    expect(crop.operations.preview).toHaveBeenCalledWith('batch-1', 'item-1')
    expect(crop.controller.cropEditor.value).toEqual({
      itemId: 'item-1', itemName: '题目.png', dataUrl: 'data:image/png;base64,source',
    })
    const report = await crop.controller.applyCrop(recipes)
    expect(crop.operations.apply).toHaveBeenCalledWith({
      batchId: 'batch-1', expectedRevision: 7, itemId: 'item-1', recipes,
    })
    expect(report).toMatchObject({ sourceItemId: 'item-1', derivedItemIds: ['derived-2'] })

    const revert = harness()
    await revert.controller.revertCrop('crop-1')
    expect(revert.confirm).toHaveBeenCalledWith({
      eyebrow: '裁剪结果 · 恢复确认',
      title: '恢复裁剪前的原图？',
      description: '这次裁出的所有区域会从采集工作台移除，原图会回到原来的位置。',
      confirmLabel: '恢复原图',
      cancelLabel: '保留裁剪结果',
      tone: 'warning',
    })
    expect(revert.operations.revert).toHaveBeenCalledWith({
      batchId: 'batch-1', expectedRevision: 7, derivationId: 'crop-1',
    })
  })

  it('cancels safely and revalidates the target after confirmation', async () => {
    const cancelled = harness()
    cancelled.confirm.mockResolvedValue(false)
    await cancelled.controller.removeItem('item-1')
    expect(cancelled.operations.remove).not.toHaveBeenCalled()

    const confirmation = deferred<boolean>()
    const switched = harness()
    switched.confirm.mockReturnValue(confirmation.promise)
    const removing = switched.controller.removeItem('item-1')
    await vi.waitFor(() => expect(switched.confirm).toHaveBeenCalledOnce())
    switched.setCurrent({ ...detail, batch: { ...detail.batch, id: 'batch-2' } })
    confirmation.resolve(true)
    await removing
    expect(switched.operations.remove).not.toHaveBeenCalled()

    const missing = harness()
    await missing.controller.revertCrop('missing-crop')
    await missing.controller.openCropEditor('derived-1')
    expect(missing.confirm).toHaveBeenCalledOnce()
    expect(missing.operations.revert).not.toHaveBeenCalled()
    expect(missing.operations.preview).not.toHaveBeenCalled()
  })

  it('owns success state, editor lifecycle, cache invalidation, and list refresh', async () => {
    const remove = harness()
    await remove.controller.removeItem('item-1')
    expect(remove.onBusyChange.mock.calls).toEqual([[true], [false]])
    expect(remove.onDetailChange).toHaveBeenCalledWith(updated)
    expect(remove.invalidatePreview).toHaveBeenCalledWith('item-1')

    const crop = harness()
    await crop.controller.openCropEditor('item-1')
    await crop.controller.applyCrop(recipes)
    expect(crop.onDetailChange).toHaveBeenCalledWith(updated)
    expect(crop.invalidatePreview).toHaveBeenCalledWith('item-1')
    expect(crop.controller.cropEditor.value).toBeUndefined()
    expect(crop.onSaveStateChange).toHaveBeenCalledWith('saved')
    expect(crop.loadBatches).toHaveBeenCalledOnce()

    const revert = harness()
    await revert.controller.revertCrop('crop-1')
    expect(revert.invalidatePreview).toHaveBeenCalledWith('derived-1')
    expect(revert.onDetailChange).toHaveBeenCalledWith(updated)
    expect(revert.onSaveStateChange).toHaveBeenCalledWith('saved')
    expect(revert.loadBatches).toHaveBeenCalledOnce()
  })

  it('forwards command errors, refreshes conflicts, and preserves fallback copy', async () => {
    const conflict = harness()
    conflict.operations.apply.mockResolvedValue(
      failure('capture_revision_conflict', '批次已更新', true, 'diag-conflict'),
    )
    await conflict.controller.openCropEditor('item-1')
    await conflict.controller.applyCrop(recipes)
    expect(conflict.onError).toHaveBeenLastCalledWith('批次已更新')
    expect(conflict.loadDetail).toHaveBeenCalledWith('batch-1')
    expect(conflict.controller.cropEditor.value).toBeDefined()

    const cases = [
      ['remove', '图片没有删除成功。'],
      ['preview', '没有读取到裁剪大图，原图仍然安全保留，请重试。'],
      ['apply', '裁剪没有保存，原图和当前分组均未改变，请重试。'],
      ['revert', '没有恢复成功，现有图片保持不变。'],
    ] as const
    for (const [operation, copy] of cases) {
      const current = harness()
      current.operations[operation].mockRejectedValue(new Error('offline'))
      if (operation === 'remove') await current.controller.removeItem('item-1')
      if (operation === 'preview') await current.controller.openCropEditor('item-1')
      if (operation === 'apply') {
        await current.controller.openCropEditor('item-1')
        await current.controller.applyCrop(recipes)
      }
      if (operation === 'revert') await current.controller.revertCrop('crop-1')
      expect(current.onError).toHaveBeenLastCalledWith(copy)
    }
  })

  it('forwards recoverable remove, preview, and revert command errors', async () => {
    const remove = harness()
    remove.operations.remove.mockResolvedValue(
      failure('capture_busy', 'remove-failed', true, 'diag-remove'),
    )
    await remove.controller.removeItem('item-1')
    expect(remove.onError).toHaveBeenLastCalledWith('remove-failed')

    const preview = harness()
    preview.operations.preview.mockResolvedValue(
      failure('capture_busy', 'preview-failed', true, 'diag-preview'),
    )
    await preview.controller.openCropEditor('item-1')
    expect(preview.onError).toHaveBeenLastCalledWith('preview-failed')

    const revert = harness()
    revert.operations.revert.mockResolvedValue(
      failure('capture_busy', 'revert-failed', true, 'diag-revert'),
    )
    await revert.controller.revertCrop('crop-1')
    expect(revert.onError).toHaveBeenLastCalledWith('revert-failed')
  })

  it('ignores late preview completion after leaving the batch', async () => {
    const gate = deferred<AppResult<CaptureItemPreview>>()
    const current = harness()
    current.operations.preview.mockReturnValue(gate.promise)
    const opening = current.controller.openCropEditor('item-1')
    await vi.waitFor(() => expect(current.operations.preview).toHaveBeenCalledOnce())
    current.setCurrent(undefined)
    gate.resolve(success({ itemId: 'item-1', mediaType: 'image/png', dataUrl: 'late' }))
    await opening
    expect(current.controller.cropEditor.value).toBeUndefined()
    expect(current.onError).toHaveBeenCalledTimes(1)
    expect(current.onError).toHaveBeenLastCalledWith('')
  })

  it('keeps durable refreshes but ignores late mutation results and failures', async () => {
    const removeGate = deferred<AppResult<CaptureBatchDetail>>()
    const removed = harness()
    removed.operations.remove.mockReturnValue(removeGate.promise)
    const removing = removed.controller.removeItem('item-1')
    await vi.waitFor(() => expect(removed.operations.remove).toHaveBeenCalledOnce())
    removed.setCurrent(undefined)
    removeGate.resolve(success(updated))
    await removing
    expect(removed.onDetailChange).not.toHaveBeenCalled()
    expect(removed.loadBatches).toHaveBeenCalledOnce()

    const successGate = deferred<AppResult<CaptureCropApplyReport>>()
    const stale = harness()
    await stale.controller.openCropEditor('item-1')
    stale.operations.apply.mockReturnValue(successGate.promise)
    const applying = stale.controller.applyCrop(recipes)
    await vi.waitFor(() => expect(stale.operations.apply).toHaveBeenCalledOnce())
    stale.setCurrent({ ...detail, batch: { ...detail.batch, revision: 8 } })
    successGate.resolve(success({
      detail: updated, operationId: 'operation-1', sourceItemId: 'item-1',
      derivedItemIds: ['derived-2'], derivationIds: ['crop-2'],
    }))
    await applying
    expect(stale.onDetailChange).not.toHaveBeenCalled()
    expect(stale.onSaveStateChange).not.toHaveBeenCalled()
    expect(stale.loadBatches).toHaveBeenCalledOnce()

    const failureGate = deferred<AppResult<CaptureBatchDetail>>()
    const failed = harness()
    failed.operations.revert.mockReturnValue(failureGate.promise)
    const reverting = failed.controller.revertCrop('crop-1')
    await vi.waitFor(() => expect(failed.operations.revert).toHaveBeenCalledOnce())
    failed.setCurrent(undefined)
    failureGate.reject(new Error('late offline'))
    await reverting
    expect(failed.onError).toHaveBeenCalledTimes(1)
    expect(failed.onError).toHaveBeenLastCalledWith('')
    expect(failed.loadDetail).not.toHaveBeenCalled()
  })

  it('ignores blocked and batchless actions and closes the editor explicitly', async () => {
    const blocked = harness()
    blocked.setBlocked(true)
    await blocked.controller.removeItem('item-1')
    await blocked.controller.openCropEditor('item-1')
    await blocked.controller.revertCrop('crop-1')
    expect(blocked.confirm).not.toHaveBeenCalled()
    expect(blocked.operations.preview).not.toHaveBeenCalled()

    const batchless = harness()
    batchless.setCurrent(undefined)
    await batchless.controller.removeItem('item-1')
    await batchless.controller.applyCrop(recipes)
    expect(batchless.operations.remove).not.toHaveBeenCalled()
    expect(batchless.operations.apply).not.toHaveBeenCalled()

    const editor = harness()
    await editor.controller.openCropEditor('item-1')
    editor.controller.closeCropEditor()
    expect(editor.controller.cropEditor.value).toBeUndefined()
  })
})
