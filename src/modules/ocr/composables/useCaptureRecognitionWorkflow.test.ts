import { describe, expect, it, vi } from 'vitest'
import type {
  CaptureBatchDetail,
  CaptureRecognitionJob,
} from '../../../shared/api/bindings'
import { failure, success, type AppResult } from '../../../shared/api/app-result'
import { useCaptureRecognitionWorkflow } from './useCaptureRecognitionWorkflow'

const detail = {
  batch: {
    id: 'batch-1', subject: '数学', state: 'organizing', itemCount: 2,
    draftCount: 0, readyCount: 0, updatedAtUtcMs: 1, revision: 7,
  },
  items: [], drafts: [], unassignedItemIds: ['item-1', 'item-2'], pairSuggestions: [],
} as CaptureBatchDetail

function job(states: Array<'proposed' | 'accepted' | 'rejected'> = ['proposed', 'proposed']) {
  return {
    id: 'job-1', batchId: 'batch-1', state: 'review', totalItems: 2, processedItems: 2,
    suggestions: states.map((state, index) => ({
      id: `suggestion-${index + 1}`,
      itemId: `item-${index + 1}`,
      regions: [],
      confidenceBasisPoints: 9000,
      reviewBand: 'high' as const,
      state,
      reasonCodes: [],
    })),
    createdAtUtcMs: 1,
    updatedAtUtcMs: 2,
  } satisfies CaptureRecognitionJob
}

function deferred<T>() {
  let resolve!: (value: T) => void
  let reject!: (reason?: unknown) => void
  const promise = new Promise<T>((finish, fail) => { resolve = finish; reject = fail })
  return { promise, resolve, reject }
}

function harness() {
  let current: CaptureBatchDetail | undefined = detail
  let requestedBatchId = 'batch-1'
  const operations = {
    capability: vi.fn().mockResolvedValue(success({
      supported: true,
      detail: 'ready',
      components: [],
      recognitionFeature: {
        state: 'ready',
        requiredComponentId: 'opencv_preprocess',
        detail: 'ready',
      },
      automaticRecognitionEnabled: false,
    })),
    status: vi.fn().mockResolvedValue(success(job())),
    lastOperation: vi.fn().mockResolvedValue(success(null)),
    start: vi.fn().mockResolvedValue(success(job())),
    cancel: vi.fn().mockResolvedValue(success({ ...job(), state: 'cancelled' as const })),
    review: vi.fn().mockImplementation(async (input) => success(job([
      input.suggestionId === 'suggestion-1' ? input.decision : 'proposed',
      input.suggestionId === 'suggestion-2' ? input.decision : 'proposed',
    ]))),
    preview: vi.fn().mockResolvedValue(success({
      itemId: 'item-1',
      mediaType: 'image/png',
      dataUrl: 'data:image/png;base64,preview',
    })),
    apply: vi.fn().mockResolvedValue(success({
      operationId: 'operation-1',
      appliedSuggestionCount: 2,
      createdDraftCount: 0,
      createdItemCount: 3,
      pairSuggestionCount: 1,
      unmatchedAnswerCount: 0,
      staleSuggestionCount: 0,
      detail: { ...detail, batch: { ...detail.batch, revision: 8 } },
    })),
    revert: vi.fn().mockResolvedValue(success({
      operationId: 'operation-1',
      revertedItemCount: 2,
      detail: { ...detail, batch: { ...detail.batch, revision: 9 } },
    })),
  }
  const onDetailChange = vi.fn((value: CaptureBatchDetail) => { current = value })
  const onError = vi.fn()
  const controller = useCaptureRecognitionWorkflow({
    desktopAvailable: true,
    requestedBatchId: () => requestedBatchId,
    activeDetail: () => current,
    onDetailChange,
    onError,
    operations,
  })
  return {
    controller,
    operations,
    onDetailChange,
    onError,
    setCurrent: (value?: CaptureBatchDetail) => { current = value },
    setRequestedBatchId: (value: string) => { requestedBatchId = value },
  }
}

