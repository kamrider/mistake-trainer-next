<script setup lang="ts">
import { isTauri } from '@tauri-apps/api/core'
import { listen, type UnlistenFn } from '@tauri-apps/api/event'
import { computed, inject, onBeforeUnmount, onMounted, reactive, ref, watch } from 'vue'
import { routeLocationKey, routerKey } from 'vue-router'
import CaptureWorkspace from '../../modules/capture/components/CaptureWorkspace.vue'
import CaptureCropEditor from '../../modules/capture/components/CaptureCropEditor.vue'
import {
  commands,
  type CaptureBatchDetail,
  type CaptureBatchSummary,
  type CaptureDraftSummary,
  type CaptureLanAddress,
  type CaptureLanPreflight,
  type CaptureLanSession,
  type CaptureLayoutMode,
  type CaptureCropRecipe,
  type CaptureRecognitionJob,
  type CaptureRecognitionOperationSummary,
  type CaptureRecognitionRegionProposal,
  type OcrCapabilityStatus,
  type OcrRecognitionFeatureStatus,
  type SubjectPreferences,
} from '../../shared/api/bindings'
import { normalizeAppResult } from '../../shared/api/normalize-result'
import { syncControllerKey } from '../sync-controller'

const syncController = inject(syncControllerKey, undefined)
const appRouter = inject(routerKey, undefined)
const currentRoute = inject(routeLocationKey, undefined)
const batches = ref<CaptureBatchSummary[]>([])
const detail = ref<CaptureBatchDetail>()
const busy = ref(false)
const errorMessage = ref('')
const saveState = ref<'idle' | 'saving' | 'saved' | 'error'>('idle')
const commitMessage = ref('')
const previews = reactive<Record<string, string>>({})
const cropEditor = ref<{ itemId: string, itemName: string, dataUrl: string }>()
const recognitionCropEditor = ref<{
  suggestionId: string
  itemName: string
  dataUrl: string
  regions: CaptureRecognitionRegionProposal[]
}>()
const lanAddresses = ref<CaptureLanAddress[]>([])
const lanPreflight = ref<CaptureLanPreflight>()
const lanPreflightBusy = ref(false)
const lanSession = ref<CaptureLanSession>()
const recognitionCapability = ref<OcrCapabilityStatus>()
const recognitionJob = ref<CaptureRecognitionJob>()
const recognitionOperation = ref<CaptureRecognitionOperationSummary>()
const recognitionNotice = ref('')
const recognitionBusy = ref(false)
const recognitionFeature = computed<OcrRecognitionFeatureStatus>(() =>
  recognitionCapability.value?.recognitionFeature ?? {
    state: 'ready',
    requiredComponentId: 'opencv_preprocess',
    detail: '智能切图使用内置本地视觉分析，不读取文字、不需要下载模型；确认后结果只进入素材牌库。',
  })
const subjectPreferences = ref<SubjectPreferences>({
  enabledSubjects: ['语文', '数学', '英语', '政治', '历史', '地理', '物理', '化学', '生物'],
  customSubjects: [],
  captureSoundEnabled: true,
})
const subjectOptions = computed(() => [...new Set([
  ...subjectPreferences.value.enabledSubjects,
  ...subjectPreferences.value.customSubjects,
])])
const previewOrder: string[] = []
const previewRequests = new Set<string>()
const desktopAvailable = isTauri()
let unlistenBatch: UnlistenFn | undefined
let unlistenRecognition: UnlistenFn | undefined
let refreshTimer: ReturnType<typeof setTimeout> | undefined
let lanPollTimer: ReturnType<typeof setInterval> | undefined
let viewMounted = false
let requestedDetailBatchId = ''
type PendingDraftUpdate = {
  batchId: string
  draftId: string
  subject: string
  tags: string[]
  note: string
  attempts: number
}
const pendingDraftUpdates = new Map<string, PendingDraftUpdate>()
let draftSaveRunning = false
type LanPreflightCommandResult = Awaited<ReturnType<typeof commands.captureLanPreflight>>
let lanPreflightRequest: Promise<LanPreflightCommandResult> | undefined

function showError(message: string) {
  errorMessage.value = message
}

async function loadBatches() {
  if (!desktopAvailable) return
  try {
    const result = normalizeAppResult(await commands.captureBatchList())
    if (result.ok) batches.value = result.data
    else showError(result.error.userMessage)
  }
  catch {
    showError('采集箱连接中断，请重新打开应用后重试。')
  }
}

async function loadSubjectPreferences() {
  if (!desktopAvailable) return
  try {
    const result = normalizeAppResult(await commands.subjectPreferencesGet())
    if (result.ok) subjectPreferences.value = result.data
    else showError(result.error.userMessage)
  }
  catch {
    showError('科目配置暂时无法读取，当前仍可使用内置九科。')
  }
}

