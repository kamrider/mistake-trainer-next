<script setup lang="ts">
import { isTauri } from '@tauri-apps/api/core'
import { ArrowLeft, BookOpenCheck, RotateCcw } from '@lucide/vue'
import { computed, nextTick, onMounted, ref } from 'vue'
import { useRoute } from 'vue-router'
import ReviewHistoryDetail from '@/modules/review-history/components/ReviewHistoryDetail.vue'
import ReviewHistoryFilters, { type HistoryFiltersValue } from '@/modules/review-history/components/ReviewHistoryFilters.vue'
import ReviewHistoryTimeline from '@/modules/review-history/components/ReviewHistoryTimeline.vue'
import { commands, type ReviewHistoryDetail as HistoryDetail, type ReviewHistoryItem, type ReviewHistoryPage } from '@/shared/api/bindings'
import { normalizeAppResult } from '@/shared/api/normalize-result'

const route = useRoute()
const items = ref<ReviewHistoryItem[]>([])
const subjects = ref<string[]>([])
const nextCursor = ref<string | null>(null)
const totalCount = ref(0)
const loading = ref(true)
const loadingMore = ref(false)
const listError = ref('')
const stale = ref(false)
const selectedId = ref('')
const detail = ref<HistoryDetail>()
const detailLoading = ref(false)
const detailError = ref('')
const filters = ref<HistoryFiltersValue>({ range: '30_days', rating: null, subject: null, search: '' })
const failedRequest = ref<{ cursor: string | null; append: boolean }>()
let listEpoch = 0
let detailEpoch = 0
let detailTrigger: HTMLElement | undefined

const hasData = computed(() => items.value.length > 0)
const previewMode = computed(() => import.meta.env.DEV && route.query.preview === 'history' && !isTauri())

function previewPage(): ReviewHistoryPage {
  const now = Date.now()
  const values: ReviewHistoryItem[] = [
    { eventId: 'preview-3', subject: '数学', notePreview: '圆锥曲线焦点弦：重新检查离心率和长短轴关系', problemStatus: 'active', rating: 'good', durationMs: 76_000, occurredAtUtcMs: now - 32 * 60_000, algorithmVersion: 'fsrs-6.6.1', parameterVersion: 'default-6.6.1', algorithmIsCurrent: true, parametersAreCurrent: true },
    { eventId: 'preview-2', subject: '物理', notePreview: '带电粒子进入匀强磁场，方向判断仍然犹豫', problemStatus: 'archived', rating: 'again', durationMs: 128_000, occurredAtUtcMs: now - 3 * 60 * 60_000, algorithmVersion: 'fsrs-5.1', parameterVersion: 'legacy-2025', algorithmIsCurrent: false, parametersAreCurrent: false },
    { eventId: 'preview-1', subject: '英语', notePreview: '完形填空：转折后的语境线索', problemStatus: 'active', rating: 'easy', durationMs: 42_000, occurredAtUtcMs: now - 86_400_000, algorithmVersion: 'fsrs-6.6.1', parameterVersion: 'default-6.6.1', algorithmIsCurrent: true, parametersAreCurrent: true },
  ]
  return { items: values, nextCursor: null, totalCount: values.length, availableSubjects: ['数学', '物理', '英语'] }
}

function previewDetail(eventId: string): HistoryDetail {
  const item = items.value.find(candidate => candidate.eventId === eventId) ?? previewPage().items[0]!
  return { ...item, note: item.notePreview, isCurrentDevice: eventId !== 'preview-2', reviewOrdinal: eventId === 'preview-1' ? 5 : 2, problemReviewCount: eventId === 'preview-1' ? 5 : 4, currentSchedule: { dueAtUtcMs: Date.now() + 86_400_000, stability: 4.62, difficulty: 3.18, lastReviewedAtUtcMs: item.occurredAtUtcMs, algorithmVersion: 'fsrs-6.6.1', parameterVersion: 'default-6.6.1' } }
}

async function requestPage(cursor: string | null, append: boolean) {
  const epoch = ++listEpoch
  failedRequest.value = undefined
  if (append) loadingMore.value = true
  else loading.value = true
  listError.value = ''
  try {
    const result = previewMode.value
      ? { ok: true as const, data: previewPage() }
      : normalizeAppResult(await commands.reviewHistoryList({ ...filters.value, cursor, limit: 20 }))
    if (epoch !== listEpoch) return
    if (!result.ok) {
      listError.value = result.error.userMessage
      stale.value = hasData.value
      failedRequest.value = { cursor, append }
      return
    }
    if (append) items.value = [...items.value, ...result.data.items]
    else items.value = result.data.items
    subjects.value = result.data.availableSubjects
    nextCursor.value = result.data.nextCursor
    totalCount.value = result.data.totalCount
    stale.value = false
    failedRequest.value = undefined
  }
  catch {
    if (epoch === listEpoch) {
      listError.value = '复习历史暂时无法读取，请稍后重试。'
      stale.value = hasData.value
      failedRequest.value = { cursor, append }
    }
  }
  finally {
    if (epoch === listEpoch) {
      loading.value = false
      loadingMore.value = false
    }
  }
}

