<script setup lang="ts">
import {
  ArrowLeft, Check, ChevronRight, ClipboardPaste, FolderOpen,
  Images, ListPlus, LockKeyhole, MoreHorizontal, Plus, QrCode, Save, Smartphone,
  Sparkles, Trash2, Undo2, UploadCloud,
} from '@lucide/vue'
import { computed, nextTick, onBeforeUnmount, ref, watch } from 'vue'
import { useMenuButton } from '../../../app/composables/useMenuButton'
import type {
  CaptureBatchDetail, CaptureBatchSummary, CaptureDraftSummary, CaptureItemSummary,
  CaptureLanAddress, CaptureLanPreflight, CaptureLanSession, CaptureLayoutMode,
  CaptureRecognitionJob, CaptureRecognitionOperationSummary, OcrRecognitionFeatureStatus,
} from '../../../shared/api/bindings'
import CaptureRecognitionEntry from '../../ocr/components/CaptureRecognitionEntry.vue'
import CaptureRecognitionReview from '../../ocr/components/CaptureRecognitionReview.vue'
import ActionConfirmDialog from '../../../app/components/ActionConfirmDialog.vue'
import { useActionConfirmation } from '../../../app/composables/useActionConfirmation'
import CaptureLanDialog from './CaptureLanDialog.vue'
import CaptureLayoutTemplatePanel from './CaptureLayoutTemplatePanel.vue'
import CaptureThumbnail from './CaptureThumbnail.vue'
import CaptureDraftCard from './CaptureDraftCard.vue'
import { useCapturePointerDrag, type CapturePointerDrop } from '../composables/useCapturePointerDrag'
import { useCaptureFeedback, type CaptureFeedbackRole } from '../composables/useCaptureFeedback'
import type { CaptureFileImportProgress } from '../composables/useCaptureFileImport'
import { useCaptureDraftTextEditor } from '../composables/useCaptureDraftTextEditor'
import { useCaptureBatchSubjectDraft } from '../composables/useCaptureBatchSubjectDraft'

type MoveTarget = {
  itemId: string
  targetDraftId: string | null
  targetRole: 'question' | 'answer' | null
  targetPosition: number
}

const props = defineProps<{
  batches: CaptureBatchSummary[]
  detail: CaptureBatchDetail | undefined
  previews: Record<string, string>
  busy: boolean
  errorMessage: string
  desktopAvailable: boolean
  lanAddresses: CaptureLanAddress[]
  lanPreflight: CaptureLanPreflight | undefined
  lanPreflightBusy: boolean
  lanSession: CaptureLanSession | undefined
  saveState: 'idle' | 'saving' | 'saved' | 'error'
  draftSaveRetryAvailable: boolean
  commitMessage: string
  subjectOptions: string[]
  captureSoundEnabled: boolean
  importProgress?: CaptureFileImportProgress | undefined
  recognitionFeature?: OcrRecognitionFeatureStatus | undefined
  recognitionJob?: CaptureRecognitionJob | undefined
  recognitionOperation?: CaptureRecognitionOperationSummary | undefined
  recognitionNotice?: string | undefined
  recognitionBusy?: boolean | undefined
  recognitionOperationBusy?: boolean | undefined
}>()

const emit = defineEmits<{
  createBatch: [subject: string]
  openBatch: [batchId: string]
  back: []
  discardBatch: [batchId: string]
  importSelect: []
  importFiles: [files: File[]]
  finishCollecting: [subject: string]
  assignBatchSubject: [subject: string]
  applyLayout: [mode: CaptureLayoutMode, questions: number, answers: number, splitIndex: number | null]
  moveItem: [target: MoveTarget]
  stageItemRole: [itemId: string, stagedRole: 'question' | 'answer']
  mergeCard: [itemIds: string[], targetDraftId: string | null, newDraftSubject: string | null]
  applyPairSuggestions: [pairIds: string[]]
  deleteDraft: [draftId: string]
  updateDraft: [draft: CaptureDraftSummary, subject: string, tags: string[], note: string]
  retryDraftSave: []
  removeItem: [itemId: string]
  commitReady: []
  preview: [itemId: string]
  crop: [itemId: string]
  revertCrop: [derivationId: string]
  mobileCapture: [selectedAddress: string | null]
  refreshLanAddresses: []
  refreshLanPreflight: []
  stopMobileCapture: []
  recognitionStart: []
  recognitionCancel: [jobId: string]
  recognitionResume: [jobId: string]
  recognitionOpenSetup: []
  recognitionReview: [input: {
    jobId: string
    suggestionId: string
    decision: 'accepted' | 'rejected'
    editedRegions: null
  }]
  recognitionReviewMany: [inputs: Array<{
    jobId: string
    suggestionId: string
    decision: 'accepted'
    editedRegions: null
  }>]
  recognitionEdit: [suggestionId: string]
  recognitionApply: [suggestionIds: string[]]
  recognitionRevert: [operationId: string]
}>()

const newSubject = ref('')
const dropActive = ref(false)
const dropEnabled = computed(() => Boolean(
  props.detail
  && props.detail.batch.state !== 'completed'
  && props.desktopAvailable
  && !props.busy,
))
const dropHint = computed(() => {
  if (props.busy) return '当前操作完成后可继续拖入图片'
  if (!props.desktopAvailable) return '桌面版支持拖入 PNG、JPEG 和 WebP 图片'
  return '拖入一组图片，按文件顺序进入当前批次'
})
const showLanPanel = ref(false)
const lanLauncher = ref<HTMLButtonElement>()
let lanFocusReturn: HTMLElement | null = null
const selectedDraftId = ref('')
const selectedMaterialId = ref('')
const settledDraftId = ref('')
const roleChangedItemId = ref('')
const revealDraftId = ref('')
const {
  current: discardConfirmation,
  ask: askDiscardConfirmation,
  confirm: confirmDiscard,
  cancel: cancelDiscard,
} = useActionConfirmation()
const revealItemId = ref('')
const revealRequestKey = ref(0)
const recognitionReviewOpen = ref(false)
const {
  activeMenuKey: batchMenuId,
  closeMenu: closeBatchMenu,
  getMenuLauncher: getBatchMenuLauncher,
  toggleMenu: toggleBatchMenu,
  handleMenuButtonKeydown: handleBatchMenuButtonKeydown,
  handleMenuKeydown: handleBatchMenuKeydown,
} = useMenuButton()
const recognitionEntry = ref<InstanceType<typeof CaptureRecognitionEntry>>()
let settleTimer: ReturnType<typeof setTimeout> | undefined
let roleChangeTimer: ReturnType<typeof setTimeout> | undefined
let pendingCardDrop: { itemId: string, role: CaptureFeedbackRole, targetDraftId: string | null } | undefined
let pendingRoleChange: { itemId: string, role: CaptureFeedbackRole } | undefined

const {
  collectingSubject: batchSubject,
  pendingSubject: pendingBatchSubject,
  markCollectingDirty,
  selectPendingSubject,
} = useCaptureBatchSubjectDraft(() => props.detail?.batch)
const itemById = computed(() => new Map(props.detail?.items.map(item => [item.id, item]) ?? []))
const activeBatches = computed(() => props.batches.filter(batch => batch.state !== 'completed'))
const completedBatches = computed(() => props.batches.filter(batch => batch.state === 'completed'))
const readyCount = computed(() => props.detail?.drafts.filter(draft => draft.ready).length ?? 0)
const isCollecting = computed(() => props.detail?.batch.state === 'collecting')
const unassignedItems = computed(() => props.detail?.unassignedItemIds
  .map(id => itemById.value.get(id))
  .filter((item): item is CaptureItemSummary => Boolean(item)) ?? [])
const pairedSuggestionItemIds = computed(() => new Set(
  props.detail?.pairSuggestions.flatMap(pair => [
    ...pair.questionItemIds,
    ...pair.answerItemIds,
  ]) ?? [],
))
const looseUnassignedItems = computed(() => unassignedItems.value
  .filter(item => !pairedSuggestionItemIds.value.has(item.id)))
const selectedDraft = computed(() => props.detail?.drafts.find(draft => draft.id === selectedDraftId.value))
const {
  tagsText: draftTags,
  noteText: draftNote,
  markTagsDirty,
  markNoteDirty,
  prepareSave: prepareDraftTextSave,
} = useCaptureDraftTextEditor(selectedDraft)
const selectedMaterial = computed(() => unassignedItems.value.find(item => item.id === selectedMaterialId.value))
const incompleteDraftSummaries = computed(() => props.detail?.drafts
  .map((draft, index) => {
    const missing: string[] = []
    if (!draft.questionItemIds.length) missing.push('缺题面')
    if (!draft.answerItemIds.length) missing.push('缺答案')
    if (!draft.subject.trim()) missing.push('缺科目')
    return missing.length ? `第 ${index + 1} 题：${missing.join('、')}` : ''
  })
  .filter(Boolean) ?? [])