async function loadDetail(batchId: string) {
  if (!desktopAvailable) return
  requestedDetailBatchId = batchId
  try {
    const [invocation] = await Promise.all([
      commands.captureBatchDetail(batchId),
      loadRecognitionCapability(),
      loadRecognitionStatus(batchId),
      loadRecognitionLastOperation(batchId),
    ])
    const result = normalizeAppResult(invocation)
    if (requestedDetailBatchId !== batchId) return
    if (result.ok) {
      detail.value = result.data
      if (appRouter && currentRoute?.query.batchId !== batchId) {
        void appRouter.replace({
          name: 'inbox',
          query: { ...currentRoute?.query, batchId },
        })
      }
    }
    else showError(result.error.userMessage)
  }
  catch {
    showError('没有读取到这个采集批次，请返回后重试。')
  }
}

function removeCachedPreview(itemId: string) {
  delete previews[itemId]
  for (let index = previewOrder.length - 1; index >= 0; index -= 1) {
    if (previewOrder[index] === itemId) previewOrder.splice(index, 1)
  }
}

async function loadRecognitionCapability() {
  if (!desktopAvailable || !commands.ocrCapabilityStatus) return
  try {
    const invocation = await commands.ocrCapabilityStatus()
    if (invocation.status === 'ok') {
      const result = normalizeAppResult(invocation.data)
      if (result.ok) recognitionCapability.value = result.data
    }
  }
  catch {
    // The workbench remains usable and shows the conservative gate state.
  }
}

async function loadRecognitionStatus(batchId: string) {
  if (!desktopAvailable || !commands.captureRecognitionStatus) return
  try {
    const result = normalizeAppResult(await commands.captureRecognitionStatus(batchId))
    if (result.ok && requestedDetailBatchId === batchId) {
      recognitionJob.value = result.data ?? undefined
    }
  }
  catch {
    // Recognition is optional and must not block manual organization.
  }
}

async function loadRecognitionLastOperation(batchId: string) {
  if (!desktopAvailable || !commands.captureRecognitionLastOperation) return
  try {
    const result = normalizeAppResult(await commands.captureRecognitionLastOperation(batchId))
    if (result.ok && requestedDetailBatchId === batchId) {
      recognitionOperation.value = result.data ?? undefined
    }
  }
  catch {
    // Undo availability is supplementary and must not block the workbench.
  }
}

async function startRecognition() {
  const current = detail.value
  if (!current || recognitionBusy.value || !commands.captureRecognitionStart) return
  recognitionBusy.value = true
  errorMessage.value = ''
  try {
    const result = normalizeAppResult(await commands.captureRecognitionStart({
      batchId: current.batch.id,
      itemIds: [...current.unassignedItemIds],
    }))
    if (result.ok) recognitionJob.value = result.data
    else showError(result.error.userMessage)
  }
  catch {
    showError('智能切图没有启动；原图和当前分组保持不变。')
  }
  finally {
    recognitionBusy.value = false
  }
}

async function cancelRecognition(jobId: string) {
  if (recognitionBusy.value || !commands.captureRecognitionCancel) return
  recognitionBusy.value = true
  try {
    const invocation = await commands.captureRecognitionCancel(jobId)
    if (invocation.status === 'ok') {
      const result = normalizeAppResult(invocation.data)
      if (result.ok) recognitionJob.value = result.data
      else showError(result.error.userMessage)
    }
  }
  catch {
    showError('停止请求没有完成；你仍可继续手工整理。')
  }
  finally {
    recognitionBusy.value = false
  }
}

function resumeRecognition() {
  // The workbench owns the inline disclosure state; the view keeps the job alive.
}

type RecognitionReviewRequest = {
  jobId: string
  suggestionId: string
  decision: 'accepted' | 'rejected'
  editedRegions: CaptureRecognitionRegionProposal[] | null
}

async function persistRecognitionReview(input: RecognitionReviewRequest): Promise<boolean> {
  const previousJob = recognitionJob.value
  if (previousJob) {
    recognitionJob.value = {
      ...previousJob,
      suggestions: previousJob.suggestions.map(suggestion =>
        suggestion.id === input.suggestionId
          ? {
              ...suggestion,
              state: input.decision,
              regions: input.editedRegions ?? suggestion.regions,
            }
          : suggestion),
    }
  }
  try {
    const result = normalizeAppResult(await commands.captureRecognitionReview(input))
    if (result.ok) {
      recognitionJob.value = result.data
      return true
    }
    else {
      recognitionJob.value = previousJob
      showError(result.error.userMessage)
    }
  }
  catch {
    recognitionJob.value = previousJob
    showError('这条识别建议没有保存；你刚才的手工整理不会受到影响。')
  }
  return false
}

async function reviewRecognition(input: RecognitionReviewRequest) {
  if (recognitionBusy.value || !commands.captureRecognitionReview) return false
  recognitionBusy.value = true
  try {
    return await persistRecognitionReview(input)
  }
  finally {
    recognitionBusy.value = false
  }
}

async function reviewRecognitionMany(inputs: RecognitionReviewRequest[]) {
  if (recognitionBusy.value || !commands.captureRecognitionReview || !inputs.length) return
  recognitionBusy.value = true
  try {
    for (const input of inputs) {
      if (!await persistRecognitionReview(input)) break
    }
  }
  finally {
    recognitionBusy.value = false
  }
}

