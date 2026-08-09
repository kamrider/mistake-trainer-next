import { readonly, ref } from 'vue'
import type {
  CaptureBatchDetail,
  CaptureCropApplyInput,
  CaptureCropApplyReport,
  CaptureCropRecipe,
  CaptureCropRevertInput,
  CaptureItemPreview,
} from '../../../shared/api/bindings'
import type { AppError, AppResult } from '../../../shared/api/app-result'

export interface CaptureItemConfirmationRequest {
  eyebrow?: string
  title: string
  description: string
  confirmLabel: string
  cancelLabel?: string
  tone?: 'danger' | 'warning'
}

export interface CaptureCropEditorState {
  itemId: string
  itemName: string
  dataUrl: string
}

interface CaptureItemEditingOperations {
  remove: (batchId: string, expectedRevision: number, itemId: string) => Promise<AppResult<CaptureBatchDetail>>
  preview: (batchId: string, itemId: string) => Promise<AppResult<CaptureItemPreview>>
  apply: (input: CaptureCropApplyInput) => Promise<AppResult<CaptureCropApplyReport>>
  revert: (input: CaptureCropRevertInput) => Promise<AppResult<CaptureBatchDetail>>
}

interface CaptureItemEditingOptions {
  desktopAvailable: boolean
  activeDetail: () => CaptureBatchDetail | undefined
  isBlocked: () => boolean
  onBusyChange: (busy: boolean) => void
  onSaveStateChange: (state: 'saved') => void
  onDetailChange: (detail: CaptureBatchDetail) => void
  onError: (message: string) => void
  confirm: (request: CaptureItemConfirmationRequest) => Promise<boolean>
  invalidatePreview: (itemId: string) => void
  loadBatches: () => Promise<void>
  loadDetail: (batchId: string) => Promise<void>
  operations: CaptureItemEditingOperations
}

interface BatchIdentity {
  id: string
  revision: number
}

