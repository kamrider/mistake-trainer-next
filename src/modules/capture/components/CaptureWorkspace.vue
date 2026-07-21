<script setup lang="ts">
import {
  ArrowLeft, Check, ChevronRight, ClipboardPaste, FolderOpen,
  Images, LayoutGrid, ListPlus, LockKeyhole, Plus, QrCode, Save, Smartphone,
  Sparkles, Trash2, UploadCloud, X,
} from '@lucide/vue'
import { computed, nextTick, onBeforeUnmount, ref, watch } from 'vue'
import type {
  CaptureBatchDetail, CaptureBatchSummary, CaptureDraftSummary, CaptureItemSummary,
  CaptureLanAddress, CaptureLanPreflight, CaptureLanSession, CaptureLayoutMode,
} from '../../../shared/api/bindings'
import CaptureThumbnail from './CaptureThumbnail.vue'
import CaptureDraftCard from './CaptureDraftCard.vue'
import { useCapturePointerDrag, type CapturePointerDrop } from '../composables/useCapturePointerDrag'
import { useCaptureFeedback, type CaptureFeedbackRole } from '../composables/useCaptureFeedback'

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
  commitMessage: string
  subjectOptions: string[]
  captureSoundEnabled: boolean
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
  deleteDraft: [draftId: string]
  updateDraft: [draft: CaptureDraftSummary, subject: string, tags: string[], note: string]
  removeItem: [itemId: string]
  commitReady: []
  preview: [itemId: string]
  mobileCapture: [selectedAddress: string | null]
  refreshLanAddresses: []
  refreshLanPreflight: []
  stopMobileCapture: []
}>()

const newSubject = ref('')
const batchSubject = ref('')
const layoutMode = ref<CaptureLayoutMode>('alternating')
const questionImages = ref(1)
const answerImages = ref(1)
const splitIndex = ref<number | null>(null)
const dropActive = ref(false)
const showLanPanel = ref(false)
const lanLauncher = ref<HTMLButtonElement>()
const lanDialog = ref<HTMLElement>()
const lanClose = ref<HTMLButtonElement>()
let lanFocusReturn: HTMLElement | null = null
const selectedLanAddress = ref('')
const selectedDraftId = ref('')
const draftSubject = ref('')
const draftTags = ref('')
const draftNote = ref('')
const settledDraftId = ref('')
let settleTimer: ReturnType<typeof setTimeout> | undefined
let pendingCardDrop: { itemId: string, role: CaptureFeedbackRole, targetDraftId: string | null } | undefined

const itemById = computed(() => new Map(props.detail?.items.map(item => [item.id, item]) ?? []))
const activeBatches = computed(() => props.batches.filter(batch => batch.state !== 'completed'))
const completedBatches = computed(() => props.batches.filter(batch => batch.state === 'completed'))
const readyCount = computed(() => props.detail?.drafts.filter(draft => draft.ready).length ?? 0)
const isCollecting = computed(() => props.detail?.batch.state === 'collecting')
const unassignedItems = computed(() => props.detail?.unassignedItemIds
  .map(id => itemById.value.get(id))
  .filter((item): item is CaptureItemSummary => Boolean(item)) ?? [])
const selectedDraft = computed(() => props.detail?.drafts.find(draft => draft.id === selectedDraftId.value))
const lanMinutesRemaining = computed(() => props.lanSession
  ? Math.max(0, Math.ceil(((props.lanSession.expiresAtUtcMs ?? Date.now()) - Date.now()) / 60_000))
  : 0)
const lanNeedsRepair = computed(() => props.lanPreflight?.needsFirewallRepair === true)
const lanReady = computed(() => props.lanPreflight?.canStart === true)
const draggedRole = computed<CaptureFeedbackRole>(() => itemById.value.get(pointerDrag.drag.itemId)?.stagedRole === 'answer' ? 'answer' : 'question')

watch(() => props.detail, (detail) => {
  if (!detail) return
  batchSubject.value = detail.batch.subject
  splitIndex.value = Math.ceil(detail.items.length / 2)
  if (!detail.drafts.some(draft => draft.id === selectedDraftId.value)) {
    selectedDraftId.value = detail.drafts[0]?.id ?? ''
  }
}, { immediate: true })

watch(() => props.subjectOptions, (subjects) => {
  if (!newSubject.value || !subjects.includes(newSubject.value)) {
    newSubject.value = subjects[0] ?? ''
  }
}, { immediate: true })

watch(selectedDraft, (draft) => {
  draftSubject.value = draft?.subject ?? ''
  draftTags.value = draft?.tags.join('，') ?? ''
  draftNote.value = draft?.note ?? ''
}, { immediate: true })

watch(() => props.lanAddresses, (addresses) => {
  if (!addresses.some(address => address.address === selectedLanAddress.value)) {
    selectedLanAddress.value = addresses[0]?.address ?? ''
  }
}, { immediate: true })

watch(() => props.lanSession, (session) => {
  if (session && !showLanPanel.value) void showLanDialog()
})

watch(
  () => [
    props.lanSession?.sessionId,
    props.lanPreflight?.needsFirewallRepair,
    props.lanPreflight?.canStart,
  ],
  async () => {
    if (!showLanPanel.value) return
    await nextTick()
    const activeElement = document.activeElement
    if (!(activeElement instanceof Node) || !lanDialog.value?.contains(activeElement)) {
      lanClose.value?.focus()
    }
  },
  { flush: 'post' },
)

function createBatch() {
  emit('createBatch', newSubject.value.trim())
  newSubject.value = ''
}

function startMobileCapture() {
  const address = selectedLanAddress.value || props.lanAddresses[0]?.address || null
  emit('mobileCapture', address)
}