async function editRecognition(suggestionId: string) {
  const current = detail.value
  const suggestion = recognitionJob.value?.suggestions.find(item => item.id === suggestionId)
  if (!current || !suggestion || recognitionBusy.value) return
  recognitionBusy.value = true
  try {
    const result = normalizeAppResult(
      await commands.captureCropSourcePreview(current.batch.id, suggestion.itemId),
    )
    if (result.ok) {
      recognitionCropEditor.value = {
        suggestionId,
        itemName: '识别建议来源图',
        dataUrl: result.data.dataUrl,
        regions: suggestion.regions.map(region => ({ ...region, rect: { ...region.rect } })),
      }
    }
    else showError(result.error.userMessage)
  }
  catch {
    showError('没有读取到建议来源图；当前建议和原图都没有改变。')
  }
  finally {
    recognitionBusy.value = false
  }
}

async function applyRecognition(suggestionIds: string[]) {
  const current = detail.value
  const job = recognitionJob.value
  if (
    !desktopAvailable
    || !current
    || !job
    || recognitionBusy.value
    || !suggestionIds.length
    || !commands.captureRecognitionApply
  ) return
  recognitionBusy.value = true
  errorMessage.value = ''
  recognitionNotice.value = ''
  try {
    const invocation = await commands.captureRecognitionApply({
      batchId: current.batch.id,
      jobId: job.id,
      expectedRevision: current.batch.revision,
      acceptedSuggestionIds: suggestionIds,
    })
    if (invocation.status !== 'ok') {
      showError('识别结果没有应用；原图和当前题卡保持不变。')
      return
    }
    const result = normalizeAppResult(invocation.data)
    if (!result.ok) {
      showError(result.error.userMessage)
      if (result.error.code === 'capture_recognition_stale') {
        await loadRecognitionStatus(current.batch.id)
      }
      return
    }
    detail.value = result.data.detail
    recognitionJob.value = undefined
    recognitionOperation.value = {
      operationId: result.data.operationId,
      batchId: current.batch.id,
      afterRevision: result.data.detail.batch.revision,
      createdItemCount: result.data.createdItemCount,
      reverted: false,
    }
    recognitionNotice.value =
      `已切分 ${result.data.createdItemCount} 张题答图片，已放入素材牌库。`
  }
  catch {
    showError('识别结果没有应用；原图、复核选择和当前题卡都已保留。')
  }
  finally {
    recognitionBusy.value = false
  }
}

async function revertRecognition(operationId: string) {
  const current = detail.value
  if (
    !desktopAvailable
    || !current
    || recognitionBusy.value
    || !commands.captureRecognitionRevert
  ) return
  recognitionBusy.value = true
  errorMessage.value = ''
  try {
    const invocation = await commands.captureRecognitionRevert({
      batchId: current.batch.id,
      operationId,
      expectedRevision: current.batch.revision,
    })
    if (invocation.status !== 'ok') {
      showError('没有撤销智能整理；当前题卡保持不变。')
      return
    }
    const result = normalizeAppResult(invocation.data)
    if (!result.ok) {
      showError(result.error.userMessage)
      return
    }
    detail.value = result.data.detail
    recognitionOperation.value = recognitionOperation.value
      ? { ...recognitionOperation.value, reverted: true }
      : undefined
    recognitionNotice.value =
      `已撤销智能整理，恢复 ${result.data.revertedItemCount} 张来源图的原始状态。`
  }
  catch {
    showError('没有撤销智能整理；当前题卡和图片保持不变。')
  }
  finally {
    recognitionBusy.value = false
  }
}

async function saveRecognitionProposal(recipes: CaptureCropRecipe[]) {
  const editor = recognitionCropEditor.value
  const job = recognitionJob.value
  if (!editor || !job || recipes.length !== editor.regions.length) return
  const editedRegions = recipes.map((recipe, index) => ({
    ...editor.regions[index]!,
    rect: recipe.rect,
  }))
  const saved = await reviewRecognition({
    jobId: job.id,
    suggestionId: editor.suggestionId,
    decision: 'accepted',
    editedRegions,
  })
  if (saved) recognitionCropEditor.value = undefined
}

function openRecognitionSetup() {
  if (!appRouter || !detail.value) return
  void appRouter.push({
    name: 'settings',
    query: {
      section: 'ocr',
      returnTo: 'inbox',
      batchId: detail.value.batch.id,
    },
  })
}

function closeDetail() {
  requestedDetailBatchId = ''
  detail.value = undefined
  recognitionJob.value = undefined
  recognitionOperation.value = undefined
  recognitionNotice.value = ''
  void loadBatches()
  if (appRouter) {
    void appRouter.replace({
      name: 'inbox',
      query: {},
    })
  }
}

function draftUpdateKey(batchId: string, draftId: string) {
  return `${batchId}:${draftId}`
}