export function useCaptureItemEditing(options: CaptureItemEditingOptions) {
  const cropEditor = ref<CaptureCropEditorState>()
  let cropEditorBatchId: string | undefined

  function identityOf(detail: CaptureBatchDetail): BatchIdentity {
    return { id: detail.batch.id, revision: detail.batch.revision }
  }

  function isCurrent(identity: BatchIdentity) {
    const active = options.activeDetail()?.batch
    return active?.id === identity.id && active.revision === identity.revision
  }

  function closeCropEditor() {
    cropEditor.value = undefined
    cropEditorBatchId = undefined
  }

  async function reportCommandError(error: AppError, identity: BatchIdentity) {
    if (!isCurrent(identity)) return
    options.onError(error.userMessage)
    if (error.code === 'capture_revision_conflict') {
      await options.loadDetail(identity.id)
    }
  }

  async function removeItem(itemId: string) {
    const requested = options.activeDetail()
    if (
      !options.desktopAvailable
      || !requested
      || options.isBlocked()
    ) return
    const confirmed = await options.confirm({
      eyebrow: '采集图片 · 删除确认',
      title: '删除这张采集图片？',
      description: '图片会从当前批次移除；如果没有其他引用，对应的加密资产也会被清理。',
      confirmLabel: '删除图片',
      cancelLabel: '保留图片',
      tone: 'danger',
    })
    if (!confirmed) return
    const current = options.activeDetail()
    if (
      !current
      || current.batch.id !== requested.batch.id
      || options.isBlocked()
      || !current.items.some(item => item.id === itemId)
    ) return
    const identity = identityOf(current)
    options.onBusyChange(true)
    try {
      const result = await options.operations.remove(identity.id, identity.revision, itemId)
      if (result.ok) {
        options.invalidatePreview(itemId)
        if (isCurrent(identity)) options.onDetailChange(result.data)
        else await options.loadBatches()
      }
      else await reportCommandError(result.error, identity)
    }
    catch {
      if (isCurrent(identity)) options.onError('图片没有删除成功。')
    }
    finally {
      options.onBusyChange(false)
    }
  }

  async function openCropEditor(itemId: string) {
    const current = options.activeDetail()
    const item = current?.items.find(value => value.id === itemId)
    if (
      !options.desktopAvailable
      || !current
      || current.batch.state !== 'organizing'
      || options.isBlocked()
      || !item
      || item.cropDerivationId
    ) return
    const identity = identityOf(current)
    options.onBusyChange(true)
    options.onError('')
    try {
      const result = await options.operations.preview(identity.id, itemId)
      if (!isCurrent(identity)) return
      if (result.ok) {
        cropEditor.value = { itemId, itemName: item.sourceName, dataUrl: result.data.dataUrl }
        cropEditorBatchId = identity.id
      }
      else await reportCommandError(result.error, identity)
    }
    catch {
      if (isCurrent(identity)) {
        options.onError('没有读取到裁剪大图，原图仍然安全保留，请重试。')
      }
    }
    finally {
      options.onBusyChange(false)
    }
  }

  async function applyCrop(recipes: CaptureCropRecipe[]) {
    const current = options.activeDetail()
    const editor = cropEditor.value
    if (
      !options.desktopAvailable
      || !current
      || !editor
      || cropEditorBatchId !== current.batch.id
      || options.isBlocked()
      || !current.items.some(item => item.id === editor.itemId && !item.cropDerivationId)
    ) return
    const identity = identityOf(current)
    options.onBusyChange(true)
    options.onError('')
    try {
      const result = await options.operations.apply({
        batchId: identity.id,
        expectedRevision: identity.revision,
        itemId: editor.itemId,
        recipes,
      })
      if (result.ok) {
        options.invalidatePreview(editor.itemId)
        if (isCurrent(identity)) {
          options.onDetailChange(result.data.detail)
          closeCropEditor()
          options.onSaveStateChange('saved')
        }
        await options.loadBatches()
        return result.data
      }
      else await reportCommandError(result.error, identity)
    }
    catch {
      if (isCurrent(identity)) {
        options.onError('裁剪没有保存，原图和当前分组均未改变，请重试。')
      }
    }
    finally {
      options.onBusyChange(false)
    }
  }

  async function revertCrop(derivationId: string) {
    const requested = options.activeDetail()
    if (
      !options.desktopAvailable
      || !requested
      || options.isBlocked()
    ) return
    const confirmed = await options.confirm({
      eyebrow: '裁剪结果 · 恢复确认',
      title: '恢复裁剪前的原图？',
      description: '这次裁出的所有区域会从采集工作台移除，原图会回到原来的位置。',
      confirmLabel: '恢复原图',
      cancelLabel: '保留裁剪结果',
      tone: 'warning',
    })
    if (!confirmed) return
    const current = options.activeDetail()
    if (
      !current
      || current.batch.id !== requested.batch.id
      || options.isBlocked()
      || !current.items.some(item => item.cropDerivationId === derivationId)
    ) return
    const identity = identityOf(current)
    const derivedItemIds = current.items
      .filter(item => item.cropDerivationId)
      .map(item => item.id)
    options.onBusyChange(true)
    options.onError('')
    try {
      const result = await options.operations.revert({
        batchId: identity.id,
        expectedRevision: identity.revision,
        derivationId,
      })
      if (result.ok) {
        for (const itemId of derivedItemIds) options.invalidatePreview(itemId)
        if (isCurrent(identity)) {
          options.onDetailChange(result.data)
          options.onSaveStateChange('saved')
        }
        await options.loadBatches()
      }
      else await reportCommandError(result.error, identity)
    }
    catch {
      if (isCurrent(identity)) options.onError('没有恢复成功，现有图片保持不变。')
    }
    finally {
      options.onBusyChange(false)
    }
  }

  return {
    cropEditor: readonly(cropEditor),
    closeCropEditor,
    removeItem,
    openCropEditor,
    applyCrop,
    revertCrop,
  }
}