function applyFilters(value: HistoryFiltersValue) {
  filters.value = value
  closeDetail(false)
  void requestPage(null, false)
}
function resetFilters() {
  filters.value = { range: '30_days', rating: null, subject: null, search: '' }
  closeDetail(false)
  void requestPage(null, false)
}
function loadMore() { if (nextCursor.value && !loadingMore.value) void requestPage(nextCursor.value, true) }
function retryFailed() {
  const request = failedRequest.value ?? { cursor: null, append: false }
  void requestPage(request.cursor, request.append)
}

async function selectDetail(eventId: string, trigger?: HTMLElement) {
  detailTrigger = trigger ?? detailTrigger
  selectedId.value = eventId
  detail.value = undefined
  detailError.value = ''
  detailLoading.value = true
  const epoch = ++detailEpoch
  try {
    const result = previewMode.value
      ? { ok: true as const, data: previewDetail(eventId) }
      : normalizeAppResult(await commands.reviewHistoryDetail(eventId))
    if (epoch !== detailEpoch || selectedId.value !== eventId) return
    if (!result.ok) detailError.value = result.error.userMessage
    else detail.value = result.data
  }
  catch {
    if (epoch === detailEpoch) detailError.value = '这条复习记录暂时无法读取，请重试。'
  }
  finally { if (epoch === detailEpoch) detailLoading.value = false }
}

function closeDetail(restoreFocus = true) {
  detailEpoch += 1
  selectedId.value = ''
  detail.value = undefined
  detailError.value = ''
  detailLoading.value = false
  if (restoreFocus && detailTrigger) void nextTick(() => detailTrigger?.focus())
}

onMounted(() => requestPage(null, false))
</script>

<template>
  <main class="history-page">
    <header class="page-heading">
      <div><a href="#/report"><ArrowLeft :size="16" />返回学习报告</a><p>REVIEW HISTORY · 本地审计</p><h1>每一次记住与忘记，都有来路</h1><span>按真实复习事件查看评分、用时、算法版本和当前排程，不把历史记录改写成漂亮数字。</span></div>
      <div class="history-count">
        <BookOpenCheck :size="19" /><strong>{{ totalCount }}</strong><span>条匹配记录</span>
      </div>
    </header>
    <ReviewHistoryFilters
      :subjects="subjects"
      :loading="loading"
      @submit="applyFilters"
      @reset="resetFilters"
    />
    <p
      v-if="listError && hasData"
      class="error-banner"
      role="alert"
    >
      <span>{{ listError }}</span><button
        type="button"
        @click="retryFailed"
      >
        <RotateCcw :size="15" />重试
      </button>
    </p>
    <p
      v-if="stale"
      class="stale-note"
      role="status"
    >
      当前仍显示上一次成功读取的记录。
    </p>
    <section
      v-if="loading && !hasData"
      class="empty-state"
      aria-busy="true"
    >
      <span class="loading-mark" /><strong>正在整理时间线</strong><small>历史事件只从本机加密资料库读取。</small>
    </section>
    <section
      v-else-if="!hasData && listError"
      class="empty-state initial-error"
      role="alert"
    >
      <span class="empty-seal">!</span><strong>暂时无法读取复习历史</strong><small>{{ listError }}</small><button
        type="button"
        @click="retryFailed"
      >
        <RotateCcw :size="15" />重试
      </button>
    </section>
    <section
      v-else-if="!hasData"
      class="empty-state"
    >
      <span class="empty-seal">静</span><strong>这个范围内还没有复习记录</strong><small>完成一次训练评分后，真实事件会出现在这里。</small>
    </section>
    <div
      v-else
      :class="['history-workspace',{expanded:selectedId}]"
    >
      <ReviewHistoryTimeline
        :items="items"
        :selected-id="selectedId"
        :next-cursor="nextCursor"
        :loading-more="loadingMore"
        @select="selectDetail"
        @more="loadMore"
      />
      <ReviewHistoryDetail
        v-if="selectedId"
        :detail="detail"
        :loading="detailLoading"
        :error="detailError"
        @close="closeDetail"
        @retry="selectDetail(selectedId)"
      />
      <aside
        v-else
        class="detail-placeholder"
      >
        <span>审</span><strong>选择一条记录</strong><small>这里会显示不可变事件事实与当前排程投影。</small>
      </aside>
    </div>
  </main>