async function flushDraftUpdates() {
  if (draftSaveRunning || busy.value || !detail.value || !pendingDraftUpdates.size) return
  const batchId = detail.value.batch.id
  const entry = [...pendingDraftUpdates.entries()].find(([, value]) => value.batchId === batchId)
  if (!entry) {
    for (const [key, value] of pendingDraftUpdates) {
      if (value.batchId !== batchId) pendingDraftUpdates.delete(key)
    }
    return
  }

  const [key, pending] = entry
  pendingDraftUpdates.delete(key)
  draftSaveRunning = true
  busy.value = true
  saveState.value = 'saving'
  try {
    const current = detail.value
    if (!current || current.batch.id !== pending.batchId) return
    const result = normalizeAppResult(await commands.captureDraftUpdate({
      batchId: current.batch.id,
      expectedRevision: current.batch.revision,
      draftId: pending.draftId,
      subject: pending.subject,
      tags: pending.tags,
      note: pending.note,
    }))
    if (result.ok) {
      detail.value = result.data
      saveState.value = 'saved'
    }
    else if (result.error.code === 'capture_revision_conflict' && pending.attempts < 1) {
      await loadDetail(pending.batchId)
      pending.attempts += 1
      pendingDraftUpdates.set(key, pending)
    }
    else {
      saveState.value = 'error'
      showError(result.error.userMessage)
    }
  }
  catch {
    saveState.value = 'error'
    showError('草稿文字保存没有完成；本次编辑仍保留在当前输入框中，请再次修改或重试。')
  }
  finally {
    draftSaveRunning = false
    busy.value = false
  }
}

watch(busy, (isBusy) => {
  if (!isBusy) void flushDraftUpdates()
})

watch(() => detail.value?.batch.id, (batchId) => {
  if (!batchId) return
  for (const [key, value] of pendingDraftUpdates) {
    if (value.batchId !== batchId) pendingDraftUpdates.delete(key)
  }
})

async function createBatch(subject: string) {
  if (!desktopAvailable || busy.value) return
  busy.value = true
  errorMessage.value = ''
  try {
    const result = normalizeAppResult(await commands.captureBatchCreate({ subject }))
    if (result.ok) {
      await loadBatches()
      await loadDetail(result.data.id)
    }
    else showError(result.error.userMessage)
  }
  catch {
    showError('新批次没有创建成功，请稍后重试。')
  }
  finally {
    busy.value = false
  }
}

async function discardBatch(batchId: string) {
  if (!desktopAvailable || busy.value) return
  if (lanSession.value?.batchId === batchId) await stopMobileCapture(true)
  busy.value = true
  try {
    const result = normalizeAppResult(await commands.captureBatchDiscard(batchId))
    if (result.ok) {
      if (detail.value?.batch.id === batchId) detail.value = undefined
      await loadBatches()
    }
    else showError(result.error.userMessage)
  }
  catch {
    showError('批次没有删除成功，原有图片仍会保留。')
  }
  finally {
    busy.value = false
  }
}

async function importSelect() {
  const batchId = detail.value?.batch.id
  if (!desktopAvailable || !batchId || busy.value) return
  busy.value = true
  errorMessage.value = ''
  try {
    const result = normalizeAppResult(await commands.captureImportSelect(batchId))
    if (!result.ok) showError(result.error.userMessage)
    await loadDetail(batchId)
  }
  catch {
    showError('图片选择没有完成，请稍后重试。')
  }
  finally {
    busy.value = false
  }
}

async function importFiles(files: File[]) {
  const batchId = detail.value?.batch.id
  if (!desktopAvailable || !batchId || busy.value || !files.length) return
  busy.value = true
  errorMessage.value = ''
  try {
    const maxBatchItems = 150
    const skippedCount = Math.max(0, files.length - maxBatchItems)
    const filesToImport = files.slice(0, maxBatchItems)
    const failedNames: string[] = []
    const sequenceOffset = detail.value?.items.length ?? 0
    let nextFileIndex = 0
    const importOne = async (file: File, sourceSequence: number) => {
      const sourceName = file.name || 'clipboard-image'
      try {
        const bytes = [...new Uint8Array(await file.arrayBuffer())]
        const result = normalizeAppResult(await commands.captureImportBytes({
          batchId,
          clientUploadId: crypto.randomUUID(),
          sourceName,
          sourceSequence,
          bytes,
        }))
        if (!result.ok) failedNames.push(sourceName)
      }
      catch {
        failedNames.push(sourceName)
      }
    }
    const workerCount = Math.min(2, filesToImport.length)
    await Promise.all(Array.from({ length: workerCount }, async () => {
      while (nextFileIndex < filesToImport.length) {
        const index = nextFileIndex
        nextFileIndex += 1
        await importOne(filesToImport[index]!, sequenceOffset + index)
      }
    }))
    const notices: string[] = []
    if (skippedCount) {
      notices.push(`本批最多保存 ${maxBatchItems} 张，已跳过最后 ${skippedCount} 张图片。`)
    }
    if (failedNames.length) {
      const preview = failedNames.slice(0, 3).join('、')
      const suffix = failedNames.length > 3 ? ` 等 ${failedNames.length} 张` : ''
      notices.push(`${preview}${suffix} 未能加入采集箱，其余图片已继续导入。`)
    }
    if (notices.length) showError(notices.join(' '))
    await loadDetail(batchId)
  }
  catch {
    showError('拖入或粘贴的图片没有全部保存；已成功的图片仍在批次中。')
    await loadDetail(batchId)
  }
  finally {
    busy.value = false
  }
}

