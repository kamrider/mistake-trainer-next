<script setup lang="ts">
import { isTauri } from '@tauri-apps/api/core'
import { listen, type UnlistenFn } from '@tauri-apps/api/event'
import { computed, inject, onBeforeUnmount, onMounted, ref, watch } from 'vue'
import { routeLocationKey, routerKey } from 'vue-router'
import ActionConfirmDialog from '@/shared/ui/components/ActionConfirmDialog.vue'
import { useActionConfirmation } from '@/shared/ui/composables/useActionConfirmation'
import { useUnsavedChangesGuard } from '@/shared/ui/composables/useUnsavedChangesGuard'
import {
  CaptureCropEditor,
  CaptureWorkspace,
  useCaptureBatchData,
  useCaptureBatchLifecycle,
  useCaptureCropPresentation,
  useCaptureDraftPersistence,
  useCaptureFileImport,
  useCaptureImportWorkflow,
  useCaptureItemEditing,
  useCaptureLanSession,
  useCaptureOrganizerActions,
  useCapturePreviewCache,
  useCaptureQualityAnalysis,
  useCaptureRefreshScheduler,
} from '@/modules/capture'
import { useCaptureRecognitionWorkflow } from '@/modules/ocr'
import {
  createCaptureDevelopmentCropEditor,
  createCaptureDevelopmentPreview,
} from './capture-development-preview'
import {
  commands,
  type CaptureImportBytesInput,
  type SubjectPreferences,
} from '../../shared/api/bindings'
import { normalizeAppResult } from '../../shared/api/normalize-result'
import { syncControllerKey } from '../sync-controller'
import { workspaceTransitionGuardKey } from '../workspace-transition-guard'

const syncController = inject(syncControllerKey, undefined)
const workspaceTransitionGuard = inject(workspaceTransitionGuardKey, undefined)
const appRouter = inject(routerKey, undefined)
const currentRoute = inject(routeLocationKey, undefined)
const desktopAvailable = isTauri()
const busy = ref(false)
const errorMessage = ref('')
const saveState = ref<'idle' | 'saving' | 'saved' | 'error'>('idle')
const commitMessage = ref('')
const subjectPreferences = ref<SubjectPreferences>({
  enabledSubjects: ['语文', '数学', '英语', '政治', '历史', '地理', '物理', '化学', '生物'],
  customSubjects: [],
  captureSoundEnabled: true,
})
const subjectOptions = computed(() => [...new Set([
  ...subjectPreferences.value.enabledSubjects,
  ...subjectPreferences.value.customSubjects,
])])
const {
  batches,
  detail,
  requestedBatchId,
  setDetailRequestedHandler,
  loadBatches,
  loadDetail,
  replaceDetail,
  clearDetail: clearBatchDetail,
  hydrateDevelopment,
} = useCaptureBatchData({
  desktopAvailable,
  list: async () => normalizeAppResult(await commands.captureBatchList()),
  detail: async batchId => normalizeAppResult(await commands.captureBatchDetail(batchId)),
  onError: showError,
  routeBatchId: () => typeof currentRoute?.query.batchId === 'string'
    ? currentRoute.query.batchId
    : undefined,
  replaceRouteBatchId: (batchId) => {
    if (appRouter) void appRouter.replace({
      name: 'inbox',
      query: { ...currentRoute?.query, batchId },
    })
  },
})
const {
  unsaved: draftSaveUnsaved,
  retryAvailable: draftSaveRetryAvailable,
  persistenceBusy: draftPersistenceBusy,
  updateDraft,
  retry: retryDraftSave,
  clear: clearDraftPersistence,
  dispose: disposeDraftPersistence,
} = useCaptureDraftPersistence({
  desktopAvailable,
  activeDetail: () => detail.value,
  isBlocked: () => busy.value,
  update: async input => normalizeAppResult(await commands.captureDraftUpdate(input)),
  refresh: loadDetail,
  onDetailChange: replaceDetail,
  onBusyChange: value => { busy.value = value },
  onSaveStateChange: value => { saveState.value = value },
  onError: showError,
})