const draggedRole = computed<CaptureFeedbackRole>(() => itemById.value.get(pointerDrag.drag.itemId)?.stagedRole === 'answer' ? 'answer' : 'question')

watch(() => props.detail, (detail) => {
  if (!detail) return
  if (!detail.drafts.some(draft => draft.id === selectedDraftId.value)) {
    selectedDraftId.value = detail.drafts[0]?.id ?? ''
  }
  if (!detail.unassignedItemIds.includes(selectedMaterialId.value)) {
    selectedMaterialId.value = ''
  }
}, { immediate: true })

watch(() => props.subjectOptions, (subjects) => {
  // Keep the explicit "暂不设置科目" choice as the initial state. Only
  // repair a non-empty selection when settings removed that subject.
  if (newSubject.value && !subjects.includes(newSubject.value)) {
    newSubject.value = subjects[0] ?? ''
  }
}, { immediate: true })

watch(() => props.lanSession, (session) => {
  if (session && !showLanPanel.value) void showLanDialog()
})

watch(
  () => [props.recognitionJob?.id, props.recognitionJob?.state],
  ([jobId, state], previous) => {
    if (state === 'review' && (jobId !== previous?.[0] || previous?.[1] !== 'review')) {
      recognitionReviewOpen.value = true
    }
    else if (state !== 'review') {
      recognitionReviewOpen.value = false
    }
  },
  { immediate: true },
)

function createBatch() {
  emit('createBatch', newSubject.value.trim())
  newSubject.value = ''
}

function startMobileCapture(selectedAddress: string | null = null) {
  emit('mobileCapture', selectedAddress || props.lanAddresses[0]?.address || null)
}

async function showLanDialog() {
  lanFocusReturn = document.activeElement instanceof HTMLElement ? document.activeElement : lanLauncher.value ?? null
  showLanPanel.value = true
  await nextTick()
}

async function openLanPanel() {
  await showLanDialog()
  if (!props.lanSession) startMobileCapture()
}

async function closeLanPanel() {
  showLanPanel.value = false
  await nextTick()
  ;(lanFocusReturn?.isConnected ? lanFocusReturn : lanLauncher.value)?.focus()
  lanFocusReturn = null
}

function pairItems(itemIds: string[]) {
  return itemIds
    .map(itemId => itemById.value.get(itemId))
    .filter((item): item is CaptureItemSummary => Boolean(item))
}

function formatPairConfidence(value: number) {
  return `${Math.round(value / 100)}%`
}

function forwardLayoutApply(
  mode: CaptureLayoutMode,
  questions: number,
  answers: number,
  nextSplitIndex: number | null,
) {
  emit('applyLayout', mode, questions, answers, nextSplitIndex)
}

function applyPendingBatchSubject() {
  const subject = pendingBatchSubject.value.trim()
  if (!subject || subject === props.detail?.batch.subject || props.busy) return
  emit('assignBatchSubject', subject)
}

async function requestDiscard(batch: CaptureBatchSummary) {
  const launcher = getBatchMenuLauncher()
  closeBatchMenu()
  if (launcher?.isConnected) launcher.focus()
  const confirmed = await askDiscardConfirmation({
    eyebrow: '采集批次 · 删除确认',
    title: `删除“${batchTitle(batch)}”？`,
    description: '批次中的草稿会被删除；只会清理没有被其他草稿或题库引用的图片。',
    confirmLabel: '删除批次',
    cancelLabel: '保留批次',
    tone: 'danger',
  })
  if (confirmed && props.batches.some(value => value.id === batch.id)) {
    emit('discardBatch', batch.id)
  }
}

function batchTitle(batch: Pick<CaptureBatchSummary, 'subject' | 'updatedAtUtcMs'>) {
  if (batch.subject.trim()) return batch.subject.trim()
  if (batch.updatedAtUtcMs) {
    const date = new Date(batch.updatedAtUtcMs)
    const stamp = new Intl.DateTimeFormat('zh-CN', {
      month: 'numeric',
      day: 'numeric',
      hour: '2-digit',
      minute: '2-digit',
      hour12: false,
    }).format(date)
    return `未命名批次 · ${stamp}`
  }
  return '未命名批次'
}

function importDrop(event: DragEvent) {
  dropActive.value = false
  if (!dropEnabled.value) return
  const files = [...(event.dataTransfer?.files ?? [])]
  if (files.length) emit('importFiles', files)
}

function activateImportDrop() {
  if (dropEnabled.value) dropActive.value = true
}

watch(dropEnabled, (enabled) => {
  if (!enabled) dropActive.value = false
})

function handlePointerDrop(drop: CapturePointerDrop) {
  const item = itemById.value.get(drop.itemId)
  if (!item || props.busy) return
  if (drop.kind === 'unassigned') {
    if (item.draftId) emit('moveItem', { itemId: item.id, targetDraftId: null, targetRole: null, targetPosition: 0 })
    return
  }
  const targetDraftId = drop.kind === 'card' ? drop.draftId : null
  const newDraftSubject = drop.kind === 'new-card'
    ? selectedDraft.value?.subject.trim()
      || props.detail?.drafts.find(draft => draft.subject.trim())?.subject.trim()
      || props.detail?.batch.subject.trim()
      || null
    : null
  pendingCardDrop = { itemId: item.id, role: item.stagedRole === 'answer' ? 'answer' : 'question', targetDraftId }
  emit('mergeCard', [item.id], targetDraftId, newDraftSubject)
}

const pointerDrag = useCapturePointerDrag(handlePointerDrop)
const feedback = useCaptureFeedback(() => props.captureSoundEnabled)

watch(() => props.saveState, (state) => {
  if (state === 'error') {
    pendingCardDrop = undefined
    pendingRoleChange = undefined
    return
  }
  if (state !== 'saved') return

  if (pendingRoleChange) {
    const pending = pendingRoleChange
    pendingRoleChange = undefined
    roleChangedItemId.value = pending.itemId
    if (roleChangeTimer) clearTimeout(roleChangeTimer)
    roleChangeTimer = setTimeout(() => {
      if (roleChangedItemId.value === pending.itemId) roleChangedItemId.value = ''
    }, 280)
    feedback.playDrop(pending.role)
  }

  if (!pendingCardDrop) return
  const pending = pendingCardDrop
  pendingCardDrop = undefined
  const targetDraftId = pending.targetDraftId
    ?? props.detail?.items.find(item => item.id === pending.itemId)?.draftId
    ?? ''
  if (targetDraftId) {
    settledDraftId.value = targetDraftId
    selectedDraftId.value = targetDraftId
    revealDraftId.value = targetDraftId
    revealItemId.value = pending.itemId
    revealRequestKey.value += 1
    if (settleTimer) clearTimeout(settleTimer)
    settleTimer = setTimeout(() => {
      if (settledDraftId.value === targetDraftId) settledDraftId.value = ''
    }, 260)
  }
  feedback.playDrop(pending.role)
})

onBeforeUnmount(() => {
  if (settleTimer) clearTimeout(settleTimer)
  if (roleChangeTimer) clearTimeout(roleChangeTimer)
})

function selectMaterial(item: CaptureItemSummary) {
  if (pointerDrag.consumeSuppressedClick() || props.busy) return
  selectedMaterialId.value = item.id
  emit('preview', item.id)
}

function setSelectedMaterialRole(role: CaptureFeedbackRole) {
  const item = selectedMaterial.value
  if (!item || props.busy || item.stagedRole === role) return
  pendingRoleChange = { itemId: item.id, role }
  emit('stageItemRole', item.id, role)
}

function selectedMaterialSubject() {
  return selectedDraft.value?.subject.trim()
    || props.detail?.drafts.find(draft => draft.subject.trim())?.subject.trim()
    || props.detail?.batch.subject.trim()
    || null
}

function createCardFromSelectedMaterial() {
  const item = selectedMaterial.value
  if (!item || props.busy) return
  pendingCardDrop = {
    itemId: item.id,
    role: item.stagedRole === 'answer' ? 'answer' : 'question',
    targetDraftId: null,
  }
  emit('mergeCard', [item.id], null, selectedMaterialSubject())
}

function addSelectedMaterialToDraft(draft: CaptureDraftSummary) {
  const item = selectedMaterial.value
  if (!item || props.busy) return
  const role = item.stagedRole === 'answer' ? 'answer' : 'question'
  const targetPosition = role === 'answer' ? draft.answerItemIds.length : draft.questionItemIds.length
  pendingCardDrop = { itemId: item.id, role, targetDraftId: draft.id }
  emit('moveItem', {
    itemId: item.id,
    targetDraftId: draft.id,
    targetRole: role,
    targetPosition,
  })
}

