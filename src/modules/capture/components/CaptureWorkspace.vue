<script setup lang="ts">
import {
  ArrowLeft, Check, ChevronLeft, ChevronRight, ClipboardPaste, FolderOpen,
  Images, LayoutGrid, ListPlus, LockKeyhole, Plus, QrCode, Save, Smartphone,
  Sparkles, Trash2, UploadCloud, X,
} from '@lucide/vue'
import { computed, nextTick, reactive, ref, watch } from 'vue'
import type {
  CaptureBatchDetail, CaptureBatchSummary, CaptureDraftSummary, CaptureItemSummary,
  CaptureLanAddress, CaptureLanPreflight, CaptureLanSession, CaptureLanSettingsPage,
  CaptureLayoutMode,
} from '../../../shared/api/bindings'
import CaptureThumbnail from './CaptureThumbnail.vue'

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
  lanGuidePolling: boolean
  lanSession: CaptureLanSession | undefined
}>()

const emit = defineEmits<{
  createBatch: [subject: string]
  openBatch: [batchId: string]
  back: []
  discardBatch: [batchId: string]
  importSelect: []
  importFiles: [files: File[]]
  finishCollecting: [subject: string]
  applyLayout: [mode: CaptureLayoutMode, questions: number, answers: number, splitIndex: number | null]
  createDraft: []
  moveItem: [target: MoveTarget]
  updateDraft: [draft: CaptureDraftSummary, subject: string, tags: string[], note: string]
  removeItem: [itemId: string]
  commitReady: []
  preview: [itemId: string]
  mobileCapture: [selectedAddress: string | null]
  refreshLanAddresses: []
  refreshLanPreflight: []
  repairLanFirewall: []
  openLanNetworkSettings: [page: CaptureLanSettingsPage]
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
const networkGuideKind = ref<Extract<CaptureLanSettingsPage, 'wifi' | 'ethernet'>>('wifi')
const draftEdits = reactive<Record<string, { subject: string, tags: string, note: string }>>({})

const itemById = computed(() => new Map(props.detail?.items.map(item => [item.id, item]) ?? []))
const activeBatches = computed(() => props.batches.filter(batch => batch.state !== 'completed'))
const completedBatches = computed(() => props.batches.filter(batch => batch.state === 'completed'))
const readyCount = computed(() => props.detail?.drafts.filter(draft => draft.ready).length ?? 0)
const isCollecting = computed(() => props.detail?.batch.state === 'collecting')
const unassignedItems = computed(() => props.detail?.unassignedItemIds
  .map(id => itemById.value.get(id))
  .filter((item): item is CaptureItemSummary => Boolean(item)) ?? [])
const lanMinutesRemaining = computed(() => props.lanSession
  ? Math.max(0, Math.ceil(((props.lanSession.expiresAtUtcMs ?? Date.now()) - Date.now()) / 60_000))
  : 0)
const lanNeedsNetworkChange = computed(() => props.lanPreflight?.needsNetworkChange === true)
const lanNeedsRepair = computed(() => props.lanPreflight?.needsFirewallRepair === true)
const lanReady = computed(() => props.lanPreflight?.canStart === true)

watch(() => props.detail, (detail) => {
  if (!detail) return
  batchSubject.value = detail.batch.subject
  splitIndex.value = Math.ceil(detail.items.length / 2)
  for (const draft of detail.drafts) {
    const current = draftEdits[draft.id]
    if (!current) {
      draftEdits[draft.id] = {
        subject: draft.subject,
        tags: draft.tags.join('，'),
        note: draft.note,
      }
    }
  }
}, { immediate: true })

watch(() => props.lanAddresses, (addresses) => {
  if (!addresses.some(address => address.address === selectedLanAddress.value)) {
    selectedLanAddress.value = addresses[0]?.address ?? ''
  }
}, { immediate: true })

watch(() => props.lanSession, (session) => {
  if (session && !showLanPanel.value) void showLanDialog(false)
})

watch(
  () => [
    props.lanSession?.sessionId,
    props.lanPreflight?.needsNetworkChange,
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

async function showLanDialog(refresh = true) {
  lanFocusReturn = document.activeElement instanceof HTMLElement ? document.activeElement : lanLauncher.value ?? null
  showLanPanel.value = true
  await nextTick()
  lanClose.value?.focus()
  if (refresh) {
    emit('refreshLanAddresses')
    emit('refreshLanPreflight')
  }
}

function openLanPanel() {
  void showLanDialog()
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

function draggedItemId(event: DragEvent) {
  return event.dataTransfer?.getData('application/x-mistake-capture-item')
    || event.dataTransfer?.getData('text/plain')
    || ''
}

function dropItem(event: DragEvent, targetDraftId: string | null, targetRole: 'question' | 'answer' | null, targetPosition: number) {
  const itemId = draggedItemId(event)
  if (!itemId || !itemById.value.has(itemId)) return
  emit('moveItem', { itemId, targetDraftId, targetRole, targetPosition })
}

function itemsFor(draft: CaptureDraftSummary, role: 'question' | 'answer') {
  const ids = role === 'question' ? draft.questionItemIds : draft.answerItemIds
  return ids.map(id => itemById.value.get(id)).filter((item): item is CaptureItemSummary => Boolean(item))
}

function saveDraft(draft: CaptureDraftSummary) {
  const edit = draftEdits[draft.id]
  if (!edit) return
  emit(
    'updateDraft',
    draft,
    edit.subject.trim(),
    edit.tags.split(/[，,]/).map(tag => tag.trim()).filter(Boolean),
    edit.note.trim(),
  )
}

function moveAcrossDrafts(item: CaptureItemSummary, draftIndex: number, role: 'question' | 'answer', delta: number) {
  const target = props.detail?.drafts[draftIndex + delta]
  if (!target) return
  emit('moveItem', {
    itemId: item.id,
    targetDraftId: target.id,
    targetRole: role,
    targetPosition: itemsFor(target, role).length,
  })
}

function moveWithin(item: CaptureItemSummary, draft: CaptureDraftSummary, role: 'question' | 'answer', delta: number) {
  const items = itemsFor(draft, role)
  const index = items.findIndex(candidate => candidate.id === item.id)
  emit('moveItem', {
    itemId: item.id,
    targetDraftId: draft.id,
    targetRole: role,
    targetPosition: Math.max(0, Math.min(items.length - 1, index + delta)),
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
          <input
            v-model="newSubject"
            maxlength="40"
            placeholder="科目，例如：数学（可选）"
          >
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
              <template v-if="lanPreflightBusy && !lanPreflight">
                <h2 id="lan-dialog-title">
                  正在检查连接条件
                </h2>
                <p class="lan-intro">
                  正在确认 Windows 网络类型与手机连接权限……
                </p>
                <div class="lan-progress">
                  <span />检查专用网络与防火墙
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

              <template v-else-if="lanNeedsNetworkChange">
                <span class="lan-state is-attention">活动网络中含有非专用连接</span>
                <h2 id="lan-dialog-title">
                  跟着 4 步，把可信网络设为专用
                </h2>
                <p class="lan-intro">
                  只对你自己的家庭 Wi‑Fi 或个人热点这样设置。学校、商场、咖啡店等公共网络请保持“公用”，改用手机热点。
                </p>

                <div
                  class="network-kind"
                  aria-label="选择电脑的联网方式"
                >
                  <button
                    type="button"
                    aria-label="Wi‑Fi"
                    :class="{ 'is-selected': networkGuideKind === 'wifi' }"
                    :aria-pressed="networkGuideKind === 'wifi'"
                    @click="networkGuideKind = 'wifi'"
                  >
                    <strong>Wi‑Fi</strong><span>无线网络</span>
                  </button>
                  <button
                    type="button"
                    aria-label="网线 / 扩展坞"
                    :class="{ 'is-selected': networkGuideKind === 'ethernet' }"
                    :aria-pressed="networkGuideKind === 'ethernet'"
                    @click="networkGuideKind = 'ethernet'"
                  >
                    <strong>网线 / 扩展坞</strong><span>以太网</span>
                  </button>
                </div>

                <ol class="setup-steps">
                  <li>
                    <span>1</span>
                    <div>
                      <strong>打开对应的 Windows 设置页</strong>
                      <p>点击下面的“打开{{ networkGuideKind === 'wifi' ? ' Wi‑Fi ' : '以太网' }}设置”，会在应用旁边弹出系统设置。</p>
                    </div>
                  </li>
                  <li>
                    <span>2</span>
                    <div v-if="networkGuideKind === 'wifi'">
                      <strong>点击当前已连接的 Wi‑Fi 名称</strong>
                      <p>在 Wi‑Fi 页面顶部找到正在使用的网络；点击它，或点击旁边的“属性”。不要点“管理已知网络”。</p>
                    </div>
                    <div v-else>
                      <strong>打开当前以太网连接的属性</strong>
                      <p>如果设置页显示多个网口，点击状态为“已连接”的那个；通常名称是“以太网”。</p>
                    </div>
                  </li>
                  <li>
                    <span>3</span>
                    <div>
                      <strong>进入“网络配置文件类型”</strong>
                      <p>找到“网络配置文件类型”，选择“专用网络”。不要修改 IP、DNS、代理或防火墙开关。</p>
                    </div>
                  </li>
                  <li>
                    <span>4</span>
                    <div>
                      <strong>回到这里完成检测</strong>
                      <p>设置成功后本窗口会自动进入下一步；如果没有变化，点击“设置完成，立即检测”。</p>
                    </div>
                  </li>
                </ol>

                <div class="lan-actions guide-actions">
                  <button
                    class="lan-primary"
                    type="button"
                    @click="emit('openLanNetworkSettings', networkGuideKind)"
                  >
                    打开{{ networkGuideKind === 'wifi' ? ' Wi‑Fi ' : '以太网' }}设置
                  </button>
                  <button
                    class="lan-secondary"
                    type="button"
                    @click="emit('refreshLanPreflight')"
                  >
                    设置完成，立即检测
                  </button>
                </div>
                <p
                  v-if="lanGuidePolling"
                  class="guide-detection"
                >
                  <span />Windows 设置已打开；本应用会在 90 秒内自动检测，设置成功后这里会直接进入下一步。
                </p>

                <details class="lan-troubleshooting">
                  <summary>还是检测不过？按这里逐项排查</summary>
                  <ul>
                    <li><strong>先断开不需要的连接：</strong>VPN、公司网络、虚拟机网卡或另一根网线可能仍被 Windows 标记为公用。</li>
                    <li><strong>找不到“专用网络”：</strong>电脑可能受学校或公司管理。请不要绕过限制，改用个人电脑或手机热点。</li>
                    <li><strong>正在使用公共 Wi‑Fi：</strong>不要把它改成专用。可用另一台手机开热点，让电脑和拍照设备都连入；如果就用开热点的手机拍照，只需让电脑连入该热点。</li>
                  </ul>
                </details>
                <p class="lan-safety">
                  应用不会开放公用网络，也不会自动更改你的网络类型。
                </p>
              </template>

              <template v-else-if="lanNeedsRepair">
                <span class="lan-state is-attention">需要一次 Windows 授权</span>
                <h2 id="lan-dialog-title">
                  允许手机连接这台电脑
                </h2>
                <p class="lan-intro">
                  网络已经设置正确。最后需要让 Windows 防火墙认识本应用，这一步通常只做一次。
                </p>
                <ol class="setup-steps firewall-steps">
                  <li><span>1</span><div><strong>点击“修复手机连接”</strong><p>应用会请求创建一条仅用于本机手机采集的专用网络规则。</p></div></li>
                  <li><span>2</span><div><strong>Windows 弹窗中点击“是”</strong><p>看到“是否允许此应用对你的设备进行更改”时，确认应用是 Mistake Trainer Next，然后点击“是”。</p></div></li>
                  <li><span>3</span><div><strong>回到应用等待自动检测</strong><p>通过后会直接显示“连接权限已就绪”；不会要求重启，也不需要输入命令。</p></div></li>
                </ol>
                <div class="lan-actions">
                  <button
                    class="lan-primary"
                    type="button"
                    :disabled="busy"
                    @click="emit('repairLanFirewall')"
                  >
                    {{ busy ? '等待 Windows 确认…' : '修复手机连接' }}
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
                    <li>如果点了“否”或关闭弹窗，不会做任何更改，回到这里可以再次点击“修复手机连接”。</li>
                    <li>修复不会开放公用网络，也不会关闭 Windows 防火墙。</li>
                  </ul>
                </details>
              </template>

              <template v-else-if="lanReady">
                <span class="lan-state is-ready">专用网络与连接权限已就绪</span>
                <h2 id="lan-dialog-title">
                  选择手机所在的网络
                </h2>
                <p class="lan-intro">
                  仅适用于可信的家庭 Wi‑Fi 或手机个人热点。不要在公共 Wi‑Fi 上使用。
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
        <label>批次科目<input
          v-model="batchSubject"
          maxlength="40"
          placeholder="例如：数学"
        ></label>
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

        <section
          class="unassigned-strip"
          @dragover.prevent
          @drop.prevent="dropItem($event, null, null, 0)"
        >
          <div class="strip-heading">
            <div><p>未分配图片</p><span>拖到下方题图或答案区</span></div><strong>{{ unassignedItems.length }}</strong>
          </div>
          <div
            v-if="unassignedItems.length"
            class="thumbnail-row"
          >
            <div
              v-for="item in unassignedItems"
              :key="item.id"
              class="unassigned-item"
            >
              <CaptureThumbnail
                :item="item"
                :data-url="previews[item.id]"
                removable
                @preview="emit('preview', $event)"
                @remove="emit('removeItem', $event)"
              />
              <div
                v-if="detail.drafts.length"
                class="quick-assign"
              >
                <button
                  type="button"
                  @click="emit('moveItem', { itemId: item.id, targetDraftId: detail.drafts[0]!.id, targetRole: 'question', targetPosition: detail.drafts[0]!.questionItemIds.length })"
                >
                  设为题图
                </button>
                <button
                  type="button"
                  @click="emit('moveItem', { itemId: item.id, targetDraftId: detail.drafts[0]!.id, targetRole: 'answer', targetPosition: detail.drafts[0]!.answerItemIds.length })"
                >
                  设为答案
                </button>
              </div>
            </div>
          </div>
          <p
            v-else
            class="strip-empty"
          >
            <Check :size="16" />所有图片都已分配
          </p>
        </section>

        <section class="draft-stack">
          <article
            v-for="(draft, draftIndex) in detail.drafts"
            :key="draft.id"
            class="draft-card"
            :class="{ 'is-ready': draft.ready }"
          >
            <header class="draft-header">
              <span class="draft-number">{{ String(draftIndex + 1).padStart(2, '0') }}</span><div><h3>错题草稿</h3><p>{{ draft.ready ? '题答齐全，可加入题库' : '还需要题图、答案与科目' }}</p></div><span class="ready-mark">{{ draft.ready ? '就绪' : '未完成' }}</span>
            </header>
            <div class="draft-zones">
              <section
                class="draft-zone question-zone"
                @dragover.prevent
                @drop.prevent="dropItem($event, draft.id, 'question', draft.questionItemIds.length)"
              >
                <div class="zone-title">
                  <span>题图</span><small>{{ draft.questionItemIds.length }} 张</small>
                </div>
                <div
                  v-if="itemsFor(draft, 'question').length"
                  class="zone-items"
                >
                  <div
                    v-for="item in itemsFor(draft, 'question')"
                    :key="item.id"
                    class="assigned-item"
                  >
                    <CaptureThumbnail
                      :item="item"
                      :data-url="previews[item.id]"
                      @preview="emit('preview', $event)"
                    />
                    <div class="item-actions">
                      <button
                        type="button"
                        title="移到上一题"
                        :disabled="draftIndex === 0"
                        @click="moveAcrossDrafts(item, draftIndex, 'question', -1)"
                      >
                        <ChevronLeft :size="13" />
                      </button>
                      <button
                        type="button"
                        title="前移"
                        @click="moveWithin(item, draft, 'question', -1)"
                      >
                        前
                      </button>
                      <button
                        type="button"
                        title="后移"
                        @click="moveWithin(item, draft, 'question', 1)"
                      >
                        后
                      </button>
                      <button
                        type="button"
                        title="改为答案"
                        @click="emit('moveItem', { itemId: item.id, targetDraftId: draft.id, targetRole: 'answer', targetPosition: draft.answerItemIds.length })"
                      >
                        答
                      </button>
                      <button
                        type="button"
                        title="移回未分配"
                        @click="emit('moveItem', { itemId: item.id, targetDraftId: null, targetRole: null, targetPosition: 0 })"
                      >
                        <X :size="13" />
                      </button>
                      <button
                        type="button"
                        title="移到下一题"
                        :disabled="draftIndex === detail.drafts.length - 1"
                        @click="moveAcrossDrafts(item, draftIndex, 'question', 1)"
                      >
                        <ChevronRight :size="13" />
                      </button>
                    </div>
                  </div>
                </div>
                <p
                  v-else
                  class="zone-empty"
                >
                  拖入题目图片
                </p>
              </section>
              <section
                class="draft-zone answer-zone"
                @dragover.prevent
                @drop.prevent="dropItem($event, draft.id, 'answer', draft.answerItemIds.length)"
              >
                <div class="zone-title">
                  <span>答案</span><small>{{ draft.answerItemIds.length }} 张</small>
                </div>
                <div
                  v-if="itemsFor(draft, 'answer').length"
                  class="zone-items"
                >
                  <div
                    v-for="item in itemsFor(draft, 'answer')"
                    :key="item.id"
                    class="assigned-item"
                  >
                    <CaptureThumbnail
                      :item="item"
                      :data-url="previews[item.id]"
                      @preview="emit('preview', $event)"
                    />
                    <div class="item-actions">
                      <button
                        type="button"
                        title="移到上一题"
                        :disabled="draftIndex === 0"
                        @click="moveAcrossDrafts(item, draftIndex, 'answer', -1)"
                      >
                        <ChevronLeft :size="13" />
                      </button>
                      <button
                        type="button"
                        title="前移"
                        @click="moveWithin(item, draft, 'answer', -1)"
                      >
                        前
                      </button>
                      <button
                        type="button"
                        title="后移"
                        @click="moveWithin(item, draft, 'answer', 1)"
                      >
                        后
                      </button>
                      <button
                        type="button"
                        title="改为题图"
                        @click="emit('moveItem', { itemId: item.id, targetDraftId: draft.id, targetRole: 'question', targetPosition: draft.questionItemIds.length })"
                      >
                        题
                      </button>
                      <button
                        type="button"
                        title="移回未分配"
                        @click="emit('moveItem', { itemId: item.id, targetDraftId: null, targetRole: null, targetPosition: 0 })"
                      >
                        <X :size="13" />
                      </button>
                      <button
                        type="button"
                        title="移到下一题"
                        :disabled="draftIndex === detail.drafts.length - 1"
                        @click="moveAcrossDrafts(item, draftIndex, 'answer', 1)"
                      >
                        <ChevronRight :size="13" />
                      </button>
                    </div>
                  </div>
                </div>
                <p
                  v-else
                  class="zone-empty"
                >
                  拖入答案图片
                </p>
              </section>
            </div>
            <div
              v-if="draftEdits[draft.id]"
              class="draft-fields"
            >
              <label><span>科目</span><input
                v-model="draftEdits[draft.id]!.subject"
                maxlength="40"
                @change="saveDraft(draft)"
              ></label>
              <label><span>标签</span><input
                v-model="draftEdits[draft.id]!.tags"
                maxlength="200"
                placeholder="函数，粗心"
                @change="saveDraft(draft)"
              ></label>
              <label class="note-field"><span>笔记</span><textarea
                v-model="draftEdits[draft.id]!.note"
                maxlength="500"
                rows="2"
                placeholder="错因或下次提醒"
                @change="saveDraft(draft)"
              /></label>
            </div>
          </article>
          <button
            class="add-draft"
            type="button"
            :disabled="busy"
            @click="emit('createDraft')"
          >
            <Plus :size="17" />添加空白草稿
          </button>
        </section>

        <footer class="commit-dock">
          <div><p>原子批量入库</p><strong>{{ readyCount ? `${readyCount} 道已就绪` : '还没有可入库的题目' }}</strong><span>未完成草稿会继续留在采集箱</span></div>
          <button
            type="button"
            :disabled="busy || readyCount === 0"
            @click="emit('commitReady')"
          >
            <Save :size="18" />保存全部就绪题
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
.new-batch-card form { display: flex; gap: 9px; }.new-batch-card input { width: 240px; }.new-batch-card button, .capture-toolbar button, .collecting-panel>button, .layout-bar>button, .commit-dock button { display: inline-flex; gap: 8px; align-items: center; justify-content: center; min-height: 43px; padding: 0 17px; color: var(--paper); border: 0; border-radius: 999px; background: var(--green-deep); font-weight: 740; cursor: pointer; }
input, textarea, select { box-sizing: border-box; padding: 10px 12px; color: var(--ink); border: 1px solid var(--line); border-radius: 10px; outline: none; background: rgba(246,241,231,.66); font: inherit; }.new-batch-card button:disabled, button:disabled { cursor: not-allowed; opacity: .4; }
.batch-section { margin-top: 42px; }.section-heading { display: flex; justify-content: space-between; align-items: flex-end; }.section-heading p, .section-heading h2 { margin: 0; }.section-heading p { color: var(--cinnabar); font-size: 11px; font-weight: 760; letter-spacing: .12em; }.section-heading h2 { margin-top: 3px; font-size: 24px; }.section-heading>span { color: var(--ink-muted); font-size: 12px; }
.batch-grid { display: grid; grid-template-columns: repeat(3,minmax(0,1fr)); gap: 15px; margin-top: 17px; }.batch-card { position: relative; min-width: 0; border: 1px solid var(--line); border-radius: 4px 18px 18px; background: rgba(255,253,247,.7); box-shadow: var(--shadow-soft); transition: transform var(--motion-standard) var(--ease-standard), box-shadow var(--motion-standard); }.batch-card:hover { transform: translateY(-3px); box-shadow: 0 18px 36px rgba(34,48,43,.11); }.batch-open { width: 100%; padding: 24px; text-align: left; border: 0; background: transparent; cursor: pointer; }.batch-state { color: var(--cinnabar); font-size: 10px; font-weight: 800; letter-spacing: .1em; }.batch-card h3 { margin: 11px 0 6px; font-size: 22px; }.batch-card p { margin: 0; color: var(--ink-muted); font-size: 12px; }.batch-card strong { display: flex; gap: 4px; align-items: center; margin-top: 26px; color: var(--green-deep); font-size: 12px; }.batch-delete { position: absolute; top: 15px; right: 15px; display: grid; width: 32px; height: 32px; place-items: center; color: var(--ink-muted); border: 0; border-radius: 50%; background: transparent; cursor: pointer; }.batch-delete:hover { color: var(--cinnabar); background: rgba(185,88,63,.1); }
.empty-inbox { display: grid; min-height: 210px; margin-top: 17px; place-content: center; justify-items: center; gap: 13px; color: var(--ink-muted); border: 1px dashed rgba(33,51,45,.2); border-radius: 16px; }.empty-inbox p { max-width: 380px; margin: 0; text-align: center; }.completed-section { margin-top: 30px; color: var(--ink-muted); }.completed-section button { margin: 10px 8px 0 0; padding: 8px 12px; color: inherit; border: 1px solid var(--line); border-radius: 9px; background: transparent; }
.workbench-header { align-items: center; }.back-button { display: inline-flex; gap: 7px; align-items: center; padding: 9px 12px; color: var(--ink-muted); border: 1px solid var(--line); border-radius: 999px; background: rgba(255,253,247,.5); cursor: pointer; }.batch-title { flex: 1; }.batch-title h1 { font-size: clamp(32px,4vw,50px); }.workbench-stats { display: flex; gap: 7px; }.workbench-stats span { display: grid; min-width: 58px; padding: 9px; text-align: center; color: var(--ink-muted); border-radius: 10px; background: rgba(232,221,199,.48); font-size: 10px; }.workbench-stats strong { color: var(--ink); font-family: serif; font-size: 20px; }.workbench-stats .ready strong { color: var(--cinnabar); }
.capture-toolbar { display: grid; grid-template-columns: repeat(2,minmax(0,1fr)) minmax(180px,.75fr); gap: 10px; margin-top: 31px; }.capture-toolbar button, .tool-hint { display: flex; gap: 11px; align-items: center; min-height: 60px; padding: 0 17px; color: var(--ink); border: 1px solid var(--line); border-radius: 13px; background: rgba(255,253,247,.67); text-align: left; }.capture-toolbar button { justify-content: flex-start; cursor: pointer; }.capture-toolbar .primary-tool { color: var(--paper); border-color: var(--green-deep); background: var(--green-deep); }.capture-toolbar span, .tool-hint span { display: grid; gap: 2px; }.capture-toolbar strong, .tool-hint strong { font-size: 13px; }.capture-toolbar small, .tool-hint small { opacity: .68; font-size: 10px; }
.external-drop { display: flex; gap: 9px; align-items: center; justify-content: center; min-height: 44px; margin-top: 10px; color: var(--ink-muted); border: 1px dashed rgba(33,51,45,.24); border-radius: 11px; font-size: 11px; transition: background var(--motion-feedback), border-color var(--motion-feedback); }.external-drop.is-active { color: var(--green-deep); border-color: var(--green-deep); background: var(--green-soft); }
.collecting-panel { display: grid; grid-template-columns: minmax(0,1fr) 240px auto; gap: 20px; align-items: end; margin-top: 24px; padding: 26px; border: 1px solid var(--line); border-radius: 5px 20px 20px; background: rgba(255,253,247,.73); box-shadow: var(--shadow-soft); }.collecting-copy { display: flex; gap: 13px; align-items: flex-start; }.collecting-panel h2,.collecting-panel p { margin: 0; }.collecting-panel p { max-width: 550px; margin-top: 5px; color: var(--ink-muted); font-size: 12px; }.collecting-panel label,.layout-bar label,.draft-fields label { display: grid; gap: 6px; color: var(--ink-muted); font-size: 10px; font-weight: 720; }.collecting-panel .lan-live { grid-column: 1/-1; display: flex; align-items: center; gap: 8px; color: var(--green-deep); font-weight: 720; }.lan-live span { width: 8px; height: 8px; border-radius: 50%; background: #4f806e; box-shadow: 0 0 0 4px rgba(79,128,110,.14); }
.lan-overlay { position: fixed; z-index: 80; inset: 0; display: grid; place-items: center; padding: 24px; background: rgba(28,38,34,.46); backdrop-filter: blur(9px); }.lan-dialog { position: relative; width: min(760px,100%); max-height: min(780px,calc(100vh - 48px)); overflow: auto; padding: 34px; border: 1px solid rgba(33,51,45,.14); border-radius: 8px 30px 30px; background: var(--paper); box-shadow: 0 30px 90px rgba(20,30,26,.28); }.lan-dialog h2 { margin: 4px 0 0; font-family: var(--font-serif); font-size: 34px; }.lan-intro { margin: 10px 0 24px; color: var(--ink-muted); }.lan-close { position: absolute; top: 17px; right: 17px; display: grid; place-items: center; width: 38px; height: 38px; padding: 0; border: 0; border-radius: 50%; color: var(--ink); background: var(--sand); cursor: pointer; }.lan-session-grid { display: grid; grid-template-columns: minmax(250px,320px) 1fr; gap: 30px; align-items: center; }.qr-paper { padding: 18px; border: 1px solid var(--line); border-radius: 18px; background: #fff; }.qr-paper img { display: block; width: 100%; aspect-ratio: 1; }.lan-session-copy { display: grid; gap: 13px; }.lan-session-copy div { display: grid; gap: 3px; padding-bottom: 12px; border-bottom: 1px solid var(--line); }.lan-session-copy span,.lan-address-label { color: var(--ink-muted); font-size: 11px; font-weight: 720; }.lan-session-copy strong { font-size: 16px; }.lan-session-copy p,.lan-empty { margin: 2px 0; color: var(--ink-muted); font-size: 12px; line-height: 1.65; }.lan-stop,.lan-start { display: inline-flex; gap: 8px; align-items: center; justify-content: center; min-height: 44px; padding: 0 18px; border: 0; border-radius: 999px; color: var(--paper); background: var(--vermillion); font-weight: 760; cursor: pointer; }.lan-stop { justify-self: start; }.lan-start { margin-top: 20px; background: var(--green-deep); }.lan-refresh { min-height: 38px; margin-top: 12px; padding: 0 14px; color: var(--green-deep); border: 1px solid var(--line-strong); border-radius: 999px; background: transparent; font-weight: 720; cursor: pointer; }.lan-address-label { display: grid; gap: 8px; }.lan-address-label select { min-height: 48px; padding: 0 14px; border: 1px solid var(--line-strong); border-radius: 13px; color: var(--ink); background: #fffdf8; }
.lan-preflight { min-height: 240px; }.lan-progress { display: flex; gap: 10px; align-items: center; color: var(--ink-muted); font-size: 12px; }.lan-progress span { width: 9px; height: 9px; border-radius: 50%; background: var(--cinnabar); animation: lan-pulse 1.1s ease-in-out infinite alternate; }.lan-state { display: inline-flex; margin-bottom: 11px; padding: 6px 10px; border-radius: 999px; font-size: 10px; font-weight: 800; letter-spacing: .04em; }.lan-state.is-attention { color: #87402f; background: rgba(185,88,63,.12); }.lan-state.is-ready { color: #3f6857; background: var(--green-soft); }.lan-actions { display: flex; gap: 10px; align-items: center; flex-wrap: wrap; }.lan-primary,.lan-secondary { min-height: 44px; padding: 0 17px; border-radius: 999px; font-weight: 760; cursor: pointer; }.lan-primary { color: var(--paper); border: 1px solid var(--green-deep); background: var(--green-deep); }.lan-secondary { color: var(--green-deep); border: 1px solid var(--line-strong); background: transparent; }.lan-safety { max-width: 620px; margin: 16px 0 0; padding-left: 12px; color: var(--ink-muted); border-left: 2px solid var(--sand); font-size: 11px; line-height: 1.6; }
.network-kind { display: grid; grid-template-columns: repeat(2,minmax(0,1fr)); gap: 9px; margin: 0 0 14px; }.network-kind button { display: grid; gap: 2px; min-height: 58px; padding: 10px 14px; color: var(--ink-muted); text-align: left; border: 1px solid var(--line); border-radius: 12px; background: rgba(255,253,247,.62); cursor: pointer; }.network-kind button.is-selected { color: var(--green-deep); border-color: rgba(33,51,45,.42); background: var(--green-soft); box-shadow: inset 3px 0 0 var(--green-deep); }.network-kind strong { font-size: 13px; }.network-kind span { font-size: 10px; opacity: .72; }.setup-steps { display: grid; gap: 8px; margin: 0; padding: 0; list-style: none; }.setup-steps li { display: grid; grid-template-columns: 31px minmax(0,1fr); gap: 11px; align-items: start; padding: 12px 13px; border: 1px solid rgba(33,51,45,.1); border-radius: 12px; background: rgba(255,253,247,.54); }.setup-steps li>span { display: grid; width: 28px; height: 28px; color: var(--paper); place-items: center; border-radius: 50%; background: var(--green-deep); font-family: serif; font-size: 14px; font-weight: 800; }.setup-steps strong { display: block; padding-top: 2px; font-size: 12px; }.setup-steps p { margin: 3px 0 0; color: var(--ink-muted); font-size: 11px; line-height: 1.55; }.firewall-steps li>span { background: var(--cinnabar); }.guide-actions { margin-top: 14px; }.lan-troubleshooting { margin-top: 13px; color: var(--ink-muted); border: 1px solid rgba(33,51,45,.12); border-radius: 11px; background: rgba(232,221,199,.18); }.lan-troubleshooting summary { padding: 11px 13px; color: var(--green-deep); font-size: 11px; font-weight: 760; cursor: pointer; }.lan-troubleshooting ul { display: grid; gap: 7px; margin: 0; padding: 0 18px 14px 31px; font-size: 10px; line-height: 1.55; }.lan-primary:focus-visible,.lan-secondary:focus-visible,.network-kind button:focus-visible,.lan-troubleshooting summary:focus-visible { outline: 3px solid rgba(185,88,63,.28); outline-offset: 2px; }
.guide-detection { display: flex; gap: 9px; align-items: center; margin: 11px 0 0; padding: 10px 12px; color: #416353; border-radius: 10px; background: var(--green-soft); font-size: 10px; line-height: 1.5; }.guide-detection span { flex: 0 0 auto; width: 8px; height: 8px; border-radius: 50%; background: #4f806e; box-shadow: 0 0 0 4px rgba(79,128,110,.13); }
@keyframes lan-pulse { to { transform: scale(1.35); opacity: .45; } }
.layout-bar { display: flex; gap: 11px; align-items: end; margin-top: 25px; padding: 17px; border: 1px solid var(--line); border-radius: 14px; background: rgba(255,253,247,.66); }.layout-heading { display: flex; flex: 1; gap: 10px; align-items: flex-start; }.layout-heading h2,.layout-heading p { margin: 0; }.layout-heading h2 { font-size: 16px; }.layout-heading p { margin-top: 3px; color: var(--ink-muted); font-size: 10px; }.layout-bar select { min-width: 210px; }.layout-bar input { width: 78px; }
.unassigned-strip { margin-top: 17px; padding: 17px; border: 1px dashed rgba(33,51,45,.22); border-radius: 14px; background: rgba(232,221,199,.18); }.strip-heading { display: flex; justify-content: space-between; align-items: center; }.strip-heading div { display: flex; gap: 9px; align-items: baseline; }.strip-heading p { margin: 0; font-weight: 760; }.strip-heading span { color: var(--ink-muted); font-size: 10px; }.strip-heading strong { color: var(--cinnabar); }.thumbnail-row { display: grid; grid-template-columns: repeat(3,minmax(0,1fr)); gap: 9px; max-height: 290px; margin-top: 12px; overflow: auto; }.quick-assign { display: flex; gap: 5px; margin-top: 4px; }.quick-assign button,.item-actions button { min-height: 25px; padding: 0 7px; color: var(--ink-muted); border: 1px solid rgba(33,51,45,.13); border-radius: 7px; background: rgba(255,253,247,.7); font-size: 9px; cursor: pointer; }.strip-empty { display: flex; gap: 6px; align-items: center; margin: 12px 0 0; color: #537064; font-size: 11px; }
.draft-stack { display: grid; gap: 14px; margin-top: 18px; }.draft-card { padding: 19px; border: 1px solid var(--line); border-radius: 5px 18px 18px; background: rgba(255,253,247,.75); box-shadow: var(--shadow-soft); }.draft-card.is-ready { border-color: rgba(75,111,94,.33); }.draft-header { display: flex; gap: 10px; align-items: center; }.draft-number { color: var(--cinnabar); font-family: serif; font-size: 21px; }.draft-header div { flex: 1; }.draft-header h3,.draft-header p { margin: 0; }.draft-header h3 { font-size: 16px; }.draft-header p { margin-top: 2px; color: var(--ink-muted); font-size: 10px; }.ready-mark { padding: 5px 8px; color: #4e6f61; border-radius: 999px; background: var(--green-soft); font-size: 9px; font-weight: 800; }
.draft-zones { display: grid; grid-template-columns: repeat(2,minmax(0,1fr)); gap: 10px; margin-top: 14px; }.draft-zone { min-height: 118px; padding: 11px; border: 1px dashed rgba(33,51,45,.18); border-radius: 12px; background: rgba(246,241,231,.5); }.answer-zone { background: rgba(232,221,199,.3); }.zone-title { display: flex; justify-content: space-between; margin-bottom: 8px; }.zone-title span { font-size: 11px; font-weight: 800; }.zone-title small { color: var(--ink-muted); }.zone-items { display: grid; grid-template-columns: repeat(2,minmax(0,1fr)); gap: 7px; }.assigned-item { min-width: 0; }.item-actions { display: flex; gap: 3px; justify-content: center; margin-top: 4px; }.item-actions button { display: grid; flex: 1; padding: 0 3px; place-items: center; }.zone-empty { display: grid; min-height: 70px; margin: 0; place-items: center; color: var(--ink-muted); font-size: 11px; }.draft-fields { display: grid; grid-template-columns: .55fr .75fr 1.7fr; gap: 10px; margin-top: 13px; }.draft-fields textarea { width: 100%; resize: vertical; }.add-draft { display: inline-flex; gap: 7px; align-items: center; justify-content: center; min-height: 43px; color: var(--green-deep); border: 1px dashed rgba(33,51,45,.26); border-radius: 12px; background: transparent; cursor: pointer; }
.commit-dock { position: sticky; z-index: 12; bottom: 14px; display: flex; justify-content: space-between; gap: 20px; align-items: center; margin-top: 22px; padding: 15px 17px; border: 1px solid rgba(33,51,45,.15); border-radius: 15px; background: rgba(246,241,231,.94); box-shadow: 0 16px 45px rgba(34,48,43,.18); backdrop-filter: blur(16px); }.commit-dock div { display: grid; grid-template-columns: auto auto; gap: 2px 9px; }.commit-dock p,.commit-dock strong,.commit-dock span { margin: 0; }.commit-dock p { color: var(--cinnabar); font-size: 9px; font-weight: 800; letter-spacing: .1em; }.commit-dock strong { font-size: 16px; }.commit-dock span { grid-column: 1/-1; color: var(--ink-muted); font-size: 10px; }
.completed-panel { display: grid; min-height: 420px; place-content: center; justify-items: center; text-align: center; }.completed-panel h2 { margin: 14px 0 5px; }.completed-panel p { margin: 0; color: var(--ink-muted); }.completed-panel button { margin-top: 18px; padding: 10px 16px; color: var(--paper); border: 0; border-radius: 999px; background: var(--green-deep); }
@media (max-width: 980px) { .batch-grid,.thumbnail-row { grid-template-columns: repeat(2,minmax(0,1fr)); }.collecting-panel { grid-template-columns: 1fr; }.layout-bar { align-items: stretch; flex-wrap: wrap; }.draft-fields { grid-template-columns: 1fr 1fr; }.note-field { grid-column: 1/-1; }.lan-session-grid { grid-template-columns: 1fr; }.qr-paper { width: min(320px,100%); margin: auto; } }
@media (max-width: 720px) { .capture-next { padding: 30px 20px 110px; }.inbox-hero,.workbench-header,.new-batch-card { align-items: stretch; flex-direction: column; }.new-batch-card form,.capture-toolbar { grid-template-columns: 1fr; flex-direction: column; }.new-batch-card input { width: 100%; }.batch-grid,.thumbnail-row,.draft-zones,.zone-items,.draft-fields,.network-kind { grid-template-columns: 1fr; }.capture-toolbar { display: grid; }.workbench-stats { align-self: stretch; }.workbench-stats span { flex: 1; }.layout-bar select { min-width: 100%; }.commit-dock { align-items: stretch; flex-direction: column; }.commit-dock button { width: 100%; }.lan-dialog { padding: 28px 20px; }.lan-dialog h2 { font-size: 28px; }.lan-actions>* { width: 100%; } }
@media (prefers-reduced-motion: reduce) { .batch-card,.external-drop { transition: none; }.lan-progress span { animation: none; } }
</style>