async function showLanDialog() {
  lanFocusReturn = document.activeElement instanceof HTMLElement ? document.activeElement : lanLauncher.value ?? null
  showLanPanel.value = true
  await nextTick()
  lanClose.value?.focus()
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

function handleLanDialogKeydown(event: KeyboardEvent) {
  if (event.key === 'Escape') {
    event.preventDefault()
    void closeLanPanel()
    return
  }
  if (event.key !== 'Tab' || !lanDialog.value) return
  const focusable = [...lanDialog.value.querySelectorAll<HTMLElement>(
    'button:not([disabled]), select:not([disabled]), input:not([disabled]), summary, [href], [tabindex]:not([tabindex="-1"])',
  )].filter(element => !element.hasAttribute('hidden'))
  if (!focusable.length) return
  const first = focusable[0]!
  const last = focusable[focusable.length - 1]!
  if (event.shiftKey && document.activeElement === first) {
    event.preventDefault()
    last.focus()
  }
  else if (!event.shiftKey && document.activeElement === last) {
    event.preventDefault()
    first.focus()
  }
}

function formatLanBytes(value: number) {
  if (value < 1024 * 1024) return `${Math.round(value / 1024)} KB`
  return `${(value / (1024 * 1024)).toFixed(1)} MB`
}

function requestLayout() {
  if (props.detail?.drafts.length && !window.confirm('重新应用模板会清空当前分组与逐题笔记，但不会删除任何图片。继续吗？')) return
  emit('applyLayout', layoutMode.value, questionImages.value, answerImages.value, layoutMode.value === 'split' ? splitIndex.value : null)
}

function requestDiscard(batch: CaptureBatchSummary) {
  if (window.confirm(`删除“${batch.subject || '未命名批次'}”？只会清理未被其他草稿或题库引用的图片。`)) {
    emit('discardBatch', batch.id)
  }
}

function importDrop(event: DragEvent) {
  dropActive.value = false
  const files = [...(event.dataTransfer?.files ?? [])].filter(file => file.type.startsWith('image/'))
  if (files.length) emit('importFiles', files)
}

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
    return
  }
  if (state !== 'saved' || !pendingCardDrop) return
  const pending = pendingCardDrop
  pendingCardDrop = undefined
  const targetDraftId = pending.targetDraftId
    ?? props.detail?.items.find(item => item.id === pending.itemId)?.draftId
    ?? ''
  if (targetDraftId) {
    settledDraftId.value = targetDraftId
    if (settleTimer) clearTimeout(settleTimer)
    settleTimer = setTimeout(() => {
      if (settledDraftId.value === targetDraftId) settledDraftId.value = ''
    }, 260)
  }
  feedback.playDrop(pending.role)
})

onBeforeUnmount(() => {
  if (settleTimer) clearTimeout(settleTimer)
})

function toggleItemRole(item: CaptureItemSummary) {
  if (pointerDrag.consumeSuppressedClick() || props.busy) return
  emit('stageItemRole', item.id, item.stagedRole === 'question' ? 'answer' : 'question')
}

function saveSelectedDraft() {
  const draft = selectedDraft.value
  if (!draft || props.busy) return
  emit(
    'updateDraft',
    draft,
    draftSubject.value.trim(),
    draftTags.value.split(/[，,]/).map(tag => tag.trim()).filter(Boolean),
    draftNote.value.trim(),
  )
}

