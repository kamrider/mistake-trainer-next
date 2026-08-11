<script setup lang="ts">
import { isTauri } from '@tauri-apps/api/core'
import { listen, type UnlistenFn } from '@tauri-apps/api/event'
import { computed, inject, onBeforeUnmount, onMounted, ref, watch } from 'vue'
import { routeLocationKey, routerKey } from 'vue-router'
import ActionConfirmDialog from '../components/ActionConfirmDialog.vue'
import { useActionConfirmation } from '../composables/useActionConfirmation'
import { useUnsavedChangesGuard } from '../composables/useUnsavedChangesGuard'
import CaptureWorkspace from '../../modules/capture/components/CaptureWorkspace.vue'
import CaptureCropEditor from '../../modules/capture/components/CaptureCropEditor.vue'
import {
  useCaptureDraftSaveQueue,
  type CaptureDraftSaveOutcome,
  type CaptureDraftSaveQueueState,
  type CaptureDraftSaveUpdate,
} from '../../modules/capture/composables/useCaptureDraftSaveQueue'
import { useCaptureBatchLifecycle } from '../../modules/capture/composables/useCaptureBatchLifecycle'
import { useCaptureFileImport } from '../../modules/capture/composables/useCaptureFileImport'
import { useCaptureImportWorkflow } from '../../modules/capture/composables/useCaptureImportWorkflow'
import {
  useCaptureItemEditing,
  type CaptureCropEditorState,
} from '../../modules/capture/composables/useCaptureItemEditing'
import { useCaptureLanSession } from '../../modules/capture/composables/useCaptureLanSession'
import { useCaptureOrganizerActions } from '../../modules/capture/composables/useCaptureOrganizerActions'
import { useCapturePreviewCache } from '../../modules/capture/composables/useCapturePreviewCache'
import { useCaptureRefreshScheduler } from '../../modules/capture/composables/useCaptureRefreshScheduler'
import { useCaptureRecognitionWorkflow } from '../../modules/ocr/composables/useCaptureRecognitionWorkflow'
import {
  createCaptureDevelopmentCropEditor,
  createCaptureDevelopmentPreview,
} from './capture-development-preview'
import {
  commands,
  type CaptureBatchDetail,
  type CaptureBatchSummary,
  type CaptureDraftSummary,
  type CaptureCropRecipe,
  type CaptureQualityReport,
  type SubjectPreferences,
} from '../../shared/api/bindings'
import { normalizeAppResult } from '../../shared/api/normalize-result'
import { createModalReturnFocusController } from '../modal-return-focus'
import { syncControllerKey } from '../sync-controller'
import { workspaceTransitionGuardKey } from '../workspace-transition-guard'

const syncController = inject(syncControllerKey, undefined)
const workspaceTransitionGuard = inject(workspaceTransitionGuardKey, undefined)
const appRouter = inject(routerKey, undefined)
const currentRoute = inject(routeLocationKey, undefined)
const batches = ref<CaptureBatchSummary[]>([])
const detail = ref<CaptureBatchDetail>()
const busy = ref(false)
const errorMessage = ref('')
const saveState = ref<'idle' | 'saving' | 'saved' | 'error'>('idle')
const draftSaveQueueState = ref<CaptureDraftSaveQueueState>({
  pending: false,
  running: false,
  retryRequired: false,
})
const draftSaveUnsaved = computed(() =>
  draftSaveQueueState.value.pending
  || draftSaveQueueState.value.running
  || draftSaveQueueState.value.retryRequired)
const draftSaveRetryAvailable = computed(() => draftSaveQueueState.value.retryRequired)
const commitMessage = ref('')
const qualityReports = ref<Record<string, CaptureQualityReport>>({})
const qualityErrors = ref<Record<string, string>>({})
const qualityCheckingItemId = ref('')
const qualityDismissedItemIds = ref<string[]>([])
const subjectPreferences = ref<SubjectPreferences>({
  enabledSubjects: ['语文', '数学', '英语', '政治', '历史', '地理', '物理', '化学', '生物'],
  customSubjects: [],
  captureSoundEnabled: true,
})
const subjectOptions = computed(() => [...new Set([
  ...subjectPreferences.value.enabledSubjects,
  ...subjectPreferences.value.customSubjects,
])])
let requestedDetailBatchId = ''