</template>

<style scoped>
.history-page{min-height:100vh;padding:38px clamp(22px,4.5vw,68px) 72px;background:radial-gradient(circle at 95% 0,rgba(185,88,63,.075),transparent 28rem)}.page-heading{display:flex;gap:24px;align-items:flex-end;justify-content:space-between;margin-bottom:22px}.page-heading a{display:inline-flex;gap:6px;align-items:center;min-height:44px;color:var(--ink-muted);font-size:11px;text-decoration:none}.page-heading p{margin:7px 0 7px;color:var(--cinnabar);font-size:10px;font-weight:850;letter-spacing:.16em}.page-heading h1{margin:0;color:var(--green-deep);font-family:var(--font-serif);font-size:clamp(29px,4vw,45px)}.page-heading>div>span{display:block;max-width:760px;margin-top:9px;color:var(--ink-muted);font-size:13px;line-height:1.6}.history-count{display:grid;min-width:122px;padding:14px;grid-template-columns:auto 1fr;gap:2px 8px;align-items:center;border:1px solid var(--line);border-radius:14px 5px 14px 14px;background:rgba(255,253,247,.68)}.history-count svg{grid-row:1/3;color:var(--cinnabar)}.history-count strong{color:var(--green-deep);font-family:var(--font-serif);font-size:24px}.history-count span{color:var(--ink-muted);font-size:9px}.history-workspace{display:grid;grid-template-columns:minmax(0,1.55fr) minmax(280px,.75fr);gap:16px;align-items:start;margin-top:22px}.detail-placeholder{position:sticky;top:24px;display:grid;min-height:310px;padding:28px;place-items:center;align-content:center;color:var(--ink-muted);border:1px dashed var(--sand-deep);border-radius:8px 24px 24px;text-align:center}.detail-placeholder span{display:grid;width:50px;height:50px;margin-bottom:15px;place-items:center;color:white;border-radius:15px 4px 15px 15px;background:var(--cinnabar);font-family:var(--font-serif);font-size:21px;transform:rotate(-3deg)}.detail-placeholder strong{color:var(--green-deep)}.detail-placeholder small{max-width:220px;margin-top:8px;line-height:1.6}.error-banner{display:flex;gap:12px;align-items:center;justify-content:space-between;margin:14px 0 0;padding:11px 13px;color:#843d2c;border:1px solid rgba(185,88,63,.25);border-radius:11px;background:rgba(185,88,63,.08);font-size:11px}.error-banner button{display:inline-flex;gap:6px;align-items:center;min-height:38px;padding:0 12px;color:#843d2c;border:1px solid rgba(185,88,63,.25);border-radius:999px;background:transparent;cursor:pointer}.stale-note{margin:8px 0 0;color:var(--ink-muted);font-size:10px}.empty-state{display:grid;min-height:390px;margin-top:22px;place-items:center;align-content:center;color:var(--ink-muted);border:1px dashed var(--sand-deep);border-radius:8px 24px 24px;background:rgba(255,253,247,.34);text-align:center}.empty-state strong{margin-top:13px;color:var(--green-deep);font-family:var(--font-serif);font-size:19px}.empty-state small{margin-top:7px}.empty-seal{display:grid;width:52px;height:52px;place-items:center;color:white;border-radius:15px 4px 15px 15px;background:var(--green-deep);font-family:var(--font-serif);font-size:21px}.loading-mark{width:28px;height:28px;border:2px solid var(--sand-deep);border-top-color:var(--cinnabar);border-radius:50%;animation:spin .8s linear infinite}@keyframes spin{to{transform:rotate(360deg)}}@media(max-width:960px){.history-workspace{grid-template-columns:minmax(0,1.3fr) minmax(260px,.7fr)}}@media(max-width:760px){.history-page{padding:24px 15px 92px}.page-heading{align-items:flex-start}.history-count{display:none}.history-workspace{display:block}.detail-placeholder{display:none}}@media(max-width:460px){.page-heading h1{font-size:29px}.page-heading>div>span{font-size:12px}}@media(prefers-reduced-motion:reduce){.loading-mark{animation:none}}
.error-banner button,.initial-error button{min-height:44px;justify-content:center;padding-inline:14px}.initial-error button{margin-top:18px}.initial-error .empty-seal{background:var(--cinnabar)}
</style>