async function finishCollecting(subject: string) {
  const current = detail.value
  if (!desktopAvailable || !current || busy.value) return
  if (lanSession.value?.batchId === current.batch.id) await stopMobileCapture(true)
  busy.value = true
  try {
    const result = normalizeAppResult(await commands.captureBatchUpdate({
      batchId: current.batch.id,
      expectedRevision: current.batch.revision,
      subject,
      finishCollecting: true,
    }))
    if (result.ok) await loadDetail(current.batch.id)
    else showError(result.error.userMessage)
  }
  catch {
    showError('没有结束采集，请稍后重试。')
  }
  finally {
    busy.value = false
  }
}

async function applyLayout(mode: CaptureLayoutMode, questions: number, answers: number, splitIndex: number | null) {
  const current = detail.value
  if (!desktopAvailable || !current || busy.value) return
  busy.value = true
  try {
    const result = normalizeAppResult(await commands.captureLayoutApply({
      batchId: current.batch.id,
      expectedRevision: current.batch.revision,
      mode,
      questionImagesPerDraft: questions,
      answerImagesPerDraft: answers,
      splitIndex,
    }))
    if (result.ok) detail.value = result.data
    else showError(result.error.userMessage)
  }
  catch {
    showError('整理模板没有应用，原有分组仍会保留。')
  }
  finally {
    busy.value = false
  }
}

async function assignBatchSubject(subject: string) {
  const current = detail.value
  if (!desktopAvailable || !current || busy.value || !subject.trim()) return
  busy.value = true
  saveState.value = 'saving'
  errorMessage.value = ''
  try {
    const result = normalizeAppResult(await commands.captureBatchAssignSubject({
      batchId: current.batch.id,
      expectedRevision: current.batch.revision,
      subject,
    }))
    if (result.ok) {
      detail.value = result.data
      saveState.value = 'saved'
    }
    else {
      saveState.value = 'error'
      showError(result.error.userMessage)
      if (result.error.code === 'capture_revision_conflict') await loadDetail(current.batch.id)
    }
  }
  catch {
    saveState.value = 'error'
    showError('整批科目没有保存成功，原有题卡保持不变。')
  }
  finally {
    busy.value = false
  }
}

async function moveItem(target: { itemId: string, targetDraftId: string | null, targetRole: 'question' | 'answer' | null, targetPosition: number }) {
  const current = detail.value
  if (!desktopAvailable || !current || busy.value) return
  busy.value = true
  saveState.value = 'saving'
  try {
    const result = normalizeAppResult(await commands.captureItemMove({
      batchId: current.batch.id,
      expectedRevision: current.batch.revision,
      ...target,
    }))
    if (result.ok) {
      detail.value = result.data
      saveState.value = 'saved'
    }
    else {
      saveState.value = 'error'
      showError(result.error.userMessage)
      if (result.error.code === 'capture_revision_conflict') await loadDetail(current.batch.id)
    }
  }
  catch {
    saveState.value = 'error'
    showError('图片没有移动成功，请刷新批次后重试。')
  }
  finally {
    busy.value = false
  }
}

async function stageItemRole(itemId: string, stagedRole: 'question' | 'answer') {
  const current = detail.value
  if (!desktopAvailable || !current || busy.value) return
  busy.value = true
  saveState.value = 'saving'
  errorMessage.value = ''
  try {
    const result = normalizeAppResult(await commands.captureItemStageRole({
      batchId: current.batch.id,
      expectedRevision: current.batch.revision,
      itemId,
      stagedRole,
    }))
    if (result.ok) {
      detail.value = result.data
      saveState.value = 'saved'
    }
    else {
      saveState.value = 'error'
      showError(result.error.userMessage)
      if (result.error.code === 'capture_revision_conflict') await loadDetail(current.batch.id)
    }
  }
  catch {
    saveState.value = 'error'
    showError('图片角色没有保存成功，请重试。')
  }
  finally {
    busy.value = false
  }
}

async function mergeCard(itemIds: string[], targetDraftId: string | null, newDraftSubject: string | null) {
  const current = detail.value
  if (!desktopAvailable || !current || busy.value || !itemIds.length) return
  busy.value = true
  saveState.value = 'saving'
  errorMessage.value = ''
  try {
    const result = normalizeAppResult(await commands.captureCardMerge({
      batchId: current.batch.id,
      expectedRevision: current.batch.revision,
      targetDraftId,
      itemIds,
      newDraftSubject,
    }))
    if (result.ok) {
      detail.value = result.data
      saveState.value = 'saved'
    }
    else {
      saveState.value = 'error'
      showError(result.error.userMessage)
      if (result.error.code === 'capture_revision_conflict') await loadDetail(current.batch.id)
    }
  }
  catch {
    saveState.value = 'error'
    showError('题卡没有保存成功，图片仍保留在原位置。')
  }
  finally {
    busy.value = false
  }
}

