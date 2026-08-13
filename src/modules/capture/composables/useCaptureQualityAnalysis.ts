import { ref, shallowReadonly, watch } from 'vue'
import type { CaptureCropRecipe, CaptureQualityReport } from '../../../shared/api/bindings'
import type { AppResult } from '../../../shared/api/app-result'
import type { CaptureCropEditorSeed } from './useCaptureItemEditing'

export interface CaptureQualityAnalysisOptions {
  desktopAvailable: boolean
  activeBatchId: () => string | undefined
  check: (batchId: string, itemId: string) => Promise<AppResult<CaptureQualityReport>>
}

const transportFailureMessage = '图片仍可继续使用，也可以稍后重新检查。'

export function useCaptureQualityAnalysis(options: CaptureQualityAnalysisOptions) {
  const reports = ref<Record<string, CaptureQualityReport>>({})
  const errors = ref<Record<string, string>>({})
  const checkingItemId = ref('')
  const dismissedItemIds = ref<string[]>([])
  const pending = new Map<string, Promise<void>>()
  let generation = 0

  function reset() {
    generation += 1
    pending.clear()
    reports.value = {}
    errors.value = {}
    checkingItemId.value = ''
    dismissedItemIds.value = []
  }

  watch(options.activeBatchId, reset, { flush: 'sync' })

  function cropSeed(itemId: string): CaptureCropEditorSeed | undefined {
    const report = reports.value[itemId]
    if (!report) return undefined
    return {
      ...(report.suggestedCrop
        ? {
            initialRecipes: [{
              rect: report.suggestedCrop,
              perspectiveQuad: null,
              rotationDegrees: 0,
              outputMediaType: 'image/png',
              maxEdge: 4096,
              jpegQuality: 90,
            }] satisfies CaptureCropRecipe[],
          }
        : {}),
      suggestedRotationDegrees: report.suggestedRotationDegrees ?? 0,
    }
  }

  function check(itemId: string): Promise<void> {
    const batchId = options.activeBatchId()
    if (!options.desktopAvailable || !batchId || reports.value[itemId]) return Promise.resolve()
    const requestGeneration = generation
    const requestKey = `${requestGeneration}:${batchId}:${itemId}`
    const current = pending.get(requestKey)
    if (current) return current

    checkingItemId.value = itemId
    const remainingErrors = { ...errors.value }
    delete remainingErrors[itemId]
    errors.value = remainingErrors

    const request = (async () => {
      try {
        const result = await options.check(batchId, itemId)
        if (generation !== requestGeneration || options.activeBatchId() !== batchId) return
        if (result.ok) reports.value = { ...reports.value, [itemId]: result.data }
        else errors.value = { ...errors.value, [itemId]: result.error.userMessage }
      }
      catch {
        if (generation === requestGeneration && options.activeBatchId() === batchId) {
          errors.value = { ...errors.value, [itemId]: transportFailureMessage }
        }
      }
      finally {
        pending.delete(requestKey)
        if (generation === requestGeneration && checkingItemId.value === itemId) {
          checkingItemId.value = ''
        }
      }
    })()
    pending.set(requestKey, request)
    return request
  }

  function dismiss(itemId: string) {
    if (!dismissedItemIds.value.includes(itemId)) {
      dismissedItemIds.value = [...dismissedItemIds.value, itemId]
    }
  }

  return {
    reports: shallowReadonly(reports),
    errors: shallowReadonly(errors),
    checkingItemId: shallowReadonly(checkingItemId),
    dismissedItemIds: shallowReadonly(dismissedItemIds),
    cropSeed,
    check,
    dismiss,
    reset,
  }
}