const recognitionWorkflow = useCaptureRecognitionWorkflow({
  desktopAvailable: isTauri(),
  requestedBatchId: () => requestedBatchId.value,
  activeDetail: () => detail.value,
  onDetailChange: replaceDetail,
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
setDetailRequestedHandler((batchId) => {
  void loadRecognitionCapability()
  void loadRecognitionStatus(batchId)
  void loadRecognitionLastOperation(batchId)
})
const {
  current: destructiveConfirmation,
  ask: askDestructiveConfirmation,
  confirm: confirmDestructiveAction,
  cancel: cancelDestructiveAction,
} = useActionConfirmation()
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
type DevelopmentCapturePreviewMode = 'capture-batches' | 'capture-card' | 'crop-editor'
const developmentCapturePreviewMode = computed<DevelopmentCapturePreviewMode | undefined>(() => {
  if (!import.meta.env.DEV || desktopAvailable) return undefined
  const mode = currentRoute?.query.preview
  return mode === 'capture-batches' || mode === 'capture-card' || mode === 'crop-editor'
    ? mode
    : undefined
})
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
  onDetailChange: replaceDetail,
  onError: showError,
  onNotice: message => { recognitionNotice.value = message },
  reloadDetail: loadDetail,
  operations: {
    applyLayout: async input => normalizeAppResult(await commands.captureLayoutApply(input)),
    assignSubject: async input => normalizeAppResult(await commands.captureBatchAssignSubject(input)),
    moveItem: async input => normalizeAppResult(await commands.captureItemMove(input)),
    stageRole: async input => normalizeAppResult(await commands.captureItemStageRole(input)),
    mergeCard: async input => normalizeAppResult(await commands.captureCardMerge(input)),
    deleteDraft: async (batchId, expectedRevision, draftId) =>
      normalizeAppResult(await commands.captureDraftDelete(batchId, expectedRevision, draftId)),
    applyPairSuggestions: async input =>
      normalizeAppResult(await commands.capturePairSuggestionsApply(input)),
  },
})
const {
  applyLayout,
  assignBatchSubject,
  moveItem,
  stageItemRole,
  mergeCard,
  deleteDraft,
  applyPairSuggestions,
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
const {
  reports: qualityReports,
  errors: qualityErrors,
  checkingItemId: qualityCheckingItemId,
  dismissedItemIds: qualityDismissedItemIds,
  cropSeed: qualityCropSeed,
  check: checkCaptureQuality,
  dismiss: dismissCaptureQuality,
} = useCaptureQualityAnalysis({
  desktopAvailable,
  activeBatchId: () => detail.value?.batch.id,
  check: async (batchId, itemId) =>
    normalizeAppResult(await commands.captureQualityCheck(batchId, itemId)),
})

function loadDevelopmentCapturePreview(mode: DevelopmentCapturePreviewMode) {
  const preview = createCaptureDevelopmentPreview()
  hydrateDevelopment({
    batches: preview.batches,
    detail: mode === 'capture-batches' ? undefined : preview.detail,
  })
  previewCache.clear()
  Object.assign(previews, preview.previews)
  setDevelopmentCropEditor(
    mode === 'crop-editor' ? createCaptureDevelopmentCropEditor(preview) : undefined,
  )
}

const itemEditing = useCaptureItemEditing({
  desktopAvailable,
  activeDetail: () => detail.value,
  isBlocked: () => busy.value,
  onBusyChange: value => { busy.value = value },
  onSaveStateChange: value => { saveState.value = value },
  onDetailChange: replaceDetail,
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
const {
  visibleCropEditor,
  setDevelopmentCropEditor,
  openVisibleCropEditor,
  closeVisibleCropEditor,
  applyVisibleCrop,
  openRecognitionCropEditor,
  closeRecognitionCropEditor,
  saveRecognitionCropEditor,
  clearPendingFocus: clearCropPresentationFocus,
} = useCaptureCropPresentation({
  activeBatchId: () => detail.value?.batch.id,
  cropEditor,
  recognitionEditorOpen: () => Boolean(recognitionCropEditor.value),
  cropSeed: qualityCropSeed,
  openCrop: openCropEditor,
  closeCrop: closeCropEditor,
  applyCrop,
  editRecognition,
  closeRecognition: closeRecognitionProposal,
  saveRecognition: saveRecognitionProposal,
})

const fileImporter = useCaptureFileImport({
  activeBatchId: () => desktopAvailable ? detail.value?.batch.id : undefined,
  currentItemCount: () => detail.value?.items.length ?? 0,
  isBlocked: () => busy.value,
  onBusyChange: (isBusy) => { busy.value = isBusy },
  importBytes: async input =>
    normalizeAppResult(await commands.captureImportBytes({
      ...input,
      // Tauri 2 serializes Uint8Array directly to Rust Vec<u8>. Specta emits
      // number[] for Vec<u8>, so keep this compatibility cast at the IPC edge.
      bytes: input.bytes as unknown as CaptureImportBytesInput['bytes'],
    })),
  createUploadId: () => crypto.randomUUID(),
  maxBatchItems: 150,
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
  cancelImport,
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

const refreshScheduler = useCaptureRefreshScheduler({
  activeBatchId: () => detail.value?.batch.id,
  refreshDetail: loadDetail,
  refreshList: loadBatches,
  refreshLanStatus: loadLanStatus,
  delayMs: 120,
})

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
  clearBatchDetail()
  commitMessage.value = ''
  clearCropPresentationFocus()
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
    clearDraftPersistence()
    await loadDetail(batchId)
  })()
}

watch(() => detail.value?.batch.id, () => {
  previewCache.clear()
  clearImportProgress()
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
        && requestedBatchId.value !== targetBatchId
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
  const lanStartup = Promise.all([loadLanAddresses(), loadLanPreflight(), loadLanStatus()])
  await Promise.all([loadBatches(), loadSubjectPreferences()])
  const requestedBatchId = currentRoute?.query.batchId
  if (
    typeof requestedBatchId === 'string'
    && batches.value.some(batch => batch.id === requestedBatchId)
  ) {
    await loadDetail(requestedBatchId)
  }
  await lanStartup
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
  clearCropPresentationFocus()
  window.removeEventListener('paste', importFromPaste)
  unlistenBatch?.()
  unlistenRecognition?.()
  refreshScheduler.dispose()
  disposeImportWorkflow()
  previewCache.dispose()
  captureLan.dispose()
  disposeDraftPersistence()
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
    @cancel-import="cancelImport"
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
