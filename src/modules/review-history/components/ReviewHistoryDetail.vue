<script setup lang="ts">
import { CalendarClock, Cpu, History, Monitor, RotateCcw, X } from '@lucide/vue'
import { nextTick, onBeforeUnmount, onMounted, ref } from 'vue'
import type { ReviewHistoryDetail } from '@/shared/api/bindings'

const props = defineProps<{ detail: ReviewHistoryDetail | undefined; loading: boolean; error: string }>()
const emit = defineEmits<{ close: []; retry: [] }>()
const closeButton = ref<HTMLButtonElement>()
const detailLayer = ref<HTMLElement>()
const mobile = ref(false)
const inertBackground = new Map<HTMLElement, boolean>()
const ratingCopy = { again: '忘记', hard: '困难', good: '记住', easy: '轻松' } as const
function dateTime(value: number | null) { return value == null ? '—' : new Date(value).toLocaleString('zh-CN', { dateStyle: 'medium', timeStyle: 'short' }) }
function duration(value: number | null) { return value == null ? '—' : value < 60_000 ? `${Math.max(1, Math.round(value / 1000))} 秒` : `${Math.round(value / 60_000)} 分钟` }
function metric(value: number | null) { return value == null ? '—' : value.toFixed(2) }
function setBackgroundInert() {
  const layer = detailLayer.value
  const page = layer?.closest('.history-page')
  const workspace = layer?.parentElement
  if (!layer || !page || !workspace) return
  const background = [
    ...Array.from(page.children).filter(child => child !== workspace),
    ...Array.from(workspace.children).filter(child => child !== layer),
  ].filter((element): element is HTMLElement => element instanceof HTMLElement)
  for (const element of background) {
    inertBackground.set(element, element.hasAttribute('inert'))
    element.setAttribute('inert', '')
  }
}
function restoreBackground() {
  for (const [element, wasInert] of inertBackground) {
    if (!wasInert) element.removeAttribute('inert')
  }
  inertBackground.clear()
}
function focusableElements() {
  return Array.from(detailLayer.value?.querySelectorAll<HTMLElement>('button:not([disabled]),a[href],input:not([disabled]),select:not([disabled]),textarea:not([disabled]),[tabindex]:not([tabindex="-1"])') ?? [])
}
function keydown(event: KeyboardEvent) {
  if (event.key === 'Escape') {
    event.preventDefault()
    emit('close')
    return
  }
  if (!mobile.value || event.key !== 'Tab') return
  const focusable = focusableElements()
  if (!focusable.length) return
  const first = focusable[0]!
  const last = focusable.at(-1)!
  if (event.shiftKey && document.activeElement === first) {
    event.preventDefault()
    last.focus()
  }
  else if (!event.shiftKey && document.activeElement === last) {
    event.preventDefault()
    first.focus()
  }
}
onMounted(() => {
  window.addEventListener('keydown', keydown)
  mobile.value = window.matchMedia?.('(max-width: 760px)').matches ?? false
  if (mobile.value) {
    setBackgroundInert()
    void nextTick(() => closeButton.value?.focus())
  }
})
onBeforeUnmount(() => {
  window.removeEventListener('keydown', keydown)
  restoreBackground()
})
</script>

<template>
  <div
    ref="detailLayer"
    class="detail-layer"
    @click.self="emit('close')"
  >
    <aside
      class="history-detail"
      :role="mobile ? 'dialog' : undefined"
      :aria-modal="mobile ? 'true' : undefined"
      aria-label="复习记录审计详情"
      :aria-busy="loading"
    >
      <header>
        <div><p>REVIEW EVENT</p><h2>复习审计</h2></div><button
          ref="closeButton"
          type="button"
          aria-label="关闭复习详情"
          @click="emit('close')"
        >
          <X :size="18" />
        </button>
      </header>
      <div
        v-if="loading"
        class="detail-state"
        role="status"
      >
        <span class="loading-mark" />正在读取不可变记录…
      </div>
      <div
        v-else-if="error"
        class="detail-state error"
        role="alert"
      >
        <span>{{ error }}</span><button
          type="button"
          @click="emit('retry')"
        >
          <RotateCcw :size="15" />重试
        </button>
      </div>
      <template v-else-if="props.detail">
        <section class="event-summary">
          <div :class="['large-seal',props.detail.rating]">
            {{ ratingCopy[props.detail.rating] }}
          </div><div><strong>{{ props.detail.subject || '未分类' }}</strong><span>{{ dateTime(props.detail.occurredAtUtcMs) }} · {{ duration(props.detail.durationMs) }}</span></div>
        </section>
        <section class="note-paper">
          <p>当时的题目笔记</p><div>{{ props.detail.note || '未填写笔记' }}</div><small>第 {{ props.detail.reviewOrdinal }} 次 / 共 {{ props.detail.problemReviewCount }} 次复习 · {{ props.detail.problemStatus === 'archived' ? '已归档题目' : props.detail.problemStatus === 'trashed' ? '回收站题目' : '在库题目' }}</small>
        </section>
        <section class="audit-section">
          <h3><History :size="17" />不可变事件事实</h3><dl>
            <div><dt>算法版本</dt><dd>{{ props.detail.algorithmVersion }} <span :class="props.detail.algorithmIsCurrent?'current':'legacy'">{{ props.detail.algorithmIsCurrent?'当前':'历史' }}</span></dd></div>
            <div><dt>参数版本</dt><dd>{{ props.detail.parameterVersion }} <span :class="props.detail.parametersAreCurrent?'current':'legacy'">{{ props.detail.parametersAreCurrent?'当前':'历史' }}</span></dd></div>
            <div><dt>提交设备</dt><dd><Monitor :size="15" />{{ props.detail.isCurrentDevice?'本机设备':'其他设备' }}</dd></div>
          </dl>
        </section>
        <section class="audit-section schedule-section">
          <h3><CalendarClock :size="17" />当前排程投影</h3><p class="projection-note">
            这是根据全部复习事件计算出的当前状态，不是当时的历史快照。
          </p>
          <dl v-if="props.detail.currentSchedule">
            <div><dt>下次到期</dt><dd>{{ dateTime(props.detail.currentSchedule.dueAtUtcMs) }}</dd></div><div><dt>稳定度</dt><dd>{{ metric(props.detail.currentSchedule.stability) }} 天</dd></div><div><dt>难度</dt><dd>{{ metric(props.detail.currentSchedule.difficulty) }}</dd></div><div><dt>投影引擎</dt><dd><Cpu :size="15" />{{ props.detail.currentSchedule.algorithmVersion }}</dd></div>
          </dl>
          <p
            v-else
            class="missing-schedule"
          >
            这道题目前没有排程投影。
          </p>
        </section>
      </template>
    </aside>
  </div>