const recognitionWorkflow = useCaptureRecognitionWorkflow({
  desktopAvailable: isTauri(),
  requestedBatchId: () => requestedDetailBatchId,
  activeDetail: () => detail.value,
  onDetailChange: value => { detail.value = value },
  onError: showError,
  operations: {
    capability: async () => {
      const invocation = await commands.ocrCapabilityStatus()
      if (invocation.status !== 'ok') throw new Error('OCR capability transport failed')
      return normalizeAppResult(invocation.data)
    },
    status: async batchId =>
      normalizeAppResult(await commands.captureRecognitionStatus(batchId)),
    lastOperation: async batchId =>
      normalizeAppResult(await commands.captureRecognitionLastOperation(batchId)),
    start: async input =>
      normalizeAppResult(await commands.captureRecognitionStart(input)),
    cancel: async (jobId) => {
      const invocation = await commands.captureRecognitionCancel(jobId)
      if (invocation.status !== 'ok') throw new Error('Recognition cancel transport failed')
      return normalizeAppResult(invocation.data)
    },
    review: async input =>
      normalizeAppResult(await commands.captureRecognitionReview(input)),
    preview: async (batchId, itemId) =>
      normalizeAppResult(await commands.captureCropSourcePreview(batchId, itemId)),
    apply: async (input) => {
      const invocation = await commands.captureRecognitionApply(input)
      if (invocation.status !== 'ok') throw new Error('Recognition apply transport failed')
      return normalizeAppResult(invocation.data)
    },
    revert: async (input) => {
      const invocation = await commands.captureRecognitionRevert(input)
      if (invocation.status !== 'ok') throw new Error('Recognition revert transport failed')
      return normalizeAppResult(invocation.data)
    },
  },
})
const {
  feature: recognitionFeature,
  job: recognitionJob,
  operation: recognitionOperation,
  notice: recognitionNotice,
  busy: recognitionBusy,
  operationBusy: recognitionOperationBusy,
  cropEditor: recognitionCropEditor,
  loadCapability: loadRecognitionCapability,
  loadStatus: loadRecognitionStatus,
  loadLastOperation: loadRecognitionLastOperation,
  start: startRecognition,
  cancel: cancelRecognition,
  resume: resumeRecognition,
  review: reviewRecognition,
  reviewMany: reviewRecognitionMany,
  edit: editRecognition,
  apply: applyRecognition,
  revert: revertRecognition,
  saveProposal: saveRecognitionProposal,
  closeProposal: closeRecognitionProposal,
  reset: resetRecognition,
  dispose: disposeRecognition,
} = recognitionWorkflow
const {
  current: destructiveConfirmation,
  ask: askDestructiveConfirmation,
  confirm: confirmDestructiveAction,
  cancel: cancelDestructiveAction,
} = useActionConfirmation()
const draftPersistenceBusy = computed(() =>
  draftSaveQueueState.value.pending || draftSaveQueueState.value.running)
const captureOperationBusy = computed(() => busy.value || recognitionBusy.value)
const captureTransitionBusy = computed(() =>
  draftPersistenceBusy.value
  || captureOperationBusy.value
  || Boolean(destructiveConfirmation.value))
