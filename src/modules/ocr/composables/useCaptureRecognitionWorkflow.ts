import { computed, ref } from 'vue'
import type {
  CaptureBatchDetail,
  CaptureCropRecipe,
  CaptureItemPreview,
  CaptureRecognitionApplyInput,
  CaptureRecognitionApplyReport,
  CaptureRecognitionJob,
  CaptureRecognitionOperationSummary,
  CaptureRecognitionRegionProposal,
  CaptureRecognitionRevertInput,
  CaptureRecognitionRevertReport,
  CaptureRecognitionReviewInput,
  CaptureRecognitionStartInput,
  OcrCapabilityStatus,
  OcrRecognitionFeatureStatus,
} from '../../../shared/api/bindings'
import type { AppResult } from '../../../shared/api/app-result'

export interface CaptureRecognitionCropEditorState {
  suggestionId: string
  itemName: string
  dataUrl: string
  regions: CaptureRecognitionRegionProposal[]
}

interface CaptureRecognitionOperations {
  capability: () => Promise<AppResult<OcrCapabilityStatus>>
  status: (batchId: string) => Promise<AppResult<CaptureRecognitionJob | null>>
  lastOperation: (batchId: string) => Promise<AppResult<CaptureRecognitionOperationSummary | null>>
  start: (input: CaptureRecognitionStartInput) => Promise<AppResult<CaptureRecognitionJob>>
  cancel: (jobId: string) => Promise<AppResult<CaptureRecognitionJob>>
  review: (input: CaptureRecognitionReviewInput) => Promise<AppResult<CaptureRecognitionJob>>
  preview: (batchId: string, itemId: string) => Promise<AppResult<CaptureItemPreview>>
  apply: (input: CaptureRecognitionApplyInput) => Promise<AppResult<CaptureRecognitionApplyReport>>
  revert: (input: CaptureRecognitionRevertInput) => Promise<AppResult<CaptureRecognitionRevertReport>>
}

interface CaptureRecognitionWorkflowOptions {
  desktopAvailable: boolean
  requestedBatchId: () => string
  activeDetail: () => CaptureBatchDetail | undefined
  onDetailChange: (detail: CaptureBatchDetail) => void
  onError: (message: string) => void
  operations: CaptureRecognitionOperations
}

interface ReviewWaiter {
  resolve: (saved: boolean) => void
}

interface QueuedReview {
  input: CaptureRecognitionReviewInput
  started: boolean
  waiters: ReviewWaiter[]
}

const defaultFeature: OcrRecognitionFeatureStatus = {
  state: 'ready',
  requiredComponentId: 'opencv_preprocess',
  detail: '基础版面预切可直接使用；确认后结果只进入素材牌库。',
}

function projectReview(
  source: CaptureRecognitionJob,
  input: CaptureRecognitionReviewInput,
): CaptureRecognitionJob {
  return {
    ...source,
    suggestions: source.suggestions.map(suggestion =>
      suggestion.id === input.suggestionId
        ? {
            ...suggestion,
            state: input.decision,
            regions: input.editedRegions ?? suggestion.regions,
          }
        : suggestion),
  }
}