async function deleteDraft(draftId: string) {
  const current = detail.value
  if (!desktopAvailable || !current || busy.value) return
  busy.value = true
  saveState.value = 'saving'
  errorMessage.value = ''
  try {
    const result = normalizeAppResult(await commands.captureDraftDelete(
      current.batch.id,
      current.batch.revision,
      draftId,
    ))
    if (result.ok) {
      detail.value = result.data
      saveState.value = 'saved'
    }
    else {
      saveState.value = 'error'
      showError(result.error.userMessage)
      if (result.error.code === 'capture_revision_conflict') await loadDetail(current.batch.id)
    }
  }
  catch {
    saveState.value = 'error'
    showError('题卡没有撤销成功，原有图片和分组仍会保留。')
  }
  finally {
    busy.value = false
  }
}

async function updateDraft(draft: CaptureDraftSummary, subject: string, tags: string[], note: string) {
  const current = detail.value
  if (!desktopAvailable || !current) return
  pendingDraftUpdates.set(draftUpdateKey(current.batch.id, draft.id), {
    batchId: current.batch.id,
    draftId: draft.id,
    subject,
    tags,
    note,
    attempts: 0,
  })
  void flushDraftUpdates()
}

async function removeItem(itemId: string) {
  const current = detail.value
  if (!desktopAvailable || !current || busy.value) return
  if (!window.confirm('删除这张采集图片？如果没有其他引用，对应的加密资产也会被清理。')) return
  busy.value = true
  try {
    const result = normalizeAppResult(await commands.captureItemRemove(current.batch.id, current.batch.revision, itemId))
    if (result.ok) {
      detail.value = result.data
      removeCachedPreview(itemId)
    }
    else showError(result.error.userMessage)
  }
  catch {
    showError('图片没有删除成功。')
  }
  finally {
    busy.value = false
  }
}

async function commitReady() {
  const current = detail.value
  if (!desktopAvailable || !current || busy.value) return
  busy.value = true
  commitMessage.value = ''
  try {
    const result = normalizeAppResult(await commands.captureCommitReady(current.batch.id, current.batch.revision))
    if (result.ok) {
      commitMessage.value = result.data.committedCount
        ? `已将 ${result.data.committedCount} 道题加入题库。`
        : '没有可加入题库的完整题卡。'
      if (result.data.committedCount > 0) syncController?.scheduleMutation()
      await loadDetail(current.batch.id)
      await loadBatches()
    }
    else showError(result.error.userMessage)
  }
  catch {
    showError('批量入库没有完成，所有草稿仍保持原样，可以直接重试。')
  }
  finally {
    busy.value = false
  }
}

async function loadPreview(itemId: string) {
  const batchId = detail.value?.batch.id
  if (!desktopAvailable || !batchId || previews[itemId] || previewRequests.has(itemId)) return
  previewRequests.add(itemId)
  try {
    const result = normalizeAppResult(await commands.captureItemPreview(batchId, itemId))
    if (!result.ok) return
    removeCachedPreview(itemId)
    previews[itemId] = result.data.dataUrl
    previewOrder.push(itemId)
    while (previewOrder.length > 40) {
      const expired = previewOrder.shift()
      if (expired) removeCachedPreview(expired)
    }
  }
  catch {
    // A failed thumbnail must not interrupt organizing the rest of the batch.
  }
  finally {
    previewRequests.delete(itemId)
  }
}

async function openCropEditor(itemId: string) {
  const current = detail.value
  if (!desktopAvailable || !current || current.batch.state !== 'organizing' || busy.value) return
  const item = current.items.find(value => value.id === itemId)
  if (!item || item.cropDerivationId) return
  busy.value = true
  errorMessage.value = ''
  try {
    const result = normalizeAppResult(await commands.captureCropSourcePreview(current.batch.id, itemId))
    if (result.ok) cropEditor.value = { itemId, itemName: item.sourceName, dataUrl: result.data.dataUrl }
    else showError(result.error.userMessage)
  }
  catch {
    showError('没有读取到裁剪大图，原图仍然安全保留，请重试。')
  }
  finally {
    busy.value = false
  }
}

async function applyCrop(recipes: CaptureCropRecipe[]) {
  const current = detail.value
  const editor = cropEditor.value
  if (!desktopAvailable || !current || !editor || busy.value) return
  busy.value = true
  errorMessage.value = ''
  try {
    const result = normalizeAppResult(await commands.captureCropApply({
      batchId: current.batch.id,
      expectedRevision: current.batch.revision,
      itemId: editor.itemId,
      recipes,
    }))
    if (result.ok) {
      detail.value = result.data.detail
      removeCachedPreview(editor.itemId)
      cropEditor.value = undefined
      saveState.value = 'saved'
      await loadBatches()
    }
    else showError(result.error.userMessage)
  }
  catch {
    showError('裁剪没有保存，原图和当前分组均未改变，请重试。')
  }
  finally {
    busy.value = false
  }
}