const draftSaveBlockedMessage = '最新草稿仍在保存，请等待完成后再离开。'
const captureOperationBlockedMessage = '采集操作正在完成，请等待完成后再离开。'
const destructiveConfirmationBlockedMessage = '请先完成当前确认操作，再离开采集箱。'
const {
  current: draftLeaveConfirmation,
  confirm: confirmDraftLeave,
  cancel: cancelDraftLeave,
  attemptLeave: attemptDraftLeave,
} = useUnsavedChangesGuard({
  dirty: () => draftSaveUnsaved.value,
  busy: () => captureTransitionBusy.value,
  onBusy: () => {
    showError(destructiveConfirmation.value
      ? destructiveConfirmationBlockedMessage
      : draftPersistenceBusy.value
        ? draftSaveBlockedMessage
        : captureOperationBlockedMessage)
  },
  ...(appRouter
    ? {
        registerNavigation: attempt => appRouter.beforeEach((to, from) => {
          if (from.name !== 'inbox') return true
          if (to.name !== 'inbox') return attempt()
          const activeBatchId = detail.value?.batch.id
          if (!activeBatchId) return true
          const targetBatchId = typeof to.query.batchId === 'string'
            ? to.query.batchId
            : undefined
          if (targetBatchId === activeBatchId) return true
          return attempt()
        }),
    }
    : {}),
  ...(workspaceTransitionGuard
    ? { registerContextTransition: workspaceTransitionGuard.register }
    : {}),
  confirmation: {
    eyebrow: '采集草稿 · 离开确认',
    title: '放弃尚未保存的采集草稿？',
    description: '最近一次科目、标签或备注修改尚未保存。你可以继续留在采集箱重试，也可以明确放弃这次修改并离开。',
    cancelLabel: '继续留在采集箱',
    confirmLabel: '放弃草稿修改并离开',
    tone: 'danger',
  },
})
watch(captureTransitionBusy, (isBusy) => {
  if (
    !isBusy
    && [
      draftSaveBlockedMessage,
      captureOperationBlockedMessage,
      destructiveConfirmationBlockedMessage,
    ].includes(errorMessage.value)
  ) {
    errorMessage.value = ''
  }
})
const desktopAvailable = isTauri()
type DevelopmentCapturePreviewMode = 'capture-batches' | 'capture-card' | 'crop-editor'
const developmentCapturePreviewMode = computed<DevelopmentCapturePreviewMode | undefined>(() => {
  if (!import.meta.env.DEV || desktopAvailable) return undefined
  const mode = currentRoute?.query.preview
  return mode === 'capture-batches' || mode === 'capture-card' || mode === 'crop-editor'
    ? mode
    : undefined
})
const developmentCropEditor = ref<CaptureCropEditorState>()
let unlistenBatch: UnlistenFn | undefined
let unlistenRecognition: UnlistenFn | undefined

function showError(message: string) {
  errorMessage.value = message
}

const organizerActions = useCaptureOrganizerActions({
  desktopAvailable,
  activeDetail: () => detail.value,
  isBlocked: () => busy.value,
  onBusyChange: value => { busy.value = value },
  onSaveStateChange: value => { saveState.value = value },
  onDetailChange: value => { detail.value = value },
  onError: showError,
  reloadDetail: loadDetail,
  operations: {
    applyLayout: async input => normalizeAppResult(await commands.captureLayoutApply(input)),
    assignSubject: async input => normalizeAppResult(await commands.captureBatchAssignSubject(input)),
    moveItem: async input => normalizeAppResult(await commands.captureItemMove(input)),
    stageRole: async input => normalizeAppResult(await commands.captureItemStageRole(input)),
    mergeCard: async input => normalizeAppResult(await commands.captureCardMerge(input)),
    deleteDraft: async (batchId, expectedRevision, draftId) =>
      normalizeAppResult(await commands.captureDraftDelete(batchId, expectedRevision, draftId)),
  },
})
const {
  applyLayout,
  assignBatchSubject,
  moveItem,
  stageItemRole,
  mergeCard,
  deleteDraft,
} = organizerActions

const previewCache = useCapturePreviewCache({
  activeBatchId: () => desktopAvailable ? detail.value?.batch.id : undefined,
  fetchPreview: async (batchId, itemId) =>
    normalizeAppResult(await commands.captureItemPreview(batchId, itemId)),
  maxEntries: 40,
})
const previews = previewCache.previews
const loadPreview = previewCache.load
const removeCachedPreview = previewCache.invalidate

