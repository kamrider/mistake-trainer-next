import { computed, readonly, ref, type DeepReadonly, type Ref } from 'vue'
import type { CaptureCropRecipe } from '../../../shared/api/bindings'
import { createModalReturnFocusController } from '../../../shared/ui/modal-return-focus'
import type { CaptureCropEditorSeed, CaptureCropEditorState } from './useCaptureItemEditing'

export interface CaptureCropPresentationOptions {
  activeBatchId: () => string | undefined
  cropEditor: Readonly<Ref<DeepReadonly<CaptureCropEditorState> | undefined>>
  recognitionEditorOpen: () => boolean
  cropSeed: (itemId: string) => CaptureCropEditorSeed | undefined
  openCrop: (itemId: string, seed?: CaptureCropEditorSeed) => Promise<unknown>
  closeCrop: () => void
  applyCrop: (recipes: CaptureCropRecipe[]) => Promise<{ derivedItemIds: string[] } | undefined>
  editRecognition: (suggestionId: string) => Promise<unknown>
  closeRecognition: () => void
  saveRecognition: (recipes: CaptureCropRecipe[]) => Promise<unknown>
}

function enabledButton(dataAttribute: string, targetId: string) {
  return Array.from(document.querySelectorAll<HTMLButtonElement>(`button[data-${dataAttribute}]`))
    .find(button => button.getAttribute(`data-${dataAttribute}`) === targetId && !button.disabled)
}

export function useCaptureCropPresentation(options: CaptureCropPresentationOptions) {
  const developmentCropEditor = ref<CaptureCropEditorState>()
  const visibleCropEditor = computed(() => developmentCropEditor.value ?? options.cropEditor.value)
  const cropFocus = createModalReturnFocusController({
    currentContextId: options.activeBatchId,
    isModalOpen: () => Boolean(visibleCropEditor.value),
    findFallback: itemId => enabledButton('crop-item-id', itemId),
  })
  const recognitionFocus = createModalReturnFocusController({
    currentContextId: options.activeBatchId,
    isModalOpen: options.recognitionEditorOpen,
    findFallback: suggestionId => enabledButton('recognition-edit-suggestion-id', suggestionId),
  })

  function captureFocus(
    controller: typeof cropFocus,
    targetId: string,
    dataAttribute: string,
  ) {
    const contextId = options.activeBatchId()
    if (!contextId) {
      controller.clear()
      return
    }
    const active = document.activeElement
    controller.capture({
      contextId,
      targetId,
      element: active instanceof HTMLButtonElement
        && active.getAttribute(`data-${dataAttribute}`) === targetId
        ? active
        : undefined,
    })
  }

  function setDevelopmentCropEditor(state: CaptureCropEditorState | undefined, targetId = state?.itemId) {
    if (state && targetId) captureFocus(cropFocus, targetId, 'crop-item-id')
    developmentCropEditor.value = state
  }

  async function openVisibleCropEditor(itemId: string) {
    captureFocus(cropFocus, itemId, 'crop-item-id')
    await options.openCrop(itemId, options.cropSeed(itemId))
    if (!options.cropEditor.value) await cropFocus.restore()
  }

  async function closeVisibleCropEditor() {
    developmentCropEditor.value = undefined
    options.closeCrop()
    await cropFocus.restore()
  }

  async function applyVisibleCrop(recipes: CaptureCropRecipe[]) {
    if (developmentCropEditor.value) {
      developmentCropEditor.value = undefined
      await cropFocus.restore()
      return
    }
    const report = await options.applyCrop(recipes)
    if (!options.cropEditor.value) {
      await cropFocus.restore(
        report?.derivedItemIds[0]
          ? () => enabledButton('crop-result-item-id', report.derivedItemIds[0]!)
          : undefined,
      )
    }
  }

  async function openRecognitionCropEditor(suggestionId: string) {
    captureFocus(recognitionFocus, suggestionId, 'recognition-edit-suggestion-id')
    await options.editRecognition(suggestionId)
    if (!options.recognitionEditorOpen()) await recognitionFocus.restore()
  }

  async function closeRecognitionCropEditor() {
    options.closeRecognition()
    await recognitionFocus.restore()
  }

  async function saveRecognitionCropEditor(recipes: CaptureCropRecipe[]) {
    await options.saveRecognition(recipes)
    if (!options.recognitionEditorOpen()) await recognitionFocus.restore()
  }

  function clearPendingFocus() {
    cropFocus.clear()
    recognitionFocus.clear()
  }

  return {
    developmentCropEditor: readonly(developmentCropEditor),
    visibleCropEditor,
    setDevelopmentCropEditor,
    openVisibleCropEditor,
    closeVisibleCropEditor,
    applyVisibleCrop,
    openRecognitionCropEditor,
    closeRecognitionCropEditor,
    saveRecognitionCropEditor,
    clearPendingFocus,
  }
}