async function revertCrop(derivationId: string) {
  const current = detail.value
  if (!desktopAvailable || !current || busy.value) return
  if (!window.confirm('恢复裁剪前的原图？这次裁出的所有区域会从采集工作台移除，但原图会回到原来的位置。')) return
  busy.value = true
  errorMessage.value = ''
  try {
    const result = normalizeAppResult(await commands.captureCropRevert({
      batchId: current.batch.id,
      expectedRevision: current.batch.revision,
      derivationId,
    }))
    if (result.ok) {
      for (const item of current.items.filter(value => value.cropDerivationId)) {
        removeCachedPreview(item.id)
      }
      detail.value = result.data
      saveState.value = 'saved'
      await loadBatches()
    }
    else showError(result.error.userMessage)
  }
  catch {
    showError('没有恢复成功，现有图片保持不变。')
  }
  finally {
    busy.value = false
  }
}

async function requestLanPreflight(): Promise<LanPreflightCommandResult> {
  if (lanPreflightRequest) return lanPreflightRequest
  const request = commands.captureLanPreflight()
  lanPreflightRequest = request
  try {
    return await request
  }
  finally {
    if (lanPreflightRequest === request) lanPreflightRequest = undefined
  }
}

async function loadLanAddresses(): Promise<CaptureLanAddress[]> {
  if (!desktopAvailable) return []
  try {
    const result = normalizeAppResult(await commands.captureLanAddresses())
    if (!viewMounted) return []
    if (result.ok) {
      lanAddresses.value = result.data
      return result.data
    }
  }
  catch {
    if (viewMounted) lanAddresses.value = []
  }
  return []
}

async function loadLanPreflight(): Promise<CaptureLanPreflight | undefined> {
  if (!desktopAvailable) return lanPreflight.value
  lanPreflightBusy.value = true
  try {
    const result = normalizeAppResult(await requestLanPreflight())
    if (!viewMounted) return undefined
    if (result.ok) {
      lanPreflight.value = result.data
      return result.data
    }
    lanPreflight.value = undefined
    showError(result.error.userMessage)
  }
  catch {
    if (viewMounted) {
      lanPreflight.value = undefined
      showError('没有读取到 Windows 手机连接权限，请重新检测。')
    }
  }
  finally {
    if (viewMounted) lanPreflightBusy.value = false
  }
  return undefined
}

async function requestLanPermission(): Promise<CaptureLanPreflight | undefined> {
  if (!desktopAvailable) return undefined
  lanPreflightBusy.value = true
  try {
    const result = normalizeAppResult(await commands.captureLanFirewallRepair())
    if (result.ok) {
      lanPreflight.value = result.data
      return result.data
    }
    showError(result.error.userMessage)
  }
  catch {
    showError('Windows 授权没有完成；下次点击“手机扫码”时会再次请求。')
  }
  finally {
    if (viewMounted) lanPreflightBusy.value = false
  }
  return undefined
}

async function loadLanStatus() {
  if (!desktopAvailable) return
  try {
    const result = normalizeAppResult(await commands.captureLanStatus())
    if (result.ok) lanSession.value = result.data ?? undefined
  }
  catch {
    lanSession.value = undefined
  }
}

async function startMobileCapture(selectedAddress: string | null) {
  const requestedBatchId = detail.value?.batch.id
  if (!desktopAvailable || !requestedBatchId || busy.value) return
  busy.value = true
  errorMessage.value = ''
  try {
    let preflight = await loadLanPreflight()
    if (preflight?.needsFirewallRepair) preflight = await requestLanPermission()
    const current = detail.value
    if (!preflight?.canStart || !current || current.batch.id !== requestedBatchId) return
    const addresses = await loadLanAddresses()
    const currentAddress = selectedAddress && addresses.some(address => address.address === selectedAddress)
      ? selectedAddress
      : addresses[0]?.address ?? null
    const result = normalizeAppResult(await commands.captureLanStart({
      batchId: current.batch.id,
      selectedAddress: currentAddress,
    }))
    if (result.ok) lanSession.value = result.data
    else showError(result.error.userMessage)
  }
  catch {
    showError('手机采集服务没有启动成功，请检查 Wi‑Fi 后重试。')
  }
  finally {
    busy.value = false
  }
}

async function stopMobileCapture(silent = false) {
  if (!desktopAvailable) return
  try {
    const result = normalizeAppResult(await commands.captureLanStop())
    if (result.ok) lanSession.value = undefined
    else if (!silent) showError(result.error.userMessage)
  }
  catch {
    if (!silent) showError('手机采集服务没有停止成功；退出应用会强制关闭端口。')
  }
}

function handlePaste(event: ClipboardEvent) {
  if (!detail.value || detail.value.batch.state === 'completed') return
  if (event.target instanceof HTMLInputElement || event.target instanceof HTMLTextAreaElement) return
  const files = [...(event.clipboardData?.items ?? [])]
    .filter(item => item.kind === 'file' && item.type.startsWith('image/'))
    .map(item => item.getAsFile())
    .filter((file): file is File => Boolean(file))
  if (!files.length) return
  event.preventDefault()
  void importFiles(files)
}