function loadDevelopmentCapturePreview(mode: DevelopmentCapturePreviewMode) {
  const preview = createCaptureDevelopmentPreview()
  batches.value = preview.batches
  detail.value = mode === 'capture-batches' ? undefined : preview.detail
  previewCache.clear()
  Object.assign(previews, preview.previews)
  developmentCropEditor.value = mode === 'crop-editor'
    ? createCaptureDevelopmentCropEditor(preview)
    : undefined
}

const itemEditing = useCaptureItemEditing({
  desktopAvailable,
  activeDetail: () => detail.value,
  isBlocked: () => busy.value,
  onBusyChange: value => { busy.value = value },
  onSaveStateChange: value => { saveState.value = value },
  onDetailChange: value => { detail.value = value },
  onError: showError,
  confirm: askDestructiveConfirmation,
  invalidatePreview: removeCachedPreview,
  loadBatches,
  loadDetail,
  operations: {
    remove: async (batchId, expectedRevision, itemId) =>
      normalizeAppResult(await commands.captureItemRemove(batchId, expectedRevision, itemId)),
    preview: async (batchId, itemId) =>
      normalizeAppResult(await commands.captureCropSourcePreview(batchId, itemId)),
    apply: async input => normalizeAppResult(await commands.captureCropApply(input)),
    revert: async input => normalizeAppResult(await commands.captureCropRevert(input)),
  },
})
const {
  cropEditor,
  closeCropEditor,
  removeItem,
  openCropEditor,
  applyCrop,
  revertCrop,
} = itemEditing
const visibleCropEditor = computed(() => developmentCropEditor.value ?? cropEditor.value)

function cropLauncherFor(itemId: string) {
  return Array.from(document.querySelectorAll<HTMLButtonElement>('button[data-crop-item-id]'))
    .find(button => button.dataset.cropItemId === itemId && !button.disabled)
}

function cropResultControlFor(itemId: string) {
  return Array.from(document.querySelectorAll<HTMLButtonElement>('button[data-crop-result-item-id]'))
    .find(button => button.dataset.cropResultItemId === itemId && !button.disabled)
}

function recognitionEditControlFor(suggestionId: string) {
  return Array.from(document.querySelectorAll<HTMLButtonElement>('button[data-recognition-edit-suggestion-id]'))
    .find(button => button.dataset.recognitionEditSuggestionId === suggestionId && !button.disabled)
}

const cropReturnFocus = createModalReturnFocusController({
  currentContextId: () => detail.value?.batch.id,
  isModalOpen: () => Boolean(visibleCropEditor.value),
  findFallback: cropLauncherFor,
})

const recognitionCropReturnFocus = createModalReturnFocusController({
  currentContextId: () => detail.value?.batch.id,
  isModalOpen: () => Boolean(recognitionCropEditor.value),
  findFallback: recognitionEditControlFor,
})

async function openVisibleCropEditor(itemId: string) {
  const activeElement = document.activeElement
  const batchId = detail.value?.batch.id
  if (!batchId) {
    cropReturnFocus.clear()
    await openCropEditor(itemId, qualityCropSeed(itemId))
    return
  }
  cropReturnFocus.capture({
    contextId: batchId,
    targetId: itemId,
    element: activeElement instanceof HTMLButtonElement && activeElement.dataset.cropItemId === itemId
      ? activeElement
      : undefined,
  })
  await openCropEditor(itemId, qualityCropSeed(itemId))
  if (!cropEditor.value) await cropReturnFocus.restore()
}

async function closeVisibleCropEditor() {
  developmentCropEditor.value = undefined
  closeCropEditor()
  await cropReturnFocus.restore()
}

async function applyVisibleCrop(recipes: CaptureCropRecipe[]) {
  if (developmentCropEditor.value) {
    developmentCropEditor.value = undefined
    await cropReturnFocus.restore()
    return
  }
  const report = await applyCrop(recipes)
  if (!cropEditor.value) {
    await cropReturnFocus.restore(
      report?.derivedItemIds[0]
        ? () => cropResultControlFor(report.derivedItemIds[0]!)
        : undefined,
    )
  }
}