export function useCaptureRecognitionWorkflow(options: CaptureRecognitionWorkflowOptions) {
  const capability = ref<OcrCapabilityStatus>()
  const job = ref<CaptureRecognitionJob>()
  const operation = ref<CaptureRecognitionOperationSummary>()
  const notice = ref('')
  const busy = ref(false)
  const cropEditor = ref<CaptureRecognitionCropEditorState>()
  const feature = computed<OcrRecognitionFeatureStatus>(
    () => capability.value?.recognitionFeature ?? defaultFeature,
  )

  let lifecycle = 0
  let disposed = false
  const operationBusy = ref(false)
  let authoritativeJob: CaptureRecognitionJob | undefined
  let reviewDrain: Promise<void> | undefined
  const reviewQueue: QueuedReview[] = []

  function requested(batchId: string, token = lifecycle) {
    return !disposed && token === lifecycle && options.requestedBatchId() === batchId
  }

  function active(batchId: string, revision?: number, token = lifecycle) {
    const current = options.activeDetail()?.batch
    return requested(batchId, token)
      && current?.id === batchId
      && (revision === undefined || current.revision === revision)
  }

  function setAuthoritativeJob(value: CaptureRecognitionJob | undefined) {
    authoritativeJob = value
    job.value = value
    applyQueuedProjection()
  }

  function applyQueuedProjection() {
    if (!authoritativeJob) {
      if (!reviewQueue.length) job.value = undefined
      return
    }
    job.value = reviewQueue.reduce(
      (projected, entry) => projectReview(projected, entry.input),
      authoritativeJob,
    )
  }

  async function loadCapability() {
    if (!options.desktopAvailable || disposed) return
    const token = lifecycle
    try {
      const result = await options.operations.capability()
      if (result.ok && !disposed && token === lifecycle) capability.value = result.data
    }
    catch {
      // Recognition is optional; keep the conservative fallback feature available.
    }
  }

  async function loadStatus(batchId: string) {
    if (!options.desktopAvailable || disposed) return
    const token = lifecycle
    try {
      const result = await options.operations.status(batchId)
      if (result.ok && requested(batchId, token)) {
        setAuthoritativeJob(result.data ?? undefined)
      }
    }
    catch {
      // Recognition status must not block manual organization.
    }
  }

  async function loadLastOperation(batchId: string) {
    if (!options.desktopAvailable || disposed) return
    const token = lifecycle
    try {
      const result = await options.operations.lastOperation(batchId)
      if (result.ok && requested(batchId, token)) operation.value = result.data ?? undefined
    }
    catch {
      // Undo availability is supplementary and must not block the workbench.
    }
  }

  async function start() {
    const current = options.activeDetail()
    if (
      !options.desktopAvailable
      || !current
      || !active(current.batch.id)
      || busy.value
      || disposed
    ) return
    const token = lifecycle
    const batchId = current.batch.id
    operationBusy.value = true
    busy.value = true
    options.onError('')
    try {
      const result = await options.operations.start({
        batchId,
        itemIds: [...current.unassignedItemIds],
      })
      if (!active(batchId, undefined, token)) return
      if (result.ok) setAuthoritativeJob(result.data)
      else options.onError(result.error.userMessage)
    }
    catch {
      if (active(batchId, undefined, token)) {
        options.onError('智能切图没有启动；原图和当前分组保持不变。')
      }
    }
    finally {
      if (token === lifecycle) {
        operationBusy.value = false
        busy.value = false
      }
    }
  }

  async function cancel(jobId: string) {
    const currentJob = job.value
    const batchId = currentJob?.batchId
    if (
      !batchId
      || currentJob.id !== jobId
      || !active(batchId)
      || busy.value
      || disposed
    ) return
    const token = lifecycle
    operationBusy.value = true
    busy.value = true
    try {
      const result = await options.operations.cancel(jobId)
      if (!active(batchId, undefined, token)) return
      if (result.ok) setAuthoritativeJob(result.data)
      else options.onError(result.error.userMessage)
    }
    catch {
      if (active(batchId, undefined, token)) {
        options.onError('停止请求没有完成；你仍可继续手工整理。')
      }
    }
    finally {
      if (token === lifecycle) {
        operationBusy.value = false
        busy.value = false
      }
    }
  }

  function finishReview(entry: QueuedReview, saved: boolean) {
    for (const waiter of entry.waiters) waiter.resolve(saved)
  }

  async function drainReviews() {
    const token = lifecycle
    try {
      while (reviewQueue.length) {
        const entry = reviewQueue[0]!
        entry.started = true
        const batchId = job.value?.batchId
        if (!batchId || !active(batchId, undefined, token)) {
          reviewQueue.shift()
          finishReview(entry, false)
          continue
        }
        let saved = false
        try {
          const result = await options.operations.review(entry.input)
          if (reviewQueue[0] !== entry || !active(batchId, undefined, token)) return
          if (result.ok) {
            authoritativeJob = result.data
            saved = true
          }
          else {
            options.onError(result.error.userMessage)
          }
        }
        catch {
          if (reviewQueue[0] !== entry || !active(batchId, undefined, token)) return
          options.onError('这条识别建议没有保存；你刚才的手工整理不会受到影响。')
        }
        reviewQueue.shift()
        applyQueuedProjection()
        finishReview(entry, saved)
      }
    }
    finally {
      if (token === lifecycle) {
        reviewDrain = undefined
        busy.value = operationBusy.value
      }
    }
  }

  function review(input: CaptureRecognitionReviewInput): Promise<boolean> {
    const currentJob = job.value
    if (
      !options.desktopAvailable
      || disposed
      || operationBusy.value
      || !currentJob
      || currentJob.id !== input.jobId
      || !active(currentJob.batchId)
    ) return Promise.resolve(false)

    if (!authoritativeJob || authoritativeJob.id !== currentJob.id) {
      authoritativeJob = currentJob
    }
    const pending = reviewQueue.find(entry =>
      !entry.started && entry.input.suggestionId === input.suggestionId)
    const result = new Promise<boolean>((resolve) => {
      if (pending) {
        pending.input = input
        pending.waiters.push({ resolve })
      }
      else {
        reviewQueue.push({ input, started: false, waiters: [{ resolve }] })
      }
    })
    busy.value = true
    applyQueuedProjection()
    if (!reviewDrain) {
      reviewDrain = drainReviews()
    }
    return result
  }

  async function reviewMany(inputs: CaptureRecognitionReviewInput[]) {
    for (const input of inputs) {
      if (!await review(input)) break
    }
  }

  async function edit(suggestionId: string) {
    const current = options.activeDetail()
    const currentJob = job.value
    const suggestion = currentJob?.suggestions.find(item => item.id === suggestionId)
    if (
      !current
      || currentJob?.batchId !== current.batch.id
      || !active(current.batch.id)
      || !suggestion
      || busy.value
      || disposed
    ) return
    const token = lifecycle
    const batchId = current.batch.id
    operationBusy.value = true
    busy.value = true
    try {
      const result = await options.operations.preview(batchId, suggestion.itemId)
      if (!active(batchId, undefined, token)) return
      if (result.ok) {
        cropEditor.value = {
          suggestionId,
          itemName: '识别建议来源图',
          dataUrl: result.data.dataUrl,
          regions: suggestion.regions.map(region => ({ ...region, rect: { ...region.rect } })),
        }
      }
      else options.onError(result.error.userMessage)
    }
    catch {
      if (active(batchId, undefined, token)) {
        options.onError('没有读取到建议来源图；当前建议和原图都没有改变。')
      }
    }
    finally {
      if (token === lifecycle) {
        operationBusy.value = false
        busy.value = false
      }
    }
  }

  async function apply(suggestionIds: string[]) {
    const current = options.activeDetail()
    const currentJob = job.value
    if (
      !options.desktopAvailable
      || !current
      || !currentJob
      || currentJob.batchId !== current.batch.id
      || !active(current.batch.id)
      || busy.value
      || !suggestionIds.length
      || disposed
    ) return
    const token = lifecycle
    const batchId = current.batch.id
    const expectedRevision = current.batch.revision
    operationBusy.value = true
    busy.value = true
    options.onError('')
    notice.value = ''
    try {
      const result = await options.operations.apply({
        batchId,
        jobId: currentJob.id,
        expectedRevision,
        acceptedSuggestionIds: suggestionIds,
      })
      if (!active(batchId, expectedRevision, token)) return
      if (!result.ok) {
        options.onError(result.error.userMessage)
        if (result.error.code === 'capture_recognition_stale') await loadStatus(batchId)
        return
      }
      options.onDetailChange(result.data.detail)
      setAuthoritativeJob(undefined)
      operation.value = {
        operationId: result.data.operationId,
        batchId,
        afterRevision: result.data.detail.batch.revision,
        createdItemCount: result.data.createdItemCount,
        reverted: false,
      }
      notice.value = result.data.pairSuggestionCount
        ? `已切分 ${result.data.createdItemCount} 张题答图片，并在素材牌库标出 ${result.data.pairSuggestionCount} 组题答建议。`
        : `已切分 ${result.data.createdItemCount} 张题答图片，已放入素材牌库。`
    }
    catch {
      if (active(batchId, expectedRevision, token)) {
        options.onError('识别结果没有应用；原图、复核选择和当前题卡都已保留。')
      }
    }
    finally {
      if (token === lifecycle) {
        operationBusy.value = false
        busy.value = false
      }
    }
  }

  async function revert(operationId: string) {
    const current = options.activeDetail()
    if (
      !options.desktopAvailable
      || !current
      || !active(current.batch.id)
      || busy.value
      || disposed
    ) return
    const token = lifecycle
    const batchId = current.batch.id
    const expectedRevision = current.batch.revision
    operationBusy.value = true
    busy.value = true
    options.onError('')
    try {
      const result = await options.operations.revert({ batchId, operationId, expectedRevision })
      if (!active(batchId, expectedRevision, token)) return
      if (!result.ok) {
        options.onError(result.error.userMessage)
        return
      }
      options.onDetailChange(result.data.detail)
      operation.value = operation.value
        ? { ...operation.value, reverted: true }
        : undefined
      notice.value = `已撤销智能整理，恢复 ${result.data.revertedItemCount} 张来源图的原始状态。`
    }
    catch {
      if (active(batchId, expectedRevision, token)) {
        options.onError('没有撤销智能整理；当前题卡和图片保持不变。')
      }
    }
    finally {
      if (token === lifecycle) {
        operationBusy.value = false
        busy.value = false
      }
    }
  }

  async function saveProposal(recipes: CaptureCropRecipe[]) {
    const editor = cropEditor.value
    const currentJob = job.value
    if (!editor || !currentJob || recipes.length !== editor.regions.length) return
    const editedRegions = recipes.map((recipe, index) => ({
      ...editor.regions[index]!,
      rect: recipe.rect,
    }))
    const saved = await review({
      jobId: currentJob.id,
      suggestionId: editor.suggestionId,
      decision: 'accepted',
      editedRegions,
    })
    if (saved && cropEditor.value?.suggestionId === editor.suggestionId) {
      cropEditor.value = undefined
    }
  }

  function reset() {
    lifecycle += 1
    operationBusy.value = false
    authoritativeJob = undefined
    for (const entry of reviewQueue.splice(0)) finishReview(entry, false)
    reviewDrain = undefined
    busy.value = false
    job.value = undefined
    operation.value = undefined
    notice.value = ''
    cropEditor.value = undefined
  }

  function dispose() {
    reset()
    disposed = true
  }

  return {
    capability,
    feature,
    job,
    operation,
    notice,
    busy,
    operationBusy,
    cropEditor,
    loadCapability,
    loadStatus,
    loadLastOperation,
    start,
    cancel,
    resume: () => undefined,
    review,
    reviewMany,
    edit,
    apply,
    revert,
    saveProposal,
    closeProposal: () => { cropEditor.value = undefined },
    reset,
    dispose,
  }
}