async function requestAnswerForDraft(draftId: string) {
  selectedDraftId.value = draftId
  await nextTick()
  document.querySelector<HTMLElement>('.unassigned-strip [data-capture-item-id]')?.focus()
}

function openRecognitionReview(jobId: string) {
  recognitionReviewOpen.value = true
  emit('recognitionResume', jobId)
}

async function closeRecognitionReview() {
  recognitionReviewOpen.value = false
  await nextTick()
  await recognitionEntry.value?.focusPrimaryAction()
}

function saveSelectedDraft() {
  const update = prepareDraftTextSave()
  if (!update) return
  emit(
    'updateDraft',
    update.draft,
    update.subject,
    update.tags,
    update.note,
  )
}

function navigateDraft(direction: 'previous' | 'next') {
  const drafts = props.detail?.drafts ?? []
  const index = drafts.findIndex(draft => draft.id === selectedDraftId.value)
  const nextDraft = drafts[direction === 'previous' ? index - 1 : index + 1]
  if (!nextDraft) return
  selectedDraftId.value = nextDraft.id
  requestAnimationFrame(() => {
    document.querySelector<HTMLElement>(`[data-draft-id="${nextDraft.id}"]`)?.scrollIntoView({ behavior: 'smooth', block: 'center' })
  })
}

function statusLabel(batch: CaptureBatchSummary) {
  if (batch.state === 'collecting') return '采集中'
  if (batch.state === 'organizing') return '待整理'
  return '已完成'
}
</script>