async function openRecognitionCropEditor(suggestionId: string) {
  const activeElement = document.activeElement
  const batchId = detail.value?.batch.id
  if (!batchId) {
    recognitionCropReturnFocus.clear()
    await editRecognition(suggestionId)
    return
  }
  recognitionCropReturnFocus.capture({
    contextId: batchId,
    targetId: suggestionId,
    element: activeElement instanceof HTMLButtonElement
      && activeElement.dataset.recognitionEditSuggestionId === suggestionId
      ? activeElement
      : undefined,
  })
  await editRecognition(suggestionId)
  if (!recognitionCropEditor.value) await recognitionCropReturnFocus.restore()
}

async function closeRecognitionCropEditor() {
  closeRecognitionProposal()
  await recognitionCropReturnFocus.restore()
}

async function saveRecognitionCropEditor(recipes: CaptureCropRecipe[]) {
  await saveRecognitionProposal(recipes)
  if (!recognitionCropEditor.value) await recognitionCropReturnFocus.restore()
}

const fileImporter = useCaptureFileImport({
  activeBatchId: () => desktopAvailable ? detail.value?.batch.id : undefined,
  currentItemCount: () => detail.value?.items.length ?? 0,
  isBlocked: () => busy.value,
  onBusyChange: (isBusy) => { busy.value = isBusy },
  importBytes: async input =>
    normalizeAppResult(await commands.captureImportBytes(input)),
  createUploadId: () => crypto.randomUUID(),
  maxBatchItems: 150,
  concurrency: 2,
})

const importWorkflow = useCaptureImportWorkflow({
  desktopAvailable,
  activeDetail: () => detail.value,
  isBlocked: () => busy.value,
  onBusyChange: value => { busy.value = value },
  onError: showError,
  loadBatches,
  loadDetail,
  select: async batchId => normalizeAppResult(await commands.captureImportSelect(batchId)),
  fileImporter,
})
const {
  progress: importProgress,
  importSelect,
  importFiles,
  importFromPaste,
  clearProgress: clearImportProgress,
  dispose: disposeImportWorkflow,
} = importWorkflow

const captureLan = useCaptureLanSession({
  desktopAvailable,
  activeBatchId: () => detail.value?.batch.id,
  isBlocked: () => busy.value,
  onBusyChange: (isBusy) => { busy.value = isBusy },
  onError: showError,
  operations: {
    addresses: async () => normalizeAppResult(await commands.captureLanAddresses()),
    preflight: async () => normalizeAppResult(await commands.captureLanPreflight()),
    repair: async () => normalizeAppResult(await commands.captureLanFirewallRepair()),
    status: async () => normalizeAppResult(await commands.captureLanStatus()),
    start: async input => normalizeAppResult(await commands.captureLanStart(input)),
    stop: async () => normalizeAppResult(await commands.captureLanStop()),
  },
})
const {
  addresses: lanAddresses,
  preflight: lanPreflight,
  preflightBusy: lanPreflightBusy,
  session: lanSession,
  loadAddresses: loadLanAddresses,
  loadPreflight: loadLanPreflight,
  loadStatus: loadLanStatus,
  stop: stopMobileCapture,
} = captureLan

async function startMobileCapture(selectedAddress: string | null) {
  if (!desktopAvailable || !detail.value?.batch.id || busy.value) return
  errorMessage.value = ''
  await captureLan.start(selectedAddress)
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
    const detailRequest = commands.captureBatchDetail(batchId)
    void loadRecognitionCapability()
    void loadRecognitionStatus(batchId)
    void loadRecognitionLastOperation(batchId)
    const invocation = await detailRequest
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
    if (requestedDetailBatchId === batchId) showError('没有读取到这个采集批次，请返回后重试。')
  }
}