</template>

<style scoped>
.detail-layer{min-width:0}.history-detail{position:sticky;top:24px;overflow:auto;max-height:calc(100vh - 48px);padding:22px;border:1px solid var(--line);border-radius:8px 24px 24px;background:rgba(255,253,247,.88);box-shadow:0 20px 58px rgba(34,48,43,.09)}.history-detail>header{display:flex;justify-content:space-between;align-items:flex-start}.history-detail header p{margin:0 0 4px;color:var(--cinnabar);font-size:9px;font-weight:850;letter-spacing:.15em}.history-detail h2{margin:0;color:var(--green-deep);font-family:var(--font-serif);font-size:23px}.history-detail header button{display:grid;width:44px;height:44px;padding:0;place-items:center;border:1px solid var(--line);border-radius:50%;background:transparent;cursor:pointer}.detail-state{display:grid;min-height:280px;place-items:center;align-content:center;gap:14px;color:var(--ink-muted);text-align:center}.detail-state.error button{display:inline-flex;gap:6px;min-height:44px;padding:0 16px;align-items:center;border:1px solid var(--line);border-radius:999px;background:var(--green-soft);cursor:pointer}.loading-mark{width:25px;height:25px;border:2px solid var(--sand-deep);border-top-color:var(--cinnabar);border-radius:50%;animation:spin .8s linear infinite}.event-summary{display:flex;gap:13px;align-items:center;margin-top:25px;padding-bottom:18px;border-bottom:1px solid var(--line)}.event-summary>div:last-child{display:grid;gap:5px}.event-summary strong{color:var(--green-deep);font-size:16px}.event-summary span{color:var(--ink-muted);font-size:11px}.large-seal{display:grid;width:48px;height:48px;place-items:center;color:white;border-radius:14px 4px 14px 14px;background:var(--green-deep);font-size:11px;font-weight:800;transform:rotate(-2deg)}.large-seal.again{background:var(--cinnabar)}.large-seal.hard{color:#77412f;background:#efd2b8}.large-seal.easy{background:#557263}.note-paper{margin-top:16px;padding:15px;border-radius:5px 15px 15px;background:rgba(232,221,199,.28)}.note-paper p{margin:0 0 8px;color:var(--cinnabar);font-size:9px;font-weight:800;letter-spacing:.08em}.note-paper div{color:var(--ink);font-size:13px;line-height:1.7;white-space:pre-wrap;overflow-wrap:anywhere}.note-paper small{display:block;margin-top:10px;color:var(--ink-muted);font-size:9px}.audit-section{margin-top:20px}.audit-section h3{display:flex;gap:7px;align-items:center;margin:0 0 11px;color:var(--green-deep);font-size:12px}.audit-section dl{display:grid;gap:8px;margin:0}.audit-section dl div{display:grid;grid-template-columns:90px minmax(0,1fr);gap:8px;padding:9px 10px;border-radius:9px;background:rgba(33,51,45,.04)}.audit-section dt{color:var(--ink-muted);font-size:10px}.audit-section dd{display:flex;gap:6px;align-items:center;justify-content:flex-end;min-width:0;margin:0;color:var(--ink);font-size:10px;overflow-wrap:anywhere;text-align:right}.audit-section dd span{padding:3px 6px;border-radius:999px;font-size:8px}.audit-section .current{color:#3f6857;background:var(--green-soft)}.audit-section .legacy{color:#884530;background:rgba(185,88,63,.12)}.projection-note,.missing-schedule{margin:-3px 0 11px;color:var(--ink-muted);font-size:9px;line-height:1.55}@keyframes spin{to{transform:rotate(360deg)}}@media(max-width:760px){.detail-layer{position:fixed;z-index:80;inset:0;display:flex;align-items:flex-end;background:rgba(34,48,43,.28);backdrop-filter:blur(3px);animation:fade-in var(--motion-standard) var(--ease-standard)}.history-detail{position:relative;top:auto;width:100%;max-height:min(82vh,760px);padding:20px 18px 28px;border:0;border-top:1px solid var(--line);border-radius:24px 24px 0 0;background:var(--paper);animation:sheet-in var(--motion-page) var(--ease-standard)}}@keyframes fade-in{from{opacity:0}}@keyframes sheet-in{from{opacity:0;transform:translateY(24px)}}@media(prefers-reduced-motion:reduce){.detail-layer,.history-detail,.loading-mark{animation:none}}
</style>
