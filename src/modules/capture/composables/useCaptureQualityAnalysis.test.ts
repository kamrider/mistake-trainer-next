import { describe, expect, it, vi } from 'vitest'
import { nextTick, ref } from 'vue'
import type { CaptureQualityReport } from '../../../shared/api/bindings'
import { failure, success, type AppResult } from '../../../shared/api/app-result'
import { useCaptureQualityAnalysis } from './useCaptureQualityAnalysis'

function deferred<T>() {
  let resolve!: (value: T) => void
  const promise = new Promise<T>((finish) => { resolve = finish })
  return { promise, resolve }
}

function report(itemId = 'item-1'): CaptureQualityReport {
  return {
    itemId,
    issues: ['skewed'],
    sharpnessScore: 0.8,
    darkFraction: 0.1,
    brightFraction: 0.05,
    contrastScore: 0.7,
    suggestedRotationDegrees: 90,
    suggestedCrop: { x: 0.1, y: 0.2, width: 0.7, height: 0.6 },
  }
}

function harness(check = vi.fn(async (_batchId: string, itemId: string) => success(report(itemId)))) {
  const batchId = ref<string | undefined>('batch-1')
  const controller = useCaptureQualityAnalysis({
    desktopAvailable: true,
    activeBatchId: () => batchId.value,
    check,
  })
  return { batchId, check, controller }
}

describe('useCaptureQualityAnalysis', () => {
  it('checks once per cached item and projects the report into a crop seed', async () => {
    const h = harness()

    await h.controller.check('item-1')
    await h.controller.check('item-1')

    expect(h.check).toHaveBeenCalledOnce()
    expect(h.controller.reports.value['item-1']).toEqual(report())
    expect(h.controller.cropSeed('item-1')).toEqual({
      initialRecipes: [{
        rect: report().suggestedCrop,
        perspectiveQuad: null,
        rotationDegrees: 0,
        outputMediaType: 'image/png',
        maxEdge: 4096,
        jpegQuality: 90,
      }],
      suggestedRotationDegrees: 90,
    })
  })

  it('coalesces duplicate in-flight checks and exposes dismissal state', async () => {
    const gate = deferred<AppResult<CaptureQualityReport>>()
    const h = harness(vi.fn().mockReturnValue(gate.promise))

    const first = h.controller.check('item-1')
    const second = h.controller.check('item-1')
    expect(h.check).toHaveBeenCalledOnce()
    expect(h.controller.checkingItemId.value).toBe('item-1')

    h.controller.dismiss('item-1')
    h.controller.dismiss('item-1')
    expect(h.controller.dismissedItemIds.value).toEqual(['item-1'])

    gate.resolve(success(report()))
    await Promise.all([first, second])
    expect(h.controller.checkingItemId.value).toBe('')
  })

  it('surfaces command and transport failures without discarding usable images', async () => {
    const commandFailure = harness(vi.fn().mockResolvedValue(
      failure('quality_failed', '检查失败', true, 'diag-1'),
    ))
    await commandFailure.controller.check('item-1')
    expect(commandFailure.controller.errors.value['item-1']).toBe('检查失败')

    const transportFailure = harness(vi.fn().mockRejectedValue(new Error('offline')))
    await transportFailure.controller.check('item-1')
    expect(transportFailure.controller.errors.value['item-1'])
      .toBe('图片仍可继续使用，也可以稍后重新检查。')
  })

  it('drops late completion and resets all contextual state when the batch changes', async () => {
    const gate = deferred<AppResult<CaptureQualityReport>>()
    const h = harness(vi.fn().mockReturnValue(gate.promise))
    h.controller.dismiss('item-1')
    const pending = h.controller.check('item-1')

    h.batchId.value = 'batch-2'
    await nextTick()
    gate.resolve(success(report()))
    await pending

    expect(h.controller.reports.value).toEqual({})
    expect(h.controller.errors.value).toEqual({})
    expect(h.controller.checkingItemId.value).toBe('')
    expect(h.controller.dismissedItemIds.value).toEqual([])
    expect(h.controller.cropSeed('missing')).toBeUndefined()
  })

  it('does not call the desktop operation without an active desktop batch', async () => {
    const check = vi.fn()
    const batchId = ref<string | undefined>()
    const controller = useCaptureQualityAnalysis({
      desktopAvailable: false,
      activeBatchId: () => batchId.value,
      check,
    })

    await controller.check('item-1')
    expect(check).not.toHaveBeenCalled()
  })
})