<template>
  <main
    class="capture-next"
    aria-labelledby="capture-title"
  >
    <p
      v-if="errorMessage"
      class="error-banner"
      role="alert"
    >
      {{ errorMessage }}
    </p>

    <template v-if="!detail">
      <header class="inbox-hero">
        <div>
          <p class="eyebrow">
            采集整理 · 本地加密草稿
          </p>
          <h1 id="capture-title">
            采集箱
          </h1>
          <p class="intro">
            先快速收下图片，再在电脑上分题、配答案。中途退出也不会丢。
          </p>
        </div>
        <div class="capacity-note">
          <LockKeyhole
            :size="17"
            aria-hidden="true"
          />
          每批最多 150 张 · 1 GB
        </div>
      </header>

      <section class="new-batch-card">
        <div class="new-batch-copy">
          <span class="round-icon"><ListPlus :size="20" /></span>
          <div><h2>开始一批新采集</h2><p>科目可以先留空，手机或整理时再补。</p></div>
        </div>
        <form @submit.prevent="createBatch">
          <select
            v-model="newSubject"
          >
            <option value="">
              暂不设置科目
            </option>
            <option
              v-for="subject in subjectOptions"
              :key="subject"
              :value="subject"
            >
              {{ subject }}
            </option>
          </select>
          <button
            type="submit"
            :disabled="busy"
          >
            <Plus :size="17" />新建批次
          </button>
        </form>
      </section>

      <section class="batch-section">
        <div class="section-heading">
          <div><p>正在进行</p><h2>未完成批次</h2></div><span>{{ activeBatches.length }} 批</span>
        </div>
        <div
          v-if="activeBatches.length"
          class="batch-grid"
        >
          <article
            v-for="batch in activeBatches"
            :key="batch.id"
            class="batch-card"
          >
            <button
              class="batch-open"
              type="button"
              @click="emit('openBatch', batch.id)"
            >
              <span class="batch-state">{{ statusLabel(batch) }}</span>
              <h3>{{ batchTitle(batch) }}</h3>
              <p>{{ batch.itemCount }} 张图片 · {{ batch.draftCount }} 道草稿</p>
              <strong>{{ batch.readyCount ? `${batch.readyCount} 道可入库` : '继续整理' }} <ChevronRight :size="16" /></strong>
            </button>
            <button
              class="batch-menu-button"
              type="button"
              :aria-label="`${batchTitle(batch)}的更多操作`"
              aria-haspopup="menu"
              :aria-controls="`batch-menu-${batch.id}`"
              :aria-expanded="batchMenuId === batch.id"
              @click.stop="toggleBatchMenu($event, batch.id)"
              @keydown="handleBatchMenuButtonKeydown($event, batch.id)"
            >
              <MoreHorizontal :size="17" />
            </button>
            <div
              v-if="batchMenuId === batch.id"
              :id="`batch-menu-${batch.id}`"
              class="batch-menu"
              role="menu"
              @keydown.stop="handleBatchMenuKeydown"
            >
              <button
                type="button"
                role="menuitem"
                tabindex="-1"
                @click="requestDiscard(batch)"
              >
                <Trash2 :size="15" />
                删除批次…
              </button>
            </div>
          </article>
        </div>
        <div
          v-else
          class="empty-inbox"
        >
          <Images :size="34" /><p>还没有未完成批次。新建一批，先把今天的错题收进来。</p>
        </div>
      </section>

      <details
        v-if="completedBatches.length"
        class="completed-section"
      >
        <summary>已完成批次（{{ completedBatches.length }}）</summary>
        <button
          v-for="batch in completedBatches"
          :key="batch.id"
          type="button"
          @click="emit('openBatch', batch.id)"
        >
          {{ batchTitle(batch) }}
        </button>
      </details>
    </template>

    <template v-else>
      <header class="workbench-header">
        <button
          class="back-button"
          type="button"
          @click="emit('back')"
        >
          <ArrowLeft :size="17" />返回采集箱
        </button>
        <div class="batch-title">
          <p>{{ isCollecting ? '正在采集' : detail.batch.state === 'completed' ? '批次已完成' : '桌面整理台' }}</p>
          <h1 id="capture-title">
            {{ batchTitle(detail.batch) }}
          </h1>
        </div>
        <div class="workbench-stats">
          <span><strong>{{ detail.items.length }}</strong> 张</span><span><strong>{{ detail.drafts.length }}</strong> 题</span><span class="ready"><strong>{{ readyCount }}</strong> 就绪</span>
        </div>
      </header>

      <section
        v-if="detail.batch.state !== 'completed'"
        class="capture-toolbar"
      >
        <div class="capture-toolbar-heading">
          <p>添加素材</p>
          <span>选择一种来源；也可以直接粘贴或拖入图片。</span>
        </div>
        <div class="capture-toolbar-actions">
          <button
            ref="lanLauncher"
            type="button"
            class="primary-tool"
            :disabled="busy || !desktopAvailable || !isCollecting"
            @click="openLanPanel"
          >
            <QrCode :size="18" /><span><strong>{{ lanSession ? '手机采集中' : '手机扫码' }}</strong><small>{{ lanSession ? `已收到 ${lanSession.receivedItemCount} 张` : '同一 Wi‑Fi 连拍' }}</small></span>
          </button>
          <button
            type="button"
            :disabled="busy || !desktopAvailable"
            @click="emit('importSelect')"
          >
            <FolderOpen :size="18" /><span><strong>电脑批量选择</strong><small>PNG · JPEG · WebP</small></span>
          </button>
          <div class="tool-hint">
            <ClipboardPaste :size="17" /><span><strong>Ctrl + V 粘贴</strong><small>也可把图片拖到窗口</small></span>
          </div>
        </div>
        <div
          v-if="importProgress"
          class="import-progress"
          role="status"
          aria-live="polite"
        >
          <span>
            正在导入 {{ importProgress.completed }}/{{ importProgress.total }} 张<template v-if="importProgress.failed">，{{ importProgress.failed }} 张失败</template>
          </span>
          <progress
            :max="importProgress.total"
            :value="importProgress.completed"
          />
        </div>
      </section>

      <CaptureLanDialog
        v-if="showLanPanel"
        :addresses="lanAddresses"
        :preflight="lanPreflight"
        :preflight-busy="lanPreflightBusy"
        :session="lanSession"
        :busy="busy"
        @close="closeLanPanel"
        @start="startMobileCapture"
        @refresh-addresses="emit('refreshLanAddresses')"
        @refresh-preflight="emit('refreshLanPreflight')"
        @stop="emit('stopMobileCapture')"
      />

      <section
        v-if="detail.batch.state !== 'completed'"
        class="external-drop"
        :class="{ 'is-active': dropActive, 'is-disabled': !dropEnabled }"
        :aria-disabled="!dropEnabled"
        @dragenter.prevent="activateImportDrop"
        @dragover.prevent="activateImportDrop"
        @dragleave.self="dropActive = false"
        @drop.prevent="importDrop"
      >
        <UploadCloud :size="20" /><span>{{ dropHint }}</span>
      </section>

      <section
        v-if="isCollecting"
        class="collecting-panel"
      >
        <div class="collecting-copy">
          <Smartphone :size="25" /><div><h2>采集阶段</h2><p>手机和电脑新图片会继续进入末尾；结束采集后才开放自动分组和拖拽。</p></div>
        </div>
        <label>批次科目<select
          v-model="batchSubject"
          @change="markCollectingDirty"
        >
          <option value="">
            暂不设置科目
          </option>
          <option
            v-for="subject in subjectOptions"
            :key="subject"
            :value="subject"
          >
            {{ subject }}
          </option>
        </select></label>
        <button
          type="button"
          :disabled="busy || !detail.items.length"
          @click="emit('finishCollecting', batchSubject.trim())"
        >
          结束采集，开始整理 <ChevronRight :size="17" />
        </button>
        <p
          v-if="lanSession"
          class="lan-live"
        >
          <span />手机会话正在接收 · {{ lanSession.receivedItemCount }} 张
        </p>
      </section>

      <template v-else-if="detail.batch.state === 'organizing'">
        <section
          class="batch-subject-bar"
          aria-label="整批科目"
        >
          <div><p>整批科目</p><strong>{{ detail.batch.subject || '尚未设置' }}</strong><span>先选择科目，再确认应用；单卡仍可覆盖。</span></div>
          <div class="batch-subject-controls">
            <div class="batch-subject-options">
              <button
                v-for="subject in subjectOptions"
                :key="subject"
                type="button"
                :class="{ selected: pendingBatchSubject === subject }"
                :aria-pressed="pendingBatchSubject === subject"
                :disabled="busy"
                @click="selectPendingSubject(subject)"
              >
                {{ subject }}
              </button>
            </div>
            <div class="batch-subject-confirm">
              <span v-if="pendingBatchSubject !== detail.batch.subject">将覆盖当前题卡科目；单题仍可随后修改。</span>
              <button
                type="button"
                :disabled="busy || !pendingBatchSubject || pendingBatchSubject === detail.batch.subject"
                @click="applyPendingBatchSubject"
              >
                应用到整批
              </button>
            </div>
          </div>
        </section>

        <CaptureRecognitionEntry
          v-if="recognitionFeature"
          ref="recognitionEntry"
          :batch-state="detail.batch.state"
          :unassigned-count="unassignedItems.length"
          :feature="recognitionFeature"
          :job="recognitionJob"
          :busy="recognitionBusy"
          @start="emit('recognitionStart')"
          @cancel="emit('recognitionCancel', $event)"
          @resume="openRecognitionReview"
          @open-setup="emit('recognitionOpenSetup')"
        />

        <aside
          v-if="recognitionFeature"
          class="future-recognition-note"
          aria-labelledby="future-recognition-title"
        >
          <div>
            <p class="eyebrow">
              全自动识题 · 未开放
            </p>
            <h2 id="future-recognition-title">
              理解题意并自动判断答案归属
            </h2>
            <p>当前可按题号锚点给出题答配对建议，但不会理解题意、保存 OCR 文本或替你判断答案内容；所有配对都要先确认。</p>
          </div>
          <span class="future-mode-badge">未开放</span>
        </aside>

        <CaptureRecognitionReview
          v-if="recognitionReviewOpen && recognitionJob?.state === 'review'"
          :job="recognitionJob"
          :previews="previews"
          :busy="recognitionBusy"
          :operation-busy="recognitionOperationBusy"
          @review="emit('recognitionReview', $event)"
          @review-many="emit('recognitionReviewMany', $event)"
          @edit="emit('recognitionEdit', $event.id)"
          @preview="emit('preview', $event)"
          @apply-accepted="emit('recognitionApply', $event)"
          @close="closeRecognitionReview"
        />

        <section
          v-if="recognitionNotice || (
            recognitionOperation
            && !recognitionOperation.reverted
            && recognitionOperation.afterRevision === detail.batch.revision
          )"
          class="recognition-result"
          aria-live="polite"
        >
          <div>
            <Check :size="18" />
            <p>
              <strong>智能切图已更新素材牌库</strong>
              <span>{{ recognitionNotice || `已切分 ${recognitionOperation?.createdItemCount ?? 0} 张素材图片。` }}</span>
              <small v-if="recognitionOperation && !recognitionOperation.reverted">
                在继续移动图片、编辑题卡或正式入库前，可以安全撤销本次应用。
              </small>
            </p>
          </div>
          <button
            v-if="recognitionOperation && !recognitionOperation.reverted
              && recognitionOperation.afterRevision === detail.batch.revision"
            type="button"
            :disabled="recognitionBusy"
            @click="emit('recognitionRevert', recognitionOperation.operationId)"
          >
            <Undo2 :size="15" /> 撤销本次智能整理
          </button>
        </section>

        <CaptureLayoutTemplatePanel
          :item-count="detail.items.length"
          :draft-count="detail.drafts.length"
          :affected-note-count="detail.drafts.filter(draft => draft.note.trim() || draft.tags.length).length"
          :busy="busy"
          @apply="forwardLayoutApply"
        />

        <section
          class="organizer-grid"
          :class="{ 'has-no-materials': !unassignedItems.length }"
        >
          <aside
            class="unassigned-strip"
            :class="{
              'is-drop-active': pointerDrag.drag.hoveredTarget?.kind === 'unassigned',
              'is-empty': !unassignedItems.length,
            }"
            data-capture-drop="unassigned"
          >
            <div class="strip-heading">
              <div><p>素材牌库</p><span>{{ pointerDrag.drag.hoveredTarget?.kind === 'unassigned' ? '松开即可把图片移回素材区' : detail.pairSuggestions.length ? '先核对智能配对，再一键生成题卡' : unassignedItems.length ? '先选中图片，再明确设置用途或加入题卡' : '拖回的图片会重新出现在这里' }}</span></div><strong>{{ unassignedItems.length }}</strong>
            </div>
            <section
              v-if="detail.pairSuggestions.length"
              class="pair-suggestion-panel"
              aria-labelledby="pair-suggestion-title"
            >
              <header>
                <div>
                  <p class="eyebrow">
                    本地智能匹配
                  </p>
                  <h3 id="pair-suggestion-title">
                    已找到 {{ detail.pairSuggestions.length }} 组题面与答案
                  </h3>
                  <span>只按题号锚点配对，请快速核对；采用后只生成采集草稿，不会直接入正式题库。</span>
                </div>
                <button
                  type="button"
                  :disabled="busy"
                  @click="emit('applyPairSuggestions', detail.pairSuggestions.map(pair => pair.id))"
                >
                  <Sparkles :size="15" />
                  一键生成 {{ detail.pairSuggestions.length }} 张题卡
                </button>
              </header>
              <article
                v-for="(pair, pairIndex) in detail.pairSuggestions"
                :key="pair.id"
                class="pair-suggestion-card"
              >
                <div class="pair-suggestion-heading">
                  <strong>匹配建议 {{ pairIndex + 1 }}</strong>
                  <span>可信度 {{ formatPairConfidence(pair.confidenceBasisPoints) }}</span>
                </div>
                <div class="pair-suggestion-sides">
                  <section>
                    <p>题面</p>
                    <CaptureThumbnail
                      v-for="item in pairItems(pair.questionItemIds)"
                      :key="item.id"
                      :item="item"
                      :data-url="previews[item.id]"
                      :active="selectedMaterialId === item.id"
                      :disabled="busy"
                      variant="compact"
                      @preview="emit('preview', $event)"
                      @activate="selectMaterial(item)"
                      @pointer-start="pointerDrag.start"
                    />
                  </section>
                  <section class="is-answer">
                    <p>答案</p>
                    <CaptureThumbnail
                      v-for="item in pairItems(pair.answerItemIds)"
                      :key="item.id"
                      :item="item"
                      :data-url="previews[item.id]"
                      :active="selectedMaterialId === item.id"
                      :disabled="busy"
                      variant="compact"
                      @preview="emit('preview', $event)"
                      @activate="selectMaterial(item)"
                      @pointer-start="pointerDrag.start"
                    />
                  </section>
                </div>
                <button
                  type="button"
                  class="pair-card-action"
                  :disabled="busy"
                  @click="emit('applyPairSuggestions', [pair.id])"
                >
                  采用这组
                </button>
              </article>
            </section>
            <TransitionGroup
              v-if="looseUnassignedItems.length"
              name="organizer-move"
              tag="div"
              class="unassigned-gallery"
            >
              <article
                v-for="item in looseUnassignedItems"
                :key="item.id"
                class="unassigned-item"
                :class="[`is-${item.stagedRole}`, { 'is-role-changed': roleChangedItemId === item.id }]"
                :aria-label="`待配对图片：${item.sourceName}`"
              >
                <CaptureThumbnail
                  :item="item"
                  :data-url="previews[item.id]"
                  variant="gallery"
                  :active="selectedMaterialId === item.id"
                  :removable="!item.cropDerivationId"
                  :disabled="busy"
                  :cropable="detail.batch.state === 'organizing'"
                  @preview="emit('preview', $event)"
                  @crop="emit('crop', $event)"
                  @revert-crop="emit('revertCrop', $event)"
                  @remove="emit('removeItem', $event)"
                  @activate="selectMaterial(item)"
                  @pointer-start="pointerDrag.start"
                />
                <div class="role-chip">
                  <span>{{ item.stagedRole === 'question' ? '题面' : '答案' }}</span>
                  <small>{{ selectedMaterialId === item.id ? '已选择' : '单击选择' }} · 可拖到右侧</small>
                </div>
              </article>
            </TransitionGroup>
            <section
              v-if="selectedMaterial"
              class="material-actions"
              aria-label="所选素材操作"
            >
              <p><strong>{{ selectedMaterial.sourceName }}</strong><span>当前标记：{{ selectedMaterial.stagedRole === 'answer' ? '答案' : '题面' }}</span></p>
              <div class="material-role-actions">
                <button
                  type="button"
                  :class="{ active: selectedMaterial.stagedRole === 'question' }"
                  :aria-pressed="selectedMaterial.stagedRole === 'question'"
                  :disabled="busy || selectedMaterial.stagedRole === 'question'"
                  @click="setSelectedMaterialRole('question')"
                >
                  设为题面
                </button>
                <button
                  type="button"
                  :class="{ active: selectedMaterial.stagedRole === 'answer' }"
                  :aria-pressed="selectedMaterial.stagedRole === 'answer'"
                  :disabled="busy || selectedMaterial.stagedRole === 'answer'"
                  @click="setSelectedMaterialRole('answer')"
                >
                  设为答案
                </button>
              </div>
              <button
                type="button"
                class="material-primary-action"
                :disabled="busy"
                @click="createCardFromSelectedMaterial"
              >
                用所选素材新建题卡
              </button>
              <div
                v-if="detail.drafts.length"
                class="material-draft-actions"
              >
                <button
                  v-for="(draft, index) in detail.drafts"
                  :key="draft.id"
                  type="button"
                  :disabled="busy"
                  @click="addSelectedMaterialToDraft(draft)"
                >
                  加入第 {{ index + 1 }} 题
                </button>
              </div>
            </section>
            <p
              v-if="!unassignedItems.length"
              class="strip-empty"
            >
              <Check :size="16" /><strong>素材已全部配对</strong>
            </p>
          </aside>

          <section class="draft-stack">
            <header class="card-stack-heading">
              <div><p>问答卡</p><h2>一题一张卡，翻面核对答案</h2></div>
              <span>拖到已有卡上会融合；拖到下方空白区会直接创建新题。聚焦缩略图后 ←/→ 切换，Shift+←/→ 调整顺序，A/Q 改答案或题面</span>
            </header>
            <TransitionGroup
              name="organizer-move"
              tag="div"
              class="draft-cards"
            >
              <CaptureDraftCard
                v-for="(draft, draftIndex) in detail.drafts"
                :key="draft.id"
                :draft="draft"
                :draft-index="draftIndex"
                :items="detail.items"
                :previews="previews"
                :selected="draft.id === selectedDraftId"
                :busy="busy"
                :subject-options="subjectOptions"
                :drop-role="pointerDrag.drag.hoveredTarget?.kind === 'card' && pointerDrag.drag.hoveredTarget.draftId === draft.id ? draggedRole : null"
                :settled="settledDraftId === draft.id"
                :reveal-item-id="revealDraftId === draft.id ? revealItemId : undefined"
                :reveal-request-key="revealDraftId === draft.id ? revealRequestKey : 0"
                @select="selectedDraftId = $event"
                @navigate-draft="navigateDraft"
                @preview="emit('preview', $event)"
                @crop="emit('crop', $event)"
                @revert-crop="emit('revertCrop', $event)"
                @pointer-start="pointerDrag.start"
                @return-item="itemId => emit('moveItem', { itemId, targetDraftId: null, targetRole: null, targetPosition: 0 })"
                @change-item-role="(itemId, targetRole, targetPosition) => emit('moveItem', { itemId, targetDraftId: draft.id, targetRole, targetPosition })"
                @change-subject="subject => emit('updateDraft', draft, subject, draft.tags, draft.note)"
                @request-answer="requestAnswerForDraft"
              />
            </TransitionGroup>
            <div
              v-if="unassignedItems.length"
              class="new-card-drop"
              :class="{
                'is-drop-question': pointerDrag.drag.hoveredTarget?.kind === 'new-card' && draggedRole === 'question',
                'is-drop-answer': pointerDrag.drag.hoveredTarget?.kind === 'new-card' && draggedRole === 'answer',
              }"
              data-capture-drop="new-card"
            >
              <Plus :size="22" />
              <strong>拖到这里，自动生成一道新题</strong>
              <span>图片会按照左侧标记的“题面 / 答案”进入新卡</span>
            </div>

            <section
              v-if="selectedDraft"
              class="card-inspector"
              aria-label="当前题卡信息"
            >
              <header><div><p>当前题卡</p><h3>补充标签与笔记</h3></div><span>科目已移到卡片顶部 · 修改后自动保存</span></header>
              <div class="inspector-fields">
                <label><span>标签</span><input
                  v-model="draftTags"
                  maxlength="200"
                  placeholder="函数，粗心"
                  @input="markTagsDirty"
                  @change="saveSelectedDraft"
                ></label>
                <label class="inspector-note"><span>笔记</span><textarea
                  v-model="draftNote"
                  maxlength="500"
                  rows="2"
                  placeholder="错因或下次提醒"
                  @input="markNoteDirty"
                  @change="saveSelectedDraft"
                /></label>
              </div>
            </section>
          </section>
        </section>

        <Teleport to="body">
          <div
            v-if="pointerDrag.drag.active"
            class="capture-drag-ghost"
            :class="`is-${draggedRole}`"
            :style="pointerDrag.style.value"
            aria-hidden="true"
          >
            <img
              v-if="previews[pointerDrag.drag.itemId]"
              :src="previews[pointerDrag.drag.itemId]"
              alt=""
            >
            <Images
              v-else
              :size="24"
            />
            <span>{{ itemById.get(pointerDrag.drag.itemId)?.stagedRole === 'answer' ? '答案' : '题面' }}</span>
          </div>
        </Teleport>

        <footer class="commit-dock">
          <div class="commit-summary">
            <p>{{ saveState === 'saving' ? '草稿保存中' : saveState === 'error' ? '草稿保存失败' : '采集草稿已自动保存' }}</p>
            <strong>{{ readyCount ? `${readyCount} 道完整题卡` : '还没有可加入题库的完整题卡' }}</strong>
            <span>{{ commitMessage || (readyCount ? '保存后正式进入题库；未完成卡仍留在采集箱' : '每张卡需要题面、答案和科目') }}</span>
            <ul v-if="incompleteDraftSummaries.length">
              <li
                v-for="summary in incompleteDraftSummaries.slice(0, 3)"
                :key="summary"
              >
                {{ summary }}
              </li>
            </ul>
          </div>
          <div class="commit-actions">
            <button
              v-if="saveState === 'error' && draftSaveRetryAvailable"
              type="button"
              class="retry-draft-button"
              :disabled="busy"
              @click="emit('retryDraftSave')"
            >
              重试保存草稿
            </button>
            <button
              type="button"
              :disabled="busy || readyCount === 0"
              @click="emit('commitReady')"
            >
              <Save :size="18" />保存全部就绪题（{{ readyCount }}）
            </button>
          </div>
        </footer>
      </template>

      <section
        v-else
        class="completed-panel"
      >
        <Check :size="31" /><h2>这个批次已经整理完成</h2><p>历史记录会保留；正式题目已经进入题库。</p><button
          type="button"
          @click="emit('back')"
        >
          返回采集箱
        </button>
      </section>
    </template>
  </main>
  <ActionConfirmDialog
    v-if="discardConfirmation"
    :request="discardConfirmation"
    @cancel="cancelDiscard"
    @confirm="confirmDiscard"
  />