function scheduleRefresh(batchId: string) {
  if (refreshTimer) clearTimeout(refreshTimer)
  refreshTimer = setTimeout(() => {
    if (detail.value?.batch.id === batchId) void loadDetail(batchId)
    else void loadBatches()
    void loadLanStatus()
  }, 120)
}

onMounted(async () => {
  viewMounted = true
  window.addEventListener('paste', handlePaste)
  if (!desktopAvailable) {
    showError('浏览器预览只展示界面；请在 Windows 桌面应用中使用加密采集箱。')
    return
  }
  await Promise.all([loadBatches(), loadSubjectPreferences()])
  const requestedBatchId = currentRoute?.query.batchId
  if (
    typeof requestedBatchId === 'string'
    && batches.value.some(batch => batch.id === requestedBatchId)
  ) {
    await loadDetail(requestedBatchId)
  }
  await Promise.all([loadLanAddresses(), loadLanPreflight(), loadLanStatus()])
  lanPollTimer = setInterval(() => void loadLanStatus(), 5_000)
  const eventUnlisteners = await Promise.all([
    listen<{ batchId: string }>('capture_batch_changed', event =>
      scheduleRefresh(event.payload.batchId)),
    listen<{ batchId: string }>('capture_recognition_changed', event => {
      if (detail.value?.batch.id === event.payload.batchId) {
        void loadRecognitionStatus(event.payload.batchId)
      }
    }),
  ])
  unlistenBatch = eventUnlisteners[0]
  unlistenRecognition = eventUnlisteners[1]
})

onBeforeUnmount(() => {
  viewMounted = false
  window.removeEventListener('paste', handlePaste)
  if (refreshTimer) clearTimeout(refreshTimer)
  if (lanPollTimer) clearInterval(lanPollTimer)
  unlistenBatch?.()
  unlistenRecognition?.()
  pendingDraftUpdates.clear()
})
</script>

<template>
  <CaptureWorkspace
    :batches="batches"
    :detail="detail"
    :previews="previews"
    :busy="busy"
    :save-state="saveState"
    :commit-message="commitMessage"
    :error-message="errorMessage"
    :desktop-available="desktopAvailable"
    :lan-addresses="lanAddresses"
    :lan-preflight="lanPreflight"
    :lan-preflight-busy="lanPreflightBusy"
    :lan-session="lanSession"
    :subject-options="subjectOptions"
    :capture-sound-enabled="subjectPreferences.captureSoundEnabled"
    :recognition-feature="recognitionFeature"
    :recognition-job="recognitionJob"
    :recognition-operation="recognitionOperation"
    :recognition-notice="recognitionNotice"
    :recognition-busy="recognitionBusy"
    @create-batch="createBatch"
    @open-batch="loadDetail"
    @back="closeDetail"
    @discard-batch="discardBatch"
    @import-select="importSelect"
    @import-files="importFiles"
    @finish-collecting="finishCollecting"
    @apply-layout="applyLayout"
    @assign-batch-subject="assignBatchSubject"
    @move-item="moveItem"
    @stage-item-role="stageItemRole"
    @merge-card="mergeCard"
    @delete-draft="deleteDraft"
    @update-draft="updateDraft"
    @remove-item="removeItem"
    @commit-ready="commitReady"
    @preview="loadPreview"
    @crop="openCropEditor"
    @revert-crop="revertCrop"
    @mobile-capture="startMobileCapture"
    @refresh-lan-addresses="loadLanAddresses"
    @refresh-lan-preflight="loadLanPreflight"
    @stop-mobile-capture="stopMobileCapture()"
    @recognition-start="startRecognition"
    @recognition-cancel="cancelRecognition"
    @recognition-resume="resumeRecognition"
    @recognition-open-setup="openRecognitionSetup"
    @recognition-review="reviewRecognition"
    @recognition-review-many="reviewRecognitionMany"
    @recognition-edit="editRecognition"
    @recognition-apply="applyRecognition"
    @recognition-revert="revertRecognition"
  />
  <CaptureCropEditor
    v-if="cropEditor"
    :data-url="cropEditor.dataUrl"
    :item-name="cropEditor.itemName"
    :busy="busy"
    @close="cropEditor = undefined"
    @apply="applyCrop"
  />
  <CaptureCropEditor
    v-if="recognitionCropEditor"
    mode="proposal"
    :data-url="recognitionCropEditor.dataUrl"
    :item-name="recognitionCropEditor.itemName"
    :busy="recognitionBusy"
    :initial-recipes="recognitionCropEditor.regions.map(region => ({
      rect: region.rect,
      rotationDegrees: 0,
      outputMediaType: 'image/png',
      maxEdge: 4096,
      jpegQuality: 90,
    }))"
    @close="recognitionCropEditor = undefined"
    @save-proposal="saveRecognitionProposal"
  />
</template>