function requestDeleteDraft(draftId: string) {
  if (props.busy) return
  if (!window.confirm('撤销这张题卡？卡内所有图片都会回到左侧素材牌库，不会删除原图。')) return
  emit('deleteDraft', draftId)
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
              <h3>{{ batch.subject || '未命名批次' }}</h3>
              <p>{{ batch.itemCount }} 张图片 · {{ batch.draftCount }} 道草稿</p>
              <strong>{{ batch.readyCount ? `${batch.readyCount} 道可入库` : '继续整理' }} <ChevronRight :size="16" /></strong>
            </button>
            <button
              class="batch-delete"
              type="button"
              :aria-label="`删除 ${batch.subject || '未命名批次'}`"
              @click="requestDiscard(batch)"
            >
              <Trash2 :size="15" />
            </button>
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
          {{ batch.subject || '未命名批次' }}
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
            {{ detail.batch.subject || '未命名批次' }}
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
      </section>

      <div
        v-if="showLanPanel"
        class="lan-overlay"
        role="presentation"
        @click.self="closeLanPanel"
      >
        <section
          ref="lanDialog"
          class="lan-dialog"
          role="dialog"
          aria-modal="true"
          aria-labelledby="lan-dialog-title"
          @keydown="handleLanDialogKeydown"
        >
          <button
            ref="lanClose"
            class="lan-close"
            type="button"
            aria-label="关闭"
            @click="closeLanPanel"
          >
            <X :size="18" />
          </button>
          <template v-if="lanSession">
            <p class="eyebrow">
              手机局域网采集
            </p>
            <h2 id="lan-dialog-title">
              扫码开始连拍
            </h2>
            <p class="lan-intro">
              手机与电脑需要在同一个家庭 Wi‑Fi 或个人热点中。
            </p>
            <div class="lan-session-grid">
              <div class="qr-paper">
                <img
                  :src="lanSession.qrSvgDataUrl"
                  alt="手机采集二维码"
                >
              </div>
              <div class="lan-session-copy">
                <div><span>网络地址</span><strong>{{ lanSession.selectedAddress }}</strong></div>
                <div><span>电脑已收到</span><strong>{{ lanSession.receivedItemCount }} 张 · {{ formatLanBytes(lanSession.receivedBytes ?? 0) }}</strong></div>
                <div><span>自动过期</span><strong>约 {{ lanMinutesRemaining }} 分钟后</strong></div>
                <p>上传时请保持 Windows 应用运行。结束或退出应用会立即关闭端口。</p>
                <button
                  class="lan-stop"
                  type="button"
                  :disabled="busy"
                  @click="emit('stopMobileCapture')"
                >
                  停止手机采集
                </button>
              </div>
            </div>
          </template>
          <template v-else>
            <p class="eyebrow">
              手机局域网采集
            </p>
            <div
              class="lan-preflight"
              aria-live="polite"
            >
              <template v-if="busy || lanPreflightBusy">
                <h2 id="lan-dialog-title">
                  正在准备手机连接
                </h2>
                <p class="lan-intro">
                  首次使用时 Windows 会请求一次管理员授权。请在系统弹窗中确认应用是 Mistake Trainer Next，然后点击“是”。
                </p>
                <div class="lan-progress">
                  <span />检查持久权限并启动二维码
                </div>
              </template>

              <template v-else-if="!lanPreflight">
                <h2 id="lan-dialog-title">
                  暂时没有读到连接状态
                </h2>
                <p class="lan-intro">
                  应用没有修改任何系统设置。重新检测后仍可继续。
                </p>
                <button
                  class="lan-secondary"
                  type="button"
                  @click="emit('refreshLanPreflight')"
                >
                  重新检测
                </button>
              </template>

              <template v-else-if="!lanPreflight.supported">
                <h2 id="lan-dialog-title">
                  当前系统暂不支持自动修复
                </h2>
                <p class="lan-intro">
                  手机扫码采集的权限引导目前只支持 Windows 桌面版。
                </p>
              </template>

              <template v-else-if="lanNeedsRepair">
                <span class="lan-state is-attention">Windows 授权尚未完成</span>
                <h2 id="lan-dialog-title">
                  下次扫码会再次请求授权
                </h2>
                <p class="lan-intro">
                  点击下面的按钮会重新弹出 Windows 管理员确认。授权成功后规则会保留，以后点击“手机扫码”会直接生成二维码。
                </p>
                <div class="lan-permission-summary">
                  <LockKeyhole :size="18" /><p><strong>授权范围</strong><span>当前应用 · TCP 入站 · 本地子网；不关闭防火墙，不改动其他软件。</span></p>
                </div>
                <div class="lan-actions">
                  <button
                    class="lan-primary"
                    type="button"
                    :disabled="busy"
                    @click="startMobileCapture"
                  >
                    再次授权并生成二维码
                  </button>
                  <button
                    class="lan-secondary"
                    type="button"
                    :disabled="busy"
                    @click="emit('refreshLanPreflight')"
                  >
                    重新检测
                  </button>
                </div>
                <details class="lan-troubleshooting">
                  <summary>没有弹窗，或者我没有管理员权限</summary>
                  <ul>
                    <li>如果弹窗要求管理员密码，请输入这台电脑管理员账户的密码；不知道密码时请让电脑管理员协助。</li>
                    <li>如果点了“否”或关闭弹窗，不会记录为成功；下一次扫码会再次请求。</li>
                    <li>授权不会关闭 Windows 防火墙，也不会修改其他软件的网络规则。</li>
                    <li>本规则覆盖 Windows 网络类型，但只允许当前程序接收来自本地子网的 TCP 连接。</li>
                  </ul>
                </details>
              </template>

              <template v-else-if="lanReady">
                <span class="lan-state is-ready">持久连接权限已就绪</span>
                <h2 id="lan-dialog-title">
                  选择手机所在的网络
                </h2>
                <p class="lan-intro">
                  权限只允许本地子网访问当前应用。仍建议在家庭 Wi‑Fi 或个人热点中使用，不要在陌生公共网络上传题目。
                </p>
                <label class="lan-address-label">网络接口
                  <select
                    v-model="selectedLanAddress"
                    :disabled="busy || !lanAddresses.length"
                  >
                    <option
                      v-for="address in lanAddresses"
                      :key="address.address"
                      :value="address.address"
                    >{{ address.label }} · {{ address.address }}</option>
                  </select>
                </label>
                <p
                  v-if="!lanAddresses.length"
                  class="lan-empty"
                >
                  没有检测到家庭网络地址，请先连接 Wi‑Fi 或个人热点。
                </p>
                <button
                  v-if="!lanAddresses.length"
                  class="lan-secondary"
                  type="button"
                  :disabled="busy"
                  @click="emit('refreshLanAddresses')"
                >
                  重新检测网络
                </button>
                <button
                  class="lan-start"
                  type="button"
                  :disabled="busy || !lanAddresses.length"
                  @click="startMobileCapture"
                >
                  <QrCode :size="17" /> 生成二维码
                </button>
              </template>

              <template v-else>
                <h2 id="lan-dialog-title">
                  连接权限状态异常
                </h2>
                <p class="lan-intro">
                  二维码尚未生成，应用也没有开放端口。请重新检测后再试。
                </p>
                <button
                  class="lan-secondary"
                  type="button"
                  @click="emit('refreshLanPreflight')"
                >
                  重新检测
                </button>
              </template>
            </div>
          </template>
        </section>
      </div>

      <section
        v-if="detail.batch.state !== 'completed'"
        class="external-drop"
        :class="{ 'is-active': dropActive }"
        @dragenter.prevent="dropActive = true"
        @dragover.prevent="dropActive = true"
        @dragleave.self="dropActive = false"
        @drop.prevent="importDrop"
      >
        <UploadCloud :size="20" /><span>拖入一组图片，按文件顺序进入当前批次</span>
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
          <div><p>整批科目</p><strong>{{ detail.batch.subject || '尚未设置' }}</strong><span>选择一次，当前批次所有题卡立即统一；单卡仍可覆盖。</span></div>
          <div class="batch-subject-options">
            <button
              v-for="subject in subjectOptions"
              :key="subject"
              type="button"
              :class="{ selected: detail.batch.subject === subject }"
              :disabled="busy"
              @click="emit('assignBatchSubject', subject)"
            >
              {{ subject }}
            </button>
          </div>
        </section>

        <section class="layout-bar">
          <div class="layout-heading">
            <Sparkles :size="19" /><div><h2>顺序模板</h2><p>模板只按图片顺序分组，可撤销重做，不识别图片内容。</p></div>
          </div>
          <select
            v-model="layoutMode"
            aria-label="整理模板"
          >
            <option value="alternating">
              题1、答1、题2、答2
            </option>
            <option value="split">
              前半题图、后半答案
            </option>
            <option value="questions_only">
              每张图是一道题
            </option>
            <option value="manual">
              全部手工整理
            </option>
          </select>
          <label v-if="layoutMode === 'alternating'">题图/题<input
            v-model.number="questionImages"
            type="number"
            min="1"
            max="10"
          ></label>
          <label v-if="layoutMode === 'alternating'">答案/题<input
            v-model.number="answerImages"
            type="number"
            min="1"
            max="10"
          ></label>
          <label v-if="layoutMode === 'split'">从第几张分开<input
            v-model.number="splitIndex"
            type="number"
            min="0"
            :max="detail.items.length"
          ></label>
          <button
            type="button"
            :disabled="busy || !detail.items.length"
            @click="requestLayout"
          >
            <LayoutGrid :size="16" />应用模板
          </button>
        </section>

        <section class="organizer-grid">
          <aside
            class="unassigned-strip"
            data-capture-drop="unassigned"
          >
            <div class="strip-heading">
              <div><p>素材牌库</p><span>单击切换题面 / 答案，拖到右侧完成融合</span></div><strong>{{ unassignedItems.length }}</strong>
            </div>
            <TransitionGroup
              v-if="unassignedItems.length"
              name="organizer-move"
              tag="div"
              class="unassigned-gallery"
            >
              <article
                v-for="item in unassignedItems"
                :key="item.id"
                class="unassigned-item"
                :class="`is-${item.stagedRole}`"
                :aria-label="`待配对图片：${item.sourceName}`"
              >
                <CaptureThumbnail
                  :item="item"
                  :data-url="previews[item.id]"
                  variant="gallery"
                  removable
                  :disabled="busy"
                  @preview="emit('preview', $event)"
                  @remove="emit('removeItem', $event)"
                  @activate="toggleItemRole(item)"
                  @pointer-start="pointerDrag.start"
                />
                <div class="role-chip">
                  <span>{{ item.stagedRole === 'question' ? '题面' : '答案' }}</span>
                  <small>单击切换 · 拖到右侧</small>
                </div>
              </article>
            </TransitionGroup>
            <p
              v-else
              class="strip-empty"
            >
              <Check :size="16" />所有图片都已配对
            </p>
          </aside>

          <section class="draft-stack">
            <header class="card-stack-heading">
              <div><p>问答卡</p><h2>一题一张卡，翻面核对答案</h2></div>
              <span>拖到已有卡上会融合；拖到下方空白区会直接创建新题</span>
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
                @select="selectedDraftId = $event"
                @preview="emit('preview', $event)"
                @pointer-start="pointerDrag.start"
                @return-item="itemId => emit('moveItem', { itemId, targetDraftId: null, targetRole: null, targetPosition: 0 })"
                @change-item-role="(itemId, targetRole, targetPosition) => emit('moveItem', { itemId, targetDraftId: draft.id, targetRole, targetPosition })"
                @delete-draft="requestDeleteDraft"
                @change-subject="subject => emit('updateDraft', draft, subject, draft.tags, draft.note)"
              />
            </TransitionGroup>
            <div
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
                  @change="saveSelectedDraft"
                ></label>
                <label class="inspector-note"><span>笔记</span><textarea
                  v-model="draftNote"
                  maxlength="500"
                  rows="2"
                  placeholder="错因或下次提醒"
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
          <div><p>{{ saveState === 'saving' ? '保存中' : saveState === 'error' ? '保存失败' : '已自动保存' }}</p><strong>{{ readyCount ? `${readyCount} 道完整题卡` : '还没有可加入题库的完整题卡' }}</strong><span>{{ commitMessage || (readyCount ? '点击后正式加入题库；未完成卡仍留在采集箱' : '每张卡需要题面、答案和科目') }}</span></div>
          <button
            type="button"
            :disabled="busy || readyCount === 0"
            @click="emit('commitReady')"
          >
            <Save :size="18" />将 {{ readyCount }} 道题加入题库
          </button>
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
</template>

<style scoped>
.capture-next { max-width: 1240px; min-height: 100vh; margin: 0 auto; padding: 44px 46px 120px; box-sizing: border-box; }
.error-banner { position: sticky; z-index: 20; top: 14px; margin: 0 0 18px; padding: 12px 15px; color: #7f3829; border: 1px solid rgba(185,88,63,.28); border-radius: 11px; background: rgba(255,245,238,.96); box-shadow: var(--shadow-soft); }
.inbox-hero, .workbench-header { display: flex; justify-content: space-between; gap: 28px; align-items: flex-end; }
.eyebrow, .batch-title p { margin: 0 0 8px; color: var(--cinnabar); font-size: 12px; font-weight: 780; letter-spacing: .13em; }
h1 { margin: 0; font-size: clamp(42px,5vw,64px); letter-spacing: -.055em; line-height: 1; }
.intro { max-width: 650px; margin: 15px 0 0; color: var(--ink-muted); font-size: 15px; }
.capacity-note { display: inline-flex; gap: 8px; align-items: center; padding: 10px 14px; color: #50665d; border: 1px solid rgba(33,51,45,.13); border-radius: 999px; background: rgba(255,253,247,.62); font-size: 12px; }
.new-batch-card { display: flex; justify-content: space-between; gap: 28px; align-items: center; margin-top: 38px; padding: 24px 25px; border: 1px solid var(--line); border-radius: 5px 20px 20px; background: rgba(255,253,247,.72); box-shadow: var(--shadow-soft); }
.new-batch-copy { display: flex; gap: 13px; align-items: center; }.round-icon { display: grid; width: 42px; height: 42px; place-items: center; color: var(--paper); border-radius: 50%; background: var(--green-deep); }.new-batch-card h2, .new-batch-card p { margin: 0; }.new-batch-card h2 { font-size: 19px; }.new-batch-card p { margin-top: 4px; color: var(--ink-muted); font-size: 12px; }
.new-batch-card form { display: flex; gap: 9px; }.new-batch-card select { width: 240px; }.new-batch-card button, .capture-toolbar button, .collecting-panel>button, .layout-bar>button, .commit-dock button { display: inline-flex; gap: 8px; align-items: center; justify-content: center; min-height: 43px; padding: 0 17px; color: var(--paper); border: 0; border-radius: 999px; background: var(--green-deep); font-weight: 740; cursor: pointer; }
input, textarea, select { box-sizing: border-box; padding: 10px 12px; color: var(--ink); border: 1px solid var(--line); border-radius: 10px; outline: none; background: rgba(246,241,231,.66); font: inherit; }.new-batch-card button:disabled, button:disabled { cursor: not-allowed; opacity: .4; }
.batch-section { margin-top: 42px; }.section-heading { display: flex; justify-content: space-between; align-items: flex-end; }.section-heading p, .section-heading h2 { margin: 0; }.section-heading p { color: var(--cinnabar); font-size: 11px; font-weight: 760; letter-spacing: .12em; }.section-heading h2 { margin-top: 3px; font-size: 24px; }.section-heading>span { color: var(--ink-muted); font-size: 12px; }
.batch-grid { display: grid; grid-template-columns: repeat(3,minmax(0,1fr)); gap: 15px; margin-top: 17px; }.batch-card { position: relative; min-width: 0; border: 1px solid var(--line); border-radius: 4px 18px 18px; background: rgba(255,253,247,.7); box-shadow: var(--shadow-soft); transition: transform var(--motion-standard) var(--ease-standard), box-shadow var(--motion-standard); }.batch-card:hover { transform: translateY(-3px); box-shadow: 0 18px 36px rgba(34,48,43,.11); }.batch-open { width: 100%; padding: 24px; text-align: left; border: 0; background: transparent; cursor: pointer; }.batch-state { color: var(--cinnabar); font-size: 10px; font-weight: 800; letter-spacing: .1em; }.batch-card h3 { margin: 11px 0 6px; font-size: 22px; }.batch-card p { margin: 0; color: var(--ink-muted); font-size: 12px; }.batch-card strong { display: flex; gap: 4px; align-items: center; margin-top: 26px; color: var(--green-deep); font-size: 12px; }.batch-delete { position: absolute; top: 15px; right: 15px; display: grid; width: 32px; height: 32px; place-items: center; color: var(--ink-muted); border: 0; border-radius: 50%; background: transparent; cursor: pointer; }.batch-delete:hover { color: var(--cinnabar); background: rgba(185,88,63,.1); }
.empty-inbox { display: grid; min-height: 210px; margin-top: 17px; place-content: center; justify-items: center; gap: 13px; color: var(--ink-muted); border: 1px dashed rgba(33,51,45,.2); border-radius: 16px; }.empty-inbox p { max-width: 380px; margin: 0; text-align: center; }.completed-section { margin-top: 30px; color: var(--ink-muted); }.completed-section button { margin: 10px 8px 0 0; padding: 8px 12px; color: inherit; border: 1px solid var(--line); border-radius: 9px; background: transparent; }
.workbench-header { align-items: center; }.back-button { display: inline-flex; gap: 7px; align-items: center; padding: 9px 12px; color: var(--ink-muted); border: 1px solid var(--line); border-radius: 999px; background: rgba(255,253,247,.5); cursor: pointer; }.batch-title { flex: 1; }.batch-title h1 { font-size: clamp(32px,4vw,50px); }.workbench-stats { display: flex; gap: 7px; }.workbench-stats span { display: grid; min-width: 58px; padding: 9px; text-align: center; color: var(--ink-muted); border-radius: 10px; background: rgba(232,221,199,.48); font-size: 10px; }.workbench-stats strong { color: var(--ink); font-family: serif; font-size: 20px; }.workbench-stats .ready strong { color: var(--cinnabar); }
.capture-toolbar { display: grid; grid-template-columns: repeat(2,minmax(0,1fr)) minmax(180px,.75fr); gap: 10px; margin-top: 31px; }.capture-toolbar button, .tool-hint { display: flex; gap: 11px; align-items: center; min-height: 60px; padding: 0 17px; color: var(--ink); border: 1px solid var(--line); border-radius: 13px; background: rgba(255,253,247,.67); text-align: left; }.capture-toolbar button { justify-content: flex-start; cursor: pointer; }.capture-toolbar .primary-tool { color: var(--paper); border-color: var(--green-deep); background: var(--green-deep); }.capture-toolbar span, .tool-hint span { display: grid; gap: 2px; }.capture-toolbar strong, .tool-hint strong { font-size: 13px; }.capture-toolbar small, .tool-hint small { opacity: .68; font-size: 10px; }
.external-drop { display: flex; gap: 9px; align-items: center; justify-content: center; min-height: 44px; margin-top: 10px; color: var(--ink-muted); border: 1px dashed rgba(33,51,45,.24); border-radius: 11px; font-size: 11px; transition: background var(--motion-feedback), border-color var(--motion-feedback); }.external-drop.is-active { color: var(--green-deep); border-color: var(--green-deep); background: var(--green-soft); }
.collecting-panel { display: grid; grid-template-columns: minmax(0,1fr) 240px auto; gap: 20px; align-items: end; margin-top: 24px; padding: 26px; border: 1px solid var(--line); border-radius: 5px 20px 20px; background: rgba(255,253,247,.73); box-shadow: var(--shadow-soft); }.collecting-copy { display: flex; gap: 13px; align-items: flex-start; }.collecting-panel h2,.collecting-panel p { margin: 0; }.collecting-panel p { max-width: 550px; margin-top: 5px; color: var(--ink-muted); font-size: 12px; }.collecting-panel label,.layout-bar label { display: grid; gap: 6px; color: var(--ink-muted); font-size: 10px; font-weight: 720; }.collecting-panel .lan-live { grid-column: 1/-1; display: flex; align-items: center; gap: 8px; color: var(--green-deep); font-weight: 720; }.lan-live span { width: 8px; height: 8px; border-radius: 50%; background: #4f806e; box-shadow: 0 0 0 4px rgba(79,128,110,.14); }
.lan-overlay { position: fixed; z-index: 80; inset: 0; display: grid; place-items: center; padding: 24px; background: rgba(28,38,34,.46); backdrop-filter: blur(9px); }.lan-dialog { position: relative; width: min(760px,100%); max-height: min(780px,calc(100vh - 48px)); overflow: auto; padding: 34px; border: 1px solid rgba(33,51,45,.14); border-radius: 8px 30px 30px; background: var(--paper); box-shadow: 0 30px 90px rgba(20,30,26,.28); }.lan-dialog h2 { margin: 4px 0 0; font-family: var(--font-serif); font-size: 34px; }.lan-intro { margin: 10px 0 24px; color: var(--ink-muted); }.lan-close { position: absolute; top: 17px; right: 17px; display: grid; place-items: center; width: 38px; height: 38px; padding: 0; border: 0; border-radius: 50%; color: var(--ink); background: var(--sand); cursor: pointer; }.lan-session-grid { display: grid; grid-template-columns: minmax(250px,320px) 1fr; gap: 30px; align-items: center; }.qr-paper { padding: 18px; border: 1px solid var(--line); border-radius: 18px; background: #fff; }.qr-paper img { display: block; width: 100%; aspect-ratio: 1; }.lan-session-copy { display: grid; gap: 13px; }.lan-session-copy div { display: grid; gap: 3px; padding-bottom: 12px; border-bottom: 1px solid var(--line); }.lan-session-copy span,.lan-address-label { color: var(--ink-muted); font-size: 11px; font-weight: 720; }.lan-session-copy strong { font-size: 16px; }.lan-session-copy p,.lan-empty { margin: 2px 0; color: var(--ink-muted); font-size: 12px; line-height: 1.65; }.lan-stop,.lan-start { display: inline-flex; gap: 8px; align-items: center; justify-content: center; min-height: 44px; padding: 0 18px; border: 0; border-radius: 999px; color: var(--paper); background: var(--vermillion); font-weight: 760; cursor: pointer; }.lan-stop { justify-self: start; }.lan-start { margin-top: 20px; background: var(--green-deep); }.lan-refresh { min-height: 38px; margin-top: 12px; padding: 0 14px; color: var(--green-deep); border: 1px solid var(--line-strong); border-radius: 999px; background: transparent; font-weight: 720; cursor: pointer; }.lan-address-label { display: grid; gap: 8px; }.lan-address-label select { min-height: 48px; padding: 0 14px; border: 1px solid var(--line-strong); border-radius: 13px; color: var(--ink); background: #fffdf8; }
.lan-preflight { min-height: 240px; }.lan-progress { display: flex; gap: 10px; align-items: center; color: var(--ink-muted); font-size: 12px; }.lan-progress span { width: 9px; height: 9px; border-radius: 50%; background: var(--cinnabar); animation: lan-pulse 1.1s ease-in-out infinite alternate; }.lan-state { display: inline-flex; margin-bottom: 11px; padding: 6px 10px; border-radius: 999px; font-size: 10px; font-weight: 800; letter-spacing: .04em; }.lan-state.is-attention { color: #87402f; background: rgba(185,88,63,.12); }.lan-state.is-ready { color: #3f6857; background: var(--green-soft); }.lan-actions { display: flex; gap: 10px; align-items: center; flex-wrap: wrap; }.lan-primary,.lan-secondary { min-height: 44px; padding: 0 17px; border-radius: 999px; font-weight: 760; cursor: pointer; }.lan-primary { color: var(--paper); border: 1px solid var(--green-deep); background: var(--green-deep); }.lan-secondary { color: var(--green-deep); border: 1px solid var(--line-strong); background: transparent; }.lan-safety { max-width: 620px; margin: 16px 0 0; padding-left: 12px; color: var(--ink-muted); border-left: 2px solid var(--sand); font-size: 11px; line-height: 1.6; }
.lan-permission-summary { display: flex; gap: 10px; align-items: center; margin: 0 0 14px; padding: 12px 13px; color: var(--green-deep); border: 1px solid rgba(33,51,45,.12); border-radius: 12px; background: var(--green-soft); }.lan-permission-summary p { display: grid; gap: 2px; margin: 0; }.lan-permission-summary strong { font-size: 11px; }.lan-permission-summary span { color: var(--ink-muted); font-size: 10px; line-height: 1.5; }.lan-troubleshooting { margin-top: 13px; color: var(--ink-muted); border: 1px solid rgba(33,51,45,.12); border-radius: 11px; background: rgba(232,221,199,.18); }.lan-troubleshooting summary { padding: 11px 13px; color: var(--green-deep); font-size: 11px; font-weight: 760; cursor: pointer; }.lan-troubleshooting ul { display: grid; gap: 7px; margin: 0; padding: 0 18px 14px 31px; font-size: 10px; line-height: 1.55; }.lan-primary:focus-visible,.lan-secondary:focus-visible,.lan-troubleshooting summary:focus-visible { outline: 3px solid rgba(185,88,63,.28); outline-offset: 2px; }
@keyframes lan-pulse { to { transform: scale(1.35); opacity: .45; } }
.layout-bar { display: flex; gap: 11px; align-items: end; margin-top: 25px; padding: 17px; border: 1px solid var(--line); border-radius: 14px; background: rgba(255,253,247,.66); }.layout-heading { display: flex; flex: 1; gap: 10px; align-items: flex-start; }.layout-heading h2,.layout-heading p { margin: 0; }.layout-heading h2 { font-size: 16px; }.layout-heading p { margin-top: 3px; color: var(--ink-muted); font-size: 10px; }.layout-bar select { min-width: 210px; }.layout-bar input { width: 78px; }
.batch-subject-bar { display: grid; grid-template-columns: minmax(220px,.7fr) minmax(0,1.3fr); gap: 18px; align-items: center; margin-top: 24px; padding: 17px 19px; border: 1px solid rgba(33,51,45,.16); border-radius: 5px 17px 17px; background: linear-gradient(120deg,rgba(225,235,229,.7),rgba(255,253,247,.74)); box-shadow: var(--shadow-soft); }.batch-subject-bar p,.batch-subject-bar strong,.batch-subject-bar span { display: block; margin: 0; }.batch-subject-bar p { color: var(--cinnabar); font-size: 10px; font-weight: 840; letter-spacing: .12em; }.batch-subject-bar strong { margin-top: 3px; font-family: var(--font-serif); font-size: 21px; }.batch-subject-bar span { margin-top: 3px; color: var(--ink-muted); font-size: 10px; }.batch-subject-options { display: flex; gap: 7px; flex-wrap: wrap; justify-content: flex-end; }.batch-subject-options button { min-width: 52px; min-height: 34px; padding: 0 12px; color: var(--ink-muted); border: 1px solid var(--line); border-radius: 999px; background: var(--paper); cursor: pointer; transition: transform var(--motion-feedback),color var(--motion-feedback),background var(--motion-feedback); }.batch-subject-options button:hover { transform: translateY(-1px); }.batch-subject-options button.selected { color: var(--paper); border-color: var(--green-deep); background: var(--green-deep); }
.organizer-grid { display:grid; grid-template-columns:minmax(290px,340px) minmax(0,1fr); gap:20px; align-items:start; margin-top:18px; }
.unassigned-strip { position:sticky; top:18px; max-height:calc(100vh - 120px); padding:17px; overflow:auto; border:1px dashed rgba(33,51,45,.22); border-radius:16px; background:rgba(232,221,199,.24); box-shadow:0 12px 32px rgba(34,48,43,.06); }
.strip-heading { display:flex; justify-content:space-between; align-items:center; }.strip-heading div { display:grid; gap:2px; }.strip-heading p { margin:0; font-weight:780; }.strip-heading span { color:var(--ink-muted); font-size:10px; }.strip-heading strong { color:var(--cinnabar); font-family:serif; font-size:20px; }
.unassigned-gallery { display:grid; gap:12px; margin-top:13px; }.unassigned-item { min-width:0; padding:8px; border:2px solid rgba(33,51,45,.22); border-radius:14px; background:rgba(255,253,247,.72); transition:transform var(--motion-feedback) var(--ease-standard),border-color var(--motion-feedback),background var(--motion-feedback); }.unassigned-item:hover { transform:translateY(-2px); }.unassigned-item.is-question { border-color:rgba(33,51,45,.52); background:rgba(225,235,229,.7); }.unassigned-item.is-answer { border-color:rgba(185,88,63,.58); background:rgba(247,225,216,.68); }.role-chip { display:flex; justify-content:space-between; gap:8px; align-items:center; margin-top:7px; }.role-chip span { padding:5px 8px; color:var(--paper); border-radius:999px; background:var(--green-deep); font-size:9px; font-weight:850; }.is-answer .role-chip span { background:var(--cinnabar); }.role-chip small { color:var(--ink-muted); font-size:9px; }.strip-empty { display:flex; gap:7px; align-items:center; margin:15px 0 2px; color:#537064; font-size:11px; }
.draft-stack { min-width:0; }.card-stack-heading { display:flex; justify-content:space-between; gap:18px; align-items:end; margin:0 2px 12px; }.card-stack-heading p,.card-stack-heading h2 { margin:0; }.card-stack-heading p { color:var(--cinnabar); font-size:10px; font-weight:800; letter-spacing:.12em; }.card-stack-heading h2 { margin-top:3px; font-size:20px; }.card-stack-heading>span { max-width:240px; color:var(--ink-muted); font-size:10px; text-align:right; }.draft-cards { display:grid; gap:18px; }
.new-card-drop { display:grid; min-height:110px; margin-top:14px; padding:18px; place-content:center; justify-items:center; gap:5px; color:var(--green-deep); border:2px dashed rgba(33,51,45,.3); border-radius:16px; background:rgba(232,221,199,.17); text-align:center; transition:transform var(--motion-feedback),border-color var(--motion-feedback),background var(--motion-feedback); }.new-card-drop strong { font-size:13px; }.new-card-drop span { color:var(--ink-muted); font-size:10px; }.capture-pointer-dragging .new-card-drop { border-color:var(--cinnabar); background:rgba(185,88,63,.08); transform:scale(1.01); }.card-inspector { margin-top:16px; padding:17px; border:1px solid var(--line); border-radius:16px; background:rgba(255,253,247,.72); }.card-inspector header { display:flex; justify-content:space-between; gap:18px; align-items:end; }.card-inspector header p,.card-inspector header h3 { margin:0; }.card-inspector header p { color:var(--cinnabar); font-size:9px; font-weight:850; letter-spacing:.12em; }.card-inspector header h3 { margin-top:3px; font-size:16px; }.card-inspector header>span { color:var(--ink-muted); font-size:9px; }.inspector-fields { display:grid; grid-template-columns:.8fr 1.6fr; gap:9px; margin-top:12px; }.inspector-fields label { display:grid; gap:5px; color:var(--ink-muted); font-size:9px; font-weight:760; }.inspector-fields input,.inspector-fields textarea { width:100%; }.inspector-fields textarea { resize:vertical; }.capture-drag-ghost { position:fixed; z-index:200; top:0; left:0; display:grid; width:112px; height:88px; overflow:hidden; pointer-events:none; place-items:center; border:2px solid var(--cinnabar); border-radius:13px; color:var(--paper); background:var(--green-deep); box-shadow:0 18px 45px rgba(20,28,25,.3); will-change:transform; }.capture-drag-ghost img { width:100%; height:100%; object-fit:cover; opacity:.86; }.capture-drag-ghost span { position:absolute; right:6px; bottom:6px; padding:4px 7px; border-radius:999px; background:rgba(33,51,45,.86); font-size:9px; font-weight:800; }
.capture-pointer-dragging .new-card-drop:not(.is-drop-question):not(.is-drop-answer){border-color:rgba(33,51,45,.36);background:rgba(232,221,199,.22);transform:none}.new-card-drop.is-drop-question{color:var(--green-deep);border-color:rgba(33,51,45,.72);background:rgba(225,235,229,.82);transform:scale(1.012)}.new-card-drop.is-drop-answer{color:var(--cinnabar);border-color:rgba(185,88,63,.72);background:rgba(247,225,216,.82);transform:scale(1.012)}.capture-drag-ghost{transition:opacity var(--motion-feedback) var(--ease-standard);animation:capture-card-lift var(--motion-feedback) var(--ease-standard)}.capture-drag-ghost.is-question{border-color:rgba(33,51,45,.78);background:var(--green-deep)}.capture-drag-ghost.is-answer{border-color:rgba(185,88,63,.9);background:var(--cinnabar)}.capture-drag-ghost.is-answer span{background:rgba(125,55,39,.9)}
@keyframes capture-card-lift{from{opacity:0}}
.organizer-move-move,.organizer-move-enter-active,.organizer-move-leave-active { transition:transform var(--motion-page) var(--ease-standard),opacity var(--motion-standard) var(--ease-standard); }.organizer-move-enter-from,.organizer-move-leave-to { opacity:0; transform:translateY(10px) scale(.985); }.organizer-move-leave-active { position:absolute; }
.commit-dock { position: sticky; z-index: 12; bottom: 14px; display: flex; justify-content: space-between; gap: 20px; align-items: center; margin-top: 22px; padding: 15px 17px; border: 1px solid rgba(33,51,45,.15); border-radius: 15px; background: rgba(246,241,231,.94); box-shadow: 0 16px 45px rgba(34,48,43,.18); backdrop-filter: blur(16px); }.commit-dock div { display: grid; grid-template-columns: auto auto; gap: 2px 9px; }.commit-dock p,.commit-dock strong,.commit-dock span { margin: 0; }.commit-dock p { color: var(--cinnabar); font-size: 9px; font-weight: 800; letter-spacing: .1em; }.commit-dock strong { font-size: 16px; }.commit-dock span { grid-column: 1/-1; color: var(--ink-muted); font-size: 10px; }
.completed-panel { display: grid; min-height: 420px; place-content: center; justify-items: center; text-align: center; }.completed-panel h2 { margin: 14px 0 5px; }.completed-panel p { margin: 0; color: var(--ink-muted); }.completed-panel button { margin-top: 18px; padding: 10px 16px; color: var(--paper); border: 0; border-radius: 999px; background: var(--green-deep); }
@media (max-width: 980px) { .batch-grid { grid-template-columns: repeat(2,minmax(0,1fr)); }.collecting-panel { grid-template-columns: 1fr; }.layout-bar { align-items: stretch; flex-wrap: wrap; }.lan-session-grid { grid-template-columns: 1fr; }.qr-paper { width: min(320px,100%); margin: auto; }.batch-subject-bar { grid-template-columns: 1fr; }.batch-subject-options { justify-content: flex-start; } }
@media (max-width: 900px) { .organizer-grid { grid-template-columns:1fr; }.unassigned-strip { position:static; max-height:none; }.unassigned-gallery { grid-template-columns:repeat(2,minmax(0,1fr)); } }
@media (max-width: 720px) { .capture-next { padding: 30px 20px 110px; }.inbox-hero,.workbench-header,.new-batch-card { align-items: stretch; flex-direction: column; }.new-batch-card form,.capture-toolbar { grid-template-columns: 1fr; flex-direction: column; }.new-batch-card select { width: 100%; }.batch-grid { grid-template-columns: 1fr; }.capture-toolbar { display: grid; }.workbench-stats { align-self: stretch; }.workbench-stats span { flex: 1; }.layout-bar select { min-width: 100%; }.commit-dock { align-items: stretch; flex-direction: column; }.commit-dock button { width: 100%; }.lan-dialog { padding: 28px 20px; }.lan-dialog h2 { font-size: 28px; }.lan-actions>* { width: 100%; } }
@media (max-width: 560px) { .unassigned-gallery { grid-template-columns:1fr; }.card-stack-heading { align-items:start; flex-direction:column; }.card-stack-heading>span { text-align:left; } }
@media (prefers-reduced-motion: reduce) { .batch-card,.external-drop,.batch-subject-options button,.capture-drag-ghost,.new-card-drop,.organizer-move-move,.organizer-move-enter-active,.organizer-move-leave-active { transition: none; animation:none; }.lan-progress span { animation: none; } }
</style>