describe('useCaptureRecognitionWorkflow', () => {
  it('builds exact start, apply, and revert inputs while preserving persistent undo state', async () => {
    const h = harness()
    await h.controller.start()
    expect(h.operations.start).toHaveBeenCalledWith({
      batchId: 'batch-1',
      itemIds: ['item-1', 'item-2'],
    })

    await h.controller.apply(['suggestion-1', 'suggestion-2'])
    expect(h.operations.apply).toHaveBeenCalledWith({
      batchId: 'batch-1',
      jobId: 'job-1',
      expectedRevision: 7,
      acceptedSuggestionIds: ['suggestion-1', 'suggestion-2'],
    })
    expect(h.controller.operation.value).toMatchObject({
      operationId: 'operation-1', afterRevision: 8, createdItemCount: 3, reverted: false,
    })
    expect(h.controller.notice.value).toContain('已切分 3 张题答图片')

    await h.controller.revert('operation-1')
    expect(h.operations.revert).toHaveBeenCalledWith({
      batchId: 'batch-1',
      operationId: 'operation-1',
      expectedRevision: 8,
    })
    expect(h.controller.operation.value?.reverted).toBe(true)
    expect(h.controller.notice.value).toBe('已撤销智能整理，恢复 2 张来源图的原始状态。')
  })

  it('serializes distinct rapid review decisions instead of dropping the second action', async () => {
    const first = deferred<AppResult<CaptureRecognitionJob>>()
    const h = harness()
    h.controller.job.value = job()
    h.operations.review
      .mockReturnValueOnce(first.promise)
      .mockResolvedValueOnce(success(job(['accepted', 'rejected'])))

    const accepting = h.controller.review({
      jobId: 'job-1', suggestionId: 'suggestion-1', decision: 'accepted', editedRegions: null,
    })
    const rejecting = h.controller.review({
      jobId: 'job-1', suggestionId: 'suggestion-2', decision: 'rejected', editedRegions: null,
    })

    expect(h.controller.job.value?.suggestions.map(item => item.state)).toEqual(['accepted', 'rejected'])
    expect(h.operations.review).toHaveBeenCalledTimes(1)
    first.resolve(success(job(['accepted', 'proposed'])))
    await expect(Promise.all([accepting, rejecting])).resolves.toEqual([true, true])
    expect(h.operations.review.mock.calls.map(([input]) => input.suggestionId)).toEqual([
      'suggestion-1', 'suggestion-2',
    ])
    expect(h.controller.job.value?.suggestions.map(item => item.state)).toEqual(['accepted', 'rejected'])
  })

  it('distinguishes background review saving from exclusive recognition work', async () => {
    const reviewGate = deferred<AppResult<CaptureRecognitionJob>>()
    const reviewing = harness()
    reviewing.controller.job.value = job()
    reviewing.operations.review.mockReturnValueOnce(reviewGate.promise)

    const saving = reviewing.controller.review({
      jobId: 'job-1', suggestionId: 'suggestion-1', decision: 'accepted', editedRegions: null,
    })
    expect(reviewing.controller.busy.value).toBe(true)
    expect(reviewing.controller.operationBusy.value).toBe(false)
    reviewGate.resolve(success(job(['accepted', 'proposed'])))
    await expect(saving).resolves.toBe(true)
    expect(reviewing.controller.busy.value).toBe(false)
    expect(reviewing.controller.operationBusy.value).toBe(false)

    const startGate = deferred<AppResult<CaptureRecognitionJob>>()
    const starting = harness()
    starting.operations.start.mockReturnValueOnce(startGate.promise)
    const start = starting.controller.start()
    expect(starting.controller.busy.value).toBe(true)
    expect(starting.controller.operationBusy.value).toBe(true)
    startGate.resolve(success(job()))
    await start
    expect(starting.controller.busy.value).toBe(false)
    expect(starting.controller.operationBusy.value).toBe(false)
  })

  it('rolls back only the failed optimistic review while retaining a later queued choice', async () => {
    const first = deferred<AppResult<CaptureRecognitionJob>>()
    const h = harness()
    h.controller.job.value = job()
    h.operations.review
      .mockReturnValueOnce(first.promise)
      .mockResolvedValueOnce(success(job(['proposed', 'rejected'])))

    const accepting = h.controller.review({
      jobId: 'job-1', suggestionId: 'suggestion-1', decision: 'accepted', editedRegions: null,
    })
    const rejecting = h.controller.review({
      jobId: 'job-1', suggestionId: 'suggestion-2', decision: 'rejected', editedRegions: null,
    })
    first.resolve(failure('capture_recognition_failed', '第一条没有保存', true, 'diag-review'))

    await expect(accepting).resolves.toBe(false)
    expect(h.controller.job.value?.suggestions.map(item => item.state)).toEqual(['proposed', 'rejected'])
    await expect(rejecting).resolves.toBe(true)
    expect(h.onError).toHaveBeenCalledWith('第一条没有保存')
  })

  it('coalesces a later pending decision for the same suggestion to its newest value', async () => {
    const first = deferred<AppResult<CaptureRecognitionJob>>()
    const h = harness()
    h.controller.job.value = job()
    h.operations.review
      .mockReturnValueOnce(first.promise)
      .mockResolvedValueOnce(success(job(['accepted', 'rejected'])))

    const firstDecision = h.controller.review({
      jobId: 'job-1', suggestionId: 'suggestion-1', decision: 'accepted', editedRegions: null,
    })
    const superseded = h.controller.review({
      jobId: 'job-1', suggestionId: 'suggestion-2', decision: 'accepted', editedRegions: null,
    })
    const newest = h.controller.review({
      jobId: 'job-1', suggestionId: 'suggestion-2', decision: 'rejected', editedRegions: null,
    })
    first.resolve(success(job(['accepted', 'proposed'])))

    await expect(Promise.all([firstDecision, superseded, newest])).resolves.toEqual([true, true, true])
    expect(h.operations.review).toHaveBeenCalledTimes(2)
    expect(h.operations.review.mock.calls[1]![0]).toMatchObject({
      suggestionId: 'suggestion-2', decision: 'rejected',
    })
  })

  it('ignores late results and errors after the workflow is reset or the requested batch changes', async () => {
    const lateStart = deferred<AppResult<CaptureRecognitionJob>>()
    const h = harness()
    h.operations.start.mockReturnValueOnce(lateStart.promise)
    const starting = h.controller.start()
    h.controller.reset()
    lateStart.resolve(success(job()))
    await starting
    expect(h.controller.job.value).toBeUndefined()
    expect(h.controller.busy.value).toBe(false)

    const lateStatus = deferred<AppResult<CaptureRecognitionJob | null>>()
    h.onError.mockClear()
    h.operations.status.mockReturnValueOnce(lateStatus.promise)
    const loading = h.controller.loadStatus('batch-1')
    h.setRequestedBatchId('batch-2')
    lateStatus.resolve(success(job()))
    await loading
    expect(h.controller.job.value).toBeUndefined()
    expect(h.onError).not.toHaveBeenCalled()

    await h.controller.start()
    expect(h.operations.start).toHaveBeenCalledTimes(1)
  })
})