function qualityCropSeed(itemId: string) {
  const report = qualityReports.value[itemId]
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

async function checkCaptureQuality(itemId: string) {
  const batchId = detail.value?.batch.id
  if (!desktopAvailable || !batchId || qualityReports.value[itemId] || qualityCheckingItemId.value === itemId) return
  qualityCheckingItemId.value = itemId
  const remainingErrors = { ...qualityErrors.value }
  delete remainingErrors[itemId]
  qualityErrors.value = remainingErrors
  try {
    const result = normalizeAppResult(await commands.captureQualityCheck(batchId, itemId))
    if (detail.value?.batch.id !== batchId) return
    if (result.ok) qualityReports.value = { ...qualityReports.value, [itemId]: result.data }
    else qualityErrors.value = { ...qualityErrors.value, [itemId]: result.error.userMessage }
  }
  catch {
    if (detail.value?.batch.id === batchId) {
      qualityErrors.value = { ...qualityErrors.value, [itemId]: '图片仍可继续使用，也可以稍后重新检查。' }
    }
  }
  finally {
    if (qualityCheckingItemId.value === itemId) qualityCheckingItemId.value = ''
  }
}

function dismissCaptureQuality(itemId: string) {
  if (!qualityDismissedItemIds.value.includes(itemId)) {
    qualityDismissedItemIds.value = [...qualityDismissedItemIds.value, itemId]
  }
}

const refreshScheduler = useCaptureRefreshScheduler({
  activeBatchId: () => detail.value?.batch.id,
  refreshDetail: loadDetail,
  refreshList: loadBatches,
  refreshLanStatus: loadLanStatus,
  delayMs: 120,
})

async function applyPairSuggestions(pairIds: string[]) {
  const current = detail.value
  if (!desktopAvailable || !current || busy.value || !pairIds.length) return
  busy.value = true
  saveState.value = 'saving'
  errorMessage.value = ''
  try {
    const result = normalizeAppResult(await commands.capturePairSuggestionsApply({
      batchId: current.batch.id,
      expectedRevision: current.batch.revision,
      pairIds,
    }))
    if (result.ok) {
      detail.value = result.data
      saveState.value = 'saved'
      recognitionNotice.value = `已把 ${pairIds.length} 组题面与答案生成采集草稿；确认科目后再保存到正式题库。`
    }
    else {
      saveState.value = 'error'
      showError(
        result.error.code === 'capture_input_invalid'
          ? '这组题答素材刚刚被移动、改角色或已加入其他题卡，已刷新并保留你的现有整理。'
          : result.error.userMessage,
      )
      if (
        result.error.code === 'capture_revision_conflict'
        || result.error.code === 'capture_input_invalid'
      ) {
        await loadDetail(current.batch.id)
      }
    }
  }
  catch {
    saveState.value = 'error'
    showError('题答匹配没有应用；素材牌库和现有题卡保持不变。')
  }
  finally {
    busy.value = false
  }
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

function clearDetailState(reloadBatches: boolean) {
  requestedDetailBatchId = ''
  detail.value = undefined
  commitMessage.value = ''
  cropReturnFocus.clear()
  recognitionCropReturnFocus.clear()
  closeCropEditor()
  resetRecognition()
  if (reloadBatches) void loadBatches()
}

function leaveDetail(reloadBatches: boolean) {
  clearDetailState(reloadBatches)
  if (appRouter) {
    void appRouter.replace({
      name: 'inbox',
      query: {},
    })
  }
}

async function persistDraftUpdate(
  update: CaptureDraftSaveUpdate,
): Promise<CaptureDraftSaveOutcome> {
  const current = detail.value
  if (!current || current.batch.id !== update.batchId) {
    return {
      kind: 'failed',
      message: '当前采集批次已经切换，本次草稿没有保存。',
    }
  }
  try {
    const result = normalizeAppResult(await commands.captureDraftUpdate({
      batchId: current.batch.id,
      expectedRevision: current.batch.revision,
      draftId: update.draftId,
      subject: update.subject,
      tags: update.tags,
      note: update.note,
    }))
    if (result.ok) {
      if (detail.value?.batch.id === update.batchId) {
        detail.value = result.data
      }
      return { kind: 'saved' }
    }
    if (result.error.code === 'capture_revision_conflict') {
      return {
        kind: 'revision_conflict',
        message: result.error.userMessage,
      }
    }
    return {
      kind: 'failed',
      message: result.error.userMessage,
    }
  }
  catch {
    return {
      kind: 'failed',
      message: '草稿文字保存没有完成；本次编辑仍保留在当前输入框中，请再次修改或重试。',
    }
  }
}

function closeDetail() {
  if (!draftSaveUnsaved.value) {
    leaveDetail(true)
    return
  }
  void (async () => {
    if (await attemptDraftLeave()) leaveDetail(true)
  })()
}

function openBatch(batchId: string) {
  if (detail.value?.batch.id === batchId) return
  if (!draftSaveUnsaved.value) return loadDetail(batchId)
  void (async () => {
    if (!await attemptDraftLeave()) return
    draftSaveQueue.clear()
    await loadDetail(batchId)
  })()
}

const draftSaveQueue = useCaptureDraftSaveQueue({
  activeBatchId: () => detail.value?.batch.id,
  isBlocked: () => busy.value,
  perform: persistDraftUpdate,
  refresh: loadDetail,
  onSaving: () => { saveState.value = 'saving' },
  onSaved: () => { saveState.value = 'saved' },
  onFailed: (message) => {
    saveState.value = 'error'
    showError(message)
  },
  onBusyChange: (isBusy) => { busy.value = isBusy },
  onStateChange: (state) => {
    draftSaveQueueState.value = state
    if (state.retryRequired && !state.pending && !state.running) {
      saveState.value = 'error'
    }
  },
})

async function retryDraftSave() {
  errorMessage.value = ''
  await draftSaveQueue.retry()
}

watch(busy, (isBusy) => {
  if (!isBusy) void draftSaveQueue.flush()
})

watch(() => detail.value?.batch.id, (batchId) => {
  previewCache.clear()
  clearImportProgress()
  qualityReports.value = {}
  qualityErrors.value = {}
  qualityCheckingItemId.value = ''
  qualityDismissedItemIds.value = []
  if (batchId) draftSaveQueue.retainBatch(batchId)
  else draftSaveQueue.clear()
}, { flush: 'sync' })

watch(
  () => currentRoute
    ? [currentRoute.name, currentRoute.query.batchId] as const
    : undefined,
  (routeState) => {
    if (!routeState || routeState[0] !== 'inbox') return
    const targetBatchId = typeof routeState[1] === 'string' ? routeState[1] : ''
    if (targetBatchId) {
      if (
        detail.value?.batch.id !== targetBatchId
        && requestedDetailBatchId !== targetBatchId
      ) {
        void loadDetail(targetBatchId)
      }
    }
    else if (detail.value) {
      clearDetailState(true)
    }
  },
  { flush: 'post' },
)

const batchLifecycle = useCaptureBatchLifecycle({
  desktopAvailable,
  activeDetail: () => detail.value,
  activeLanBatchId: () => lanSession.value?.batchId,
  isBlocked: () => busy.value,
  onBusyChange: value => { busy.value = value },
  onError: showError,
  onCommitMessage: message => { commitMessage.value = message },
  onActiveBatchDiscarded: (batchId) => {
    if (detail.value?.batch.id === batchId) leaveDetail(false)
  },
  loadBatches,
  loadDetail,
  stopMobileCapture,
  scheduleSyncMutation: () => syncController?.scheduleMutation(),
  operations: {
    create: async input => normalizeAppResult(await commands.captureBatchCreate(input)),
    discard: async batchId => normalizeAppResult(await commands.captureBatchDiscard(batchId)),
    update: async input => normalizeAppResult(await commands.captureBatchUpdate(input)),
    commit: async (batchId, expectedRevision) =>
      normalizeAppResult(await commands.captureCommitReady(batchId, expectedRevision)),
  },
})
const { createBatch, discardBatch, finishCollecting, commitReady } = batchLifecycle

function updateDraft(draft: CaptureDraftSummary, subject: string, tags: string[], note: string) {
  const current = detail.value
  if (!desktopAvailable || !current) return
  draftSaveQueue.enqueue({
    batchId: current.batch.id,
    draftId: draft.id,
    subject,
    tags,
    note,
  })
}

onMounted(async () => {
  window.addEventListener('paste', importFromPaste)
  if (!desktopAvailable) {
    const previewMode = developmentCapturePreviewMode.value
    if (previewMode) {
      loadDevelopmentCapturePreview(previewMode)
      return
    }
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
  captureLan.startPolling(5_000)
  const eventUnlisteners = await Promise.all([
    listen<{ batchId: string }>('capture_batch_changed', event =>
      refreshScheduler.schedule(event.payload.batchId)),
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
  cropReturnFocus.clear()
  recognitionCropReturnFocus.clear()
  window.removeEventListener('paste', importFromPaste)
  unlistenBatch?.()
  unlistenRecognition?.()
  refreshScheduler.dispose()
  disposeImportWorkflow()
  previewCache.dispose()
  captureLan.dispose()
  draftSaveQueue.dispose()
  disposeRecognition()
})
</script>

<template>
  <CaptureWorkspace
    :batches="batches"
    :detail="detail"
    :previews="previews"
    :busy="busy"
    :save-state="saveState"
    :draft-save-retry-available="draftSaveRetryAvailable"
    :commit-message="commitMessage"
    :error-message="errorMessage"
    :desktop-available="desktopAvailable"
    :lan-addresses="lanAddresses"
    :lan-preflight="lanPreflight"
    :lan-preflight-busy="lanPreflightBusy"
    :lan-session="lanSession"
    :subject-options="subjectOptions"
    :capture-sound-enabled="subjectPreferences.captureSoundEnabled"
    :import-progress="importProgress"
    :recognition-feature="recognitionFeature"
    :recognition-job="recognitionJob"
    :recognition-operation="recognitionOperation"
    :recognition-notice="recognitionNotice"
    :recognition-busy="recognitionBusy"
    :recognition-operation-busy="recognitionOperationBusy"
    :quality-reports="qualityReports"
    :quality-errors="qualityErrors"
    :quality-checking-item-id="qualityCheckingItemId"
    :quality-dismissed-item-ids="qualityDismissedItemIds"
    @create-batch="createBatch"
    @open-batch="openBatch"
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
    @apply-pair-suggestions="applyPairSuggestions"
    @delete-draft="deleteDraft"
    @update-draft="updateDraft"
    @retry-draft-save="retryDraftSave"
    @remove-item="removeItem"
    @commit-ready="commitReady"
    @preview="loadPreview"
    @crop="openVisibleCropEditor"
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
    @recognition-edit="openRecognitionCropEditor"
    @recognition-apply="applyRecognition"
    @recognition-revert="revertRecognition"
    @quality-check="checkCaptureQuality"
    @quality-dismiss="dismissCaptureQuality"
  />
  <CaptureCropEditor
    v-if="visibleCropEditor"
    :data-url="visibleCropEditor.dataUrl"
    :item-name="visibleCropEditor.itemName"
    :busy="busy"
    :initial-recipes="visibleCropEditor.initialRecipes"
    :suggested-rotation-degrees="visibleCropEditor.suggestedRotationDegrees"
    @close="closeVisibleCropEditor"
    @apply="applyVisibleCrop"
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
    @close="closeRecognitionCropEditor"
    @save-proposal="saveRecognitionCropEditor"
  />
  <ActionConfirmDialog
    v-if="destructiveConfirmation"
    :request="destructiveConfirmation"
    @cancel="cancelDestructiveAction"
    @confirm="confirmDestructiveAction"
  />
  <ActionConfirmDialog
    v-if="draftLeaveConfirmation"
    :request="draftLeaveConfirmation"
    @cancel="cancelDraftLeave"
    @confirm="confirmDraftLeave"
  />
</template>