</template>

<style scoped>
.capture-next { max-width: 1240px; min-height: 100vh; margin: 0 auto; padding: 44px 46px 190px; box-sizing: border-box; }
.error-banner { position: sticky; z-index: 20; top: 14px; margin: 0 0 18px; padding: 12px 15px; color: #7f3829; border: 1px solid rgba(185,88,63,.28); border-radius: 11px; background: rgba(255,245,238,.96); box-shadow: var(--shadow-soft); }
.inbox-hero, .workbench-header { display: flex; justify-content: space-between; gap: 28px; align-items: flex-end; }
.eyebrow, .batch-title p { margin: 0 0 8px; color: var(--cinnabar); font-size: 12px; font-weight: 780; letter-spacing: .13em; }
h1 { margin: 0; font-size: clamp(42px,5vw,64px); letter-spacing: -.055em; line-height: 1; }
.intro { max-width: 650px; margin: 15px 0 0; color: var(--ink-muted); font-size: 15px; }
.capacity-note { display: inline-flex; gap: 8px; align-items: center; padding: 10px 14px; color: #50665d; border: 1px solid rgba(33,51,45,.13); border-radius: 999px; background: rgba(255,253,247,.62); font-size: 12px; }
.new-batch-card { display: flex; justify-content: space-between; gap: 28px; align-items: center; margin-top: 38px; padding: 24px 25px; border: 1px solid var(--line); border-radius: 5px 20px 20px; background: rgba(255,253,247,.72); box-shadow: var(--shadow-soft); }
.new-batch-copy { display: flex; gap: 13px; align-items: center; }.round-icon { display: grid; width: 42px; height: 42px; place-items: center; color: var(--paper); border-radius: 50%; background: var(--green-deep); }.new-batch-card h2, .new-batch-card p { margin: 0; }.new-batch-card h2 { font-size: 19px; }.new-batch-card p { margin-top: 4px; color: var(--ink-muted); font-size: 12px; }
.new-batch-card form { display: flex; gap: 9px; }.new-batch-card select { width: 240px; }.new-batch-card button, .capture-toolbar button, .collecting-panel>button, .commit-dock button { display: inline-flex; gap: 8px; align-items: center; justify-content: center; min-height: 44px; padding: 0 17px; color: var(--paper); border: 0; border-radius: 999px; background: var(--green-deep); font-weight: 740; cursor: pointer; }
input, textarea, select { box-sizing: border-box; padding: 10px 12px; color: var(--ink); border: 1px solid var(--line); border-radius: 10px; outline: none; background: rgba(246,241,231,.66); font: inherit; }input,select{min-height:44px}button{min-height:44px}.new-batch-card button:disabled, button:disabled { cursor: not-allowed; opacity: .4; }
.batch-section { margin-top: 42px; }.section-heading { display: flex; justify-content: space-between; align-items: flex-end; }.section-heading p, .section-heading h2 { margin: 0; }.section-heading p { color: var(--cinnabar); font-size: 12px; font-weight: 760; letter-spacing: .12em; }.section-heading h2 { margin-top: 3px; font-size: 24px; }.section-heading>span { color: var(--ink-muted); font-size: 12px; }
.batch-grid { display: grid; grid-template-columns: repeat(3,minmax(0,1fr)); gap: 15px; margin-top: 17px; }.batch-card { position: relative; min-width: 0; border: 1px solid var(--line); border-radius: 4px 18px 18px; background: rgba(255,253,247,.7); box-shadow: var(--shadow-soft); transition: transform var(--motion-standard) var(--ease-standard), box-shadow var(--motion-standard); }.batch-card:hover { transform: translateY(-3px); box-shadow: 0 18px 36px rgba(34,48,43,.11); }.batch-open { width: 100%; padding: 24px; text-align: left; border: 0; background: transparent; cursor: pointer; }.batch-state { color: var(--cinnabar); font-size: 12px; font-weight: 800; letter-spacing: .1em; }.batch-card h3 { margin: 11px 0 6px; padding-right: 56px; font-size: 22px; }.batch-card p { margin: 0; color: var(--ink-muted); font-size: 12px; }.batch-card strong { display: flex; gap: 4px; align-items: center; margin-top: 26px; color: var(--green-deep); font-size: 12px; }.batch-menu-button { position: absolute; z-index: 2; top: 15px; right: 15px; display: grid; width: 44px; height: 44px; place-items: center; color: var(--ink-muted); border: 0; border-radius: 50%; background: transparent; cursor: pointer; }.batch-menu-button:hover { color: var(--green-deep); background: var(--green-soft); }.batch-menu { position: absolute; z-index: 4; top: 61px; right: 15px; min-width: 148px; padding: 6px; border: 1px solid var(--line); border-radius: 12px; background: var(--paper); box-shadow: 0 16px 34px rgba(34,48,43,.16); }.batch-menu button { display: flex; gap: 8px; align-items: center; width: 100%; min-height: 44px; padding: 0 10px; color: var(--cinnabar); border: 0; border-radius: 8px; background: transparent; cursor: pointer; }.batch-menu button:hover { background: rgba(185,88,63,.1); }
.empty-inbox { display: grid; min-height: 210px; margin-top: 17px; place-content: center; justify-items: center; gap: 13px; color: var(--ink-muted); border: 1px dashed rgba(33,51,45,.2); border-radius: 16px; }.empty-inbox p { max-width: 380px; margin: 0; text-align: center; }.completed-section { margin-top: 30px; color: var(--ink-muted); }.completed-section button { margin: 10px 8px 0 0; padding: 8px 12px; color: inherit; border: 1px solid var(--line); border-radius: 9px; background: transparent; }
.workbench-header { align-items: center; }.back-button { display: inline-flex; gap: 7px; align-items: center; min-height:44px; padding: 9px 12px; color: var(--ink-muted); border: 1px solid var(--line); border-radius: 999px; background: rgba(255,253,247,.5); cursor: pointer; }.batch-title { flex: 1; }.batch-title h1 { font-size: clamp(32px,4vw,50px); }.workbench-stats { display: flex; gap: 7px; }.workbench-stats span { display: grid; min-width: 58px; padding: 9px; text-align: center; color: var(--ink-muted); border-radius: 10px; background: rgba(232,221,199,.48); font-size: 12px; }.workbench-stats strong { color: var(--ink); font-family: serif; font-size: 20px; }.workbench-stats .ready strong { color: var(--cinnabar); }
.capture-toolbar { margin-top:31px; padding:15px; border:1px solid var(--line); border-radius:15px; background:rgba(255,253,247,.58); }.capture-toolbar-heading { display:flex; justify-content:space-between; gap:18px; align-items:center; margin-bottom:10px; }.capture-toolbar-heading p,.capture-toolbar-heading span { margin:0; }.capture-toolbar-heading p { font-weight:800; }.capture-toolbar-heading span { color:var(--ink-muted); font-size:12px; }.capture-toolbar-actions { display:grid; grid-template-columns:repeat(2,minmax(0,1fr)) minmax(180px,.75fr); gap:10px; }.capture-toolbar button,.tool-hint { display:flex; gap:11px; align-items:center; min-height:54px; padding:0 17px; color:var(--ink); border:1px solid var(--line); border-radius:13px; background:rgba(255,253,247,.8); text-align:left; }.capture-toolbar button { justify-content:flex-start; cursor:pointer; }.capture-toolbar .primary-tool { color:var(--paper); border-color:var(--green-deep); background:var(--green-deep); }.capture-toolbar button span,.tool-hint span { display:grid; gap:2px; }.capture-toolbar strong,.tool-hint strong { font-size:13px; }.capture-toolbar small,.tool-hint small { opacity:.68; font-size:12px; }
.import-progress { display:grid; grid-template-columns:minmax(0,1fr) minmax(160px,.45fr); gap:12px; align-items:center; margin-top:11px; padding:10px 12px; color:var(--green-deep); border:1px solid rgba(79,128,110,.24); border-radius:11px; background:rgba(225,235,229,.62); font-size:12px; font-weight:740; }.import-progress progress { width:100%; height:8px; accent-color:var(--green-deep); }
.external-drop { display: flex; gap: 9px; align-items: center; justify-content: center; min-height: 44px; margin-top: 10px; color: var(--ink-muted); border: 1px dashed rgba(33,51,45,.24); border-radius: 11px; font-size: 12px; transition: background var(--motion-feedback), border-color var(--motion-feedback); }.external-drop.is-active { color: var(--green-deep); border-color: var(--green-deep); background: var(--green-soft); }.external-drop.is-disabled { opacity: .58; }
.collecting-panel { display: grid; grid-template-columns: minmax(0,1fr) 240px auto; gap: 20px; align-items: end; margin-top: 24px; padding: 26px; border: 1px solid var(--line); border-radius: 5px 20px 20px; background: rgba(255,253,247,.73); box-shadow: var(--shadow-soft); }.collecting-copy { display: flex; gap: 13px; align-items: flex-start; }.collecting-panel h2,.collecting-panel p { margin: 0; }.collecting-panel p { max-width: 550px; margin-top: 5px; color: var(--ink-muted); font-size: 12px; }.collecting-panel label { display: grid; gap: 6px; color: var(--ink-muted); font-size: 12px; font-weight: 720; }.collecting-panel .lan-live { grid-column: 1/-1; display: flex; align-items: center; gap: 8px; color: var(--green-deep); font-weight: 720; }.lan-live span { width: 8px; height: 8px; border-radius: 50%; background: #4f806e; box-shadow: 0 0 0 4px rgba(79,128,110,.14); }
.recognition-result { display: flex; gap: 18px; align-items: center; justify-content: space-between; margin-top: 16px; padding: 15px 18px; border: 1px solid rgba(79,128,110,.28); border-radius: 14px; background: rgba(225,235,229,.72); }.recognition-result>div { display: flex; gap: 11px; align-items: flex-start; color: var(--green-deep); }.recognition-result p,.recognition-result strong,.recognition-result span,.recognition-result small { display: block; margin: 0; }.recognition-result span { margin-top: 2px; color: var(--ink); font-size: 12px; }.recognition-result small { margin-top: 3px; color: var(--ink-muted); }.recognition-result button { display: inline-flex; flex: 0 0 auto; gap: 7px; align-items: center; min-height: 44px; padding: 0 14px; border: 1px solid rgba(33,51,45,.24); border-radius: 999px; background: var(--paper); color: var(--green-deep); font-weight: 740; cursor: pointer; }
.future-recognition-note { display:flex; gap:14px; align-items:center; justify-content:space-between; margin-top:8px; padding:9px 13px; color:var(--ink-muted); border-left:3px solid var(--sand); background:rgba(244,241,232,.42); }.future-recognition-note h2,.future-recognition-note p { margin:0; }.future-recognition-note .eyebrow { color:var(--ink-muted); font-size:12px; font-weight:850; letter-spacing:.08em; }.future-recognition-note h2 { display:none; }.future-recognition-note div>p:last-child { max-width:850px; margin-top:2px; font-size:12px; line-height:1.45; }.future-mode-badge { flex:0 0 auto; padding:5px 9px; color:var(--ink-muted); border:1px solid rgba(33,51,45,.17); border-radius:999px; background:rgba(255,253,247,.68); font-size:12px; font-weight:820; }
.batch-subject-bar { display:grid; grid-template-columns:minmax(220px,.6fr) minmax(0,1.4fr); gap:18px; align-items:center; margin-top:24px; padding:17px 19px; border:1px solid rgba(33,51,45,.16); border-radius:5px 17px 17px; background:linear-gradient(120deg,rgba(225,235,229,.7),rgba(255,253,247,.74)); box-shadow:var(--shadow-soft); }.batch-subject-bar p,.batch-subject-bar strong,.batch-subject-bar span { display:block; margin:0; }.batch-subject-bar p { color:var(--cinnabar); font-size:12px; font-weight:840; letter-spacing:.12em; }.batch-subject-bar strong { margin-top:3px; font-family:var(--font-serif); font-size:21px; }.batch-subject-bar span { margin-top:3px; color:var(--ink-muted); font-size:12px; }.batch-subject-controls { display:grid; gap:8px; }.batch-subject-options { display:flex; gap:7px; flex-wrap:wrap; justify-content:flex-end; }.batch-subject-options button { min-width:52px; min-height:44px; padding:0 12px; color:var(--ink-muted); border:1px solid var(--line); border-radius:999px; background:var(--paper); cursor:pointer; transition:transform var(--motion-feedback),color var(--motion-feedback),background var(--motion-feedback); }.batch-subject-options button:hover { transform:translateY(-1px); }.batch-subject-options button.selected { color:var(--paper); border-color:var(--green-deep); background:var(--green-deep); }.batch-subject-confirm { display:flex; justify-content:flex-end; gap:10px; align-items:center; min-height:44px; }.batch-subject-confirm button { min-height:44px; padding:0 13px; color:var(--paper); border:0; border-radius:999px; background:var(--green-deep); font-weight:760; cursor:pointer; }
.organizer-grid { display:grid; grid-template-columns:minmax(290px,340px) minmax(0,1fr); gap:20px; align-items:start; margin-top:18px; }
.organizer-grid.has-no-materials { grid-template-columns:minmax(170px,210px) minmax(0,1fr); }
.unassigned-strip { position:sticky; top:18px; max-height:calc(100vh - 120px); padding:17px; overflow:auto; border:1px dashed rgba(33,51,45,.22); border-radius:16px; background:rgba(232,221,199,.24); box-shadow:0 12px 32px rgba(34,48,43,.06); transition:transform var(--motion-feedback) var(--ease-standard),border-color var(--motion-feedback),background var(--motion-feedback),box-shadow var(--motion-feedback); }
.unassigned-strip.is-empty { max-height:none; padding:13px; }
.unassigned-strip.is-drop-active { border-color:var(--cinnabar); background:rgba(247,225,216,.72); box-shadow:0 18px 42px rgba(185,88,63,.14); transform:scale(1.012); }
.pair-suggestion-panel{display:grid;gap:10px;margin-top:14px;padding:12px;border:1px solid rgba(36,105,83,.22);border-radius:14px;background:linear-gradient(145deg,rgba(231,244,237,.94),rgba(255,253,247,.9));box-shadow:0 10px 26px rgba(36,105,83,.08)}.pair-suggestion-panel>header{display:grid;gap:10px}.pair-suggestion-panel>header h3{margin:2px 0 3px;font-family:var(--font-serif);font-size:15px}.pair-suggestion-panel>header span{display:block;color:var(--ink-muted);font-size:12px;line-height:1.5}.pair-suggestion-panel>header button,.pair-card-action{display:flex;justify-content:center;gap:6px;align-items:center;min-height:44px;padding:9px 11px;color:var(--paper);border:0;border-radius:10px;background:var(--green-deep);font-size:12px;font-weight:800;cursor:pointer}.pair-suggestion-card{display:grid;gap:9px;padding:10px;border:1px solid rgba(33,51,45,.13);border-radius:12px;background:rgba(255,253,247,.88)}.pair-suggestion-heading{display:flex;justify-content:space-between;gap:8px;align-items:center}.pair-suggestion-heading strong{font-size:12px}.pair-suggestion-heading span{padding:3px 6px;color:#286a56;border-radius:999px;background:rgba(36,105,83,.1);font-size:12px;font-weight:800}.pair-suggestion-sides{display:grid;gap:8px}.pair-suggestion-sides>section{display:grid;gap:6px;padding:7px;border-left:3px solid var(--green-deep);border-radius:8px;background:rgba(225,235,229,.5)}.pair-suggestion-sides>section.is-answer{border-left-color:var(--cinnabar);background:rgba(247,225,216,.5)}.pair-suggestion-sides p{margin:0;font-size:12px;font-weight:850}.pair-card-action{justify-self:end;padding:7px 10px;color:var(--green-deep);border:1px solid rgba(36,105,83,.22);background:transparent}.pair-suggestion-panel button:disabled{cursor:not-allowed;opacity:.56}
.strip-heading { display:flex; justify-content:space-between; align-items:center; }.strip-heading div { display:grid; gap:2px; }.strip-heading p { margin:0; font-weight:780; }.strip-heading span { color:var(--ink-muted); font-size:12px; }.strip-heading strong { color:var(--cinnabar); font-family:serif; font-size:20px; }
.unassigned-gallery { display:grid; gap:12px; margin-top:13px; }.unassigned-item { min-width:0; padding:8px; border:2px solid rgba(33,51,45,.22); border-radius:14px; background:rgba(255,253,247,.72); transition:transform var(--motion-feedback) var(--ease-standard),border-color var(--motion-feedback),background var(--motion-feedback),box-shadow var(--motion-feedback); }.unassigned-item:hover { transform:translateY(-2px); }.unassigned-item.is-question { border-color:rgba(33,51,45,.52); background:rgba(225,235,229,.7); }.unassigned-item.is-answer { border-color:rgba(185,88,63,.58); background:rgba(247,225,216,.68); }.unassigned-item.is-role-changed { animation: capture-role-toggle 280ms var(--ease-standard); }.role-chip { display:flex; justify-content:space-between; gap:8px; align-items:center; margin-top:7px; }.role-chip span { padding:5px 8px; color:var(--paper); border-radius:999px; background:var(--green-deep); font-size:12px; font-weight:850; }.is-answer .role-chip span { background:var(--cinnabar); }.role-chip small { color:var(--ink-muted); font-size:12px; }.strip-empty { display:flex; gap:7px; align-items:center; margin:15px 0 2px; color:#537064; font-size:12px; }
.material-actions { display:grid; gap:9px; margin-top:13px; padding:12px; border:1px solid rgba(33,51,45,.17); border-radius:13px; background:rgba(255,253,247,.88); }
.material-actions p,.material-actions strong,.material-actions span { display:block; margin:0; }
.material-actions p { min-width:0; }
.material-actions p strong { overflow:hidden; font-size:12px; text-overflow:ellipsis; white-space:nowrap; }
.material-actions p span { margin-top:2px; color:var(--ink-muted); font-size:12px; }
.material-role-actions,.material-draft-actions { display:flex; gap:6px; flex-wrap:wrap; }
.material-actions button { min-height:44px; padding:0 10px; color:var(--green-deep); border:1px solid rgba(33,51,45,.18); border-radius:999px; background:var(--paper); font-size:12px; font-weight:740; cursor:pointer; }
.material-role-actions button.active { color:var(--paper); border-color:var(--green-deep); background:var(--green-deep); opacity:1; }
.material-primary-action { width:100%; color:var(--paper)!important; border-color:var(--green-deep)!important; background:var(--green-deep)!important; }
.draft-stack { min-width:0; }.card-stack-heading { display:flex; justify-content:space-between; gap:18px; align-items:end; margin:0 2px 12px; }.card-stack-heading p,.card-stack-heading h2 { margin:0; }.card-stack-heading p { color:var(--cinnabar); font-size:12px; font-weight:800; letter-spacing:.12em; }.card-stack-heading h2 { margin-top:3px; font-size:20px; }.card-stack-heading>span { max-width:240px; color:var(--ink-muted); font-size:12px; text-align:right; }.draft-cards { display:grid; gap:18px; }
.new-card-drop { display:grid; min-height:110px; margin-top:14px; padding:18px; place-content:center; justify-items:center; gap:5px; color:var(--green-deep); border:2px dashed rgba(33,51,45,.3); border-radius:16px; background:rgba(232,221,199,.17); text-align:center; transition:transform var(--motion-feedback),border-color var(--motion-feedback),background var(--motion-feedback); }.new-card-drop strong { font-size:13px; }.new-card-drop span { color:var(--ink-muted); font-size:12px; }.capture-pointer-dragging .new-card-drop { border-color:var(--cinnabar); background:rgba(185,88,63,.08); transform:scale(1.01); }.card-inspector { margin-top:16px; padding:17px; border:1px solid var(--line); border-radius:16px; background:rgba(255,253,247,.72); }.card-inspector header { display:flex; justify-content:space-between; gap:18px; align-items:end; }.card-inspector header p,.card-inspector header h3 { margin:0; }.card-inspector header p { color:var(--cinnabar); font-size:12px; font-weight:850; letter-spacing:.12em; }.card-inspector header h3 { margin-top:3px; font-size:16px; }.card-inspector header>span { color:var(--ink-muted); font-size:12px; }.inspector-fields { display:grid; grid-template-columns:.8fr 1.6fr; gap:9px; margin-top:12px; }.inspector-fields label { display:grid; gap:5px; color:var(--ink-muted); font-size:12px; font-weight:760; }.inspector-fields input,.inspector-fields textarea { width:100%; }.inspector-fields textarea { resize:vertical; }.capture-drag-ghost { position:fixed; z-index:200; top:0; left:0; display:grid; width:112px; height:88px; overflow:hidden; pointer-events:none; place-items:center; border:2px solid var(--cinnabar); border-radius:13px; color:var(--paper); background:var(--green-deep); box-shadow:0 18px 45px rgba(20,28,25,.3); will-change:transform; }.capture-drag-ghost img { width:100%; height:100%; object-fit:cover; opacity:.86; }.capture-drag-ghost span { position:absolute; right:6px; bottom:6px; padding:4px 7px; border-radius:999px; background:rgba(33,51,45,.86); font-size:12px; font-weight:800; }
.capture-pointer-dragging .new-card-drop:not(.is-drop-question):not(.is-drop-answer){border-color:rgba(33,51,45,.36);background:rgba(232,221,199,.22);transform:none}.new-card-drop.is-drop-question{color:var(--green-deep);border-color:rgba(33,51,45,.72);background:rgba(225,235,229,.82);transform:scale(1.012)}.new-card-drop.is-drop-answer{color:var(--cinnabar);border-color:rgba(185,88,63,.72);background:rgba(247,225,216,.82);transform:scale(1.012)}.capture-drag-ghost{transition:opacity var(--motion-feedback) var(--ease-standard);animation:capture-card-lift var(--motion-feedback) var(--ease-standard)}.capture-drag-ghost.is-question{border-color:rgba(33,51,45,.78);background:var(--green-deep)}.capture-drag-ghost.is-answer{border-color:rgba(185,88,63,.9);background:var(--cinnabar)}.capture-drag-ghost.is-answer span{background:rgba(125,55,39,.9)}
@keyframes capture-card-lift{from{opacity:0}}
@keyframes capture-role-toggle{0%{transform:scale(.98);box-shadow:0 0 0 0 var(--role-ring)}45%{transform:scale(1.018);box-shadow:0 0 0 5px var(--role-ring)}100%{transform:scale(1);box-shadow:0 0 0 0 var(--role-ring)}}
.unassigned-item.is-question.is-role-changed{--role-ring:rgba(33,51,45,.18)}.unassigned-item.is-answer.is-role-changed{--role-ring:rgba(185,88,63,.18)}
.organizer-move-move,.organizer-move-enter-active,.organizer-move-leave-active { transition:transform var(--motion-page) var(--ease-standard),opacity var(--motion-standard) var(--ease-standard); }.organizer-move-enter-from,.organizer-move-leave-to { opacity:0; transform:translateY(10px) scale(.985); }.organizer-move-leave-active { position:absolute; }
.commit-dock { position: sticky; z-index: 12; bottom: 14px; display: flex; justify-content: space-between; gap: 20px; align-items: center; margin-top: 22px; padding: 15px 17px; border: 1px solid rgba(33,51,45,.15); border-radius: 15px; background: rgba(246,241,231,.94); box-shadow: 0 16px 45px rgba(34,48,43,.18); backdrop-filter: blur(16px); }.commit-summary { display: grid; grid-template-columns: auto auto; gap: 2px 9px; }.commit-dock p,.commit-dock strong,.commit-dock span { margin: 0; }.commit-dock p { color: var(--cinnabar); font-size: 12px; font-weight: 800; letter-spacing: .1em; }.commit-dock strong { font-size: 16px; }.commit-dock span { grid-column: 1/-1; color: var(--ink-muted); font-size: 12px; }
.commit-dock .commit-actions { display: flex; gap: 9px; align-items: center; }.commit-dock .retry-draft-button { color: var(--green-deep); border: 1px solid rgba(33,51,45,.18); background: rgba(255,253,247,.82); }
.commit-dock ul { grid-column:1/-1; display:flex; gap:6px; flex-wrap:wrap; margin:4px 0 0; padding:0; list-style:none; }.commit-dock li { padding:4px 7px; color:#7b493b; border-radius:999px; background:rgba(246,226,216,.72); font-size:12px; }
.completed-panel { display: grid; min-height: 420px; place-content: center; justify-items: center; text-align: center; }.completed-panel h2 { margin: 14px 0 5px; }.completed-panel p { margin: 0; color: var(--ink-muted); }.completed-panel button { margin-top: 18px; padding: 10px 16px; color: var(--paper); border: 0; border-radius: 999px; background: var(--green-deep); }
@media (max-width: 980px) { .batch-grid { grid-template-columns: repeat(2,minmax(0,1fr)); }.collecting-panel { grid-template-columns: 1fr; }.batch-subject-bar { grid-template-columns: 1fr; }.batch-subject-options { justify-content: flex-start; } }
@media (max-width: 900px) { .organizer-grid { grid-template-columns:1fr; }.unassigned-strip { position:static; max-height:none; }.unassigned-gallery { grid-template-columns:repeat(2,minmax(0,1fr)); } }
@media (max-width: 720px) { .capture-next { padding: 30px 20px 170px; }.inbox-hero,.workbench-header,.new-batch-card { align-items: stretch; flex-direction: column; }.new-batch-card form { grid-template-columns: 1fr; flex-direction: column; }.new-batch-card select { width: 100%; }.batch-grid { grid-template-columns: 1fr; }.capture-toolbar-actions,.import-progress { grid-template-columns:1fr; }.capture-toolbar-heading { align-items:flex-start; flex-direction:column; }.workbench-stats { align-self: stretch; }.workbench-stats span { flex: 1; }.commit-dock { bottom:82px; }.commit-dock,.commit-dock .commit-actions { align-items: stretch; flex-direction: column; }.commit-dock button { width: 100%; } }
@media (max-width: 560px) { .unassigned-gallery { grid-template-columns:1fr; }.card-stack-heading { align-items:start; flex-direction:column; }.card-stack-heading>span { text-align:left; } }
@media (prefers-reduced-motion: reduce) { .batch-card,.external-drop,.batch-subject-options button,.capture-drag-ghost,.new-card-drop,.organizer-move-move,.organizer-move-enter-active,.organizer-move-leave-active,.unassigned-item.is-role-changed { transition: none; animation:none; } }
</style>
