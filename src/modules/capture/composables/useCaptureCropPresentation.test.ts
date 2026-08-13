import { afterEach, describe, expect, it, vi } from 'vitest'
import { ref } from 'vue'
import type { CaptureCropRecipe } from '../../../shared/api/bindings'
import type { CaptureCropEditorState } from './useCaptureItemEditing'
import { useCaptureCropPresentation } from './useCaptureCropPresentation'

const editor: CaptureCropEditorState = {
  itemId: 'item-1',
  itemName: '题目一',
  dataUrl: 'data:image/png;base64,AA==',
}
const recipes = [{
  rect: { x: 0.1, y: 0.2, width: 0.7, height: 0.6 },
  rotationDegrees: 90,
  outputMediaType: 'image/png',
  maxEdge: 4096,
  jpegQuality: 90,
}] satisfies CaptureCropRecipe[]

function button(attribute: string, value: string, label: string) {
  const element = document.createElement('button')
  element.dataset[attribute] = value
  element.textContent = label
  document.body.append(element)
  return element
}

function harness() {
  const batchId = ref<string | undefined>('batch-1')
  const cropEditor = ref<CaptureCropEditorState>()
  const recognitionOpen = ref(false)
  const openCrop = vi.fn(async () => { cropEditor.value = editor })
  const closeCrop = vi.fn(() => { cropEditor.value = undefined })
  const applyCrop = vi.fn(async (): Promise<{ derivedItemIds: string[] } | undefined> => {
    cropEditor.value = undefined
    return { derivedItemIds: ['derived-1'] }
  })
  const editRecognition = vi.fn(async () => { recognitionOpen.value = true })
  const closeRecognition = vi.fn(() => { recognitionOpen.value = false })
  const saveRecognition = vi.fn(async () => { recognitionOpen.value = false })
  const controller = useCaptureCropPresentation({
    activeBatchId: () => batchId.value,
    cropEditor,
    recognitionEditorOpen: () => recognitionOpen.value,
    cropSeed: () => ({ suggestedRotationDegrees: 90 }),
    openCrop,
    closeCrop,
    applyCrop,
    editRecognition,
    closeRecognition,
    saveRecognition,
  })
  return {
    controller,
    batchId,
    cropEditor,
    recognitionOpen,
    openCrop,
    closeCrop,
    applyCrop,
    editRecognition,
    closeRecognition,
    saveRecognition,
  }
}

afterEach(() => document.body.replaceChildren())

describe('useCaptureCropPresentation', () => {
  it('captures the launcher and restores it when ordinary crop open fails or closes', async () => {
    const h = harness()
    const launcher = button('cropItemId', 'item-1', 'crop')
    launcher.focus()
    h.openCrop.mockImplementationOnce(async () => undefined)

    await h.controller.openVisibleCropEditor('item-1')
    expect(launcher).toHaveFocus()
    expect(h.openCrop).toHaveBeenCalledWith('item-1', { suggestedRotationDegrees: 90 })

    launcher.focus()
    await h.controller.openVisibleCropEditor('item-1')
    await h.controller.closeVisibleCropEditor()
    expect(h.closeCrop).toHaveBeenCalledOnce()
    expect(launcher).toHaveFocus()
  })

  it('focuses the first derived item after ordinary crop apply', async () => {
    const h = harness()
    const launcher = button('cropItemId', 'item-1', 'crop')
    const successor = button('cropResultItemId', 'derived-1', 'derived')
    launcher.focus()
    await h.controller.openVisibleCropEditor('item-1')

    await h.controller.applyVisibleCrop(recipes)
    expect(h.applyCrop).toHaveBeenCalledWith(recipes)
    expect(successor).toHaveFocus()
  })

  it('owns development editor close and apply without invoking native crop operations', async () => {
    const h = harness()
    const launcher = button('cropItemId', 'item-1', 'crop')
    launcher.focus()
    h.controller.setDevelopmentCropEditor(editor, 'item-1')
    expect(h.controller.visibleCropEditor.value).toEqual(editor)

    await h.controller.applyVisibleCrop(recipes)
    expect(h.applyCrop).not.toHaveBeenCalled()
    expect(h.controller.visibleCropEditor.value).toBeUndefined()
    expect(launcher).toHaveFocus()
  })

  it('coordinates recognition edit close and save focus restoration', async () => {
    const h = harness()
    const trigger = button('recognitionEditSuggestionId', 'suggestion-1', 'edit')
    trigger.focus()
    await h.controller.openRecognitionCropEditor('suggestion-1')
    await h.controller.saveRecognitionCropEditor(recipes)
    expect(h.saveRecognition).toHaveBeenCalledWith(recipes)
    expect(trigger).toHaveFocus()

    trigger.focus()
    await h.controller.openRecognitionCropEditor('suggestion-1')
    await h.controller.closeRecognitionCropEditor()
    expect(h.closeRecognition).toHaveBeenCalledOnce()
    expect(trigger).toHaveFocus()
  })

  it('does not restore stale focus after the active batch changes', async () => {
    const h = harness()
    const launcher = button('cropItemId', 'item-1', 'crop')
    launcher.focus()
    await h.controller.openVisibleCropEditor('item-1')
    h.batchId.value = 'batch-2'
    const other = button('cropItemId', 'other', 'other')
    other.focus()

    await h.controller.closeVisibleCropEditor()
    expect(launcher).not.toHaveFocus()
  })

  it('clears pending modal focus when the composition owner is disposed', async () => {
    const h = harness()
    const launcher = button('cropItemId', 'item-1', 'crop')
    launcher.focus()
    await h.controller.openVisibleCropEditor('item-1')
    const modalControl = button('modalControl', 'save', 'save')
    modalControl.focus()

    h.controller.clearPendingFocus()
    await h.controller.closeVisibleCropEditor()
    expect(launcher).not.toHaveFocus()
  })
})
