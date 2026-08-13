<script setup lang="ts">
import { Check, ChevronLeft, ChevronRight, Pencil, SkipForward, Sparkles, X } from '@lucide/vue'
import { computed, nextTick, onBeforeUnmount, onMounted, ref, watch } from 'vue'
import type {
  CaptureRecognitionJob,
  CaptureRecognitionReasonCode,
  CaptureRecognitionSuggestion,
} from '../../../shared/api/bindings'
import { acquireDialogDocumentBoundary } from '@/shared/ui/dialog-document-boundary'
import { trapDialogFocus } from '@/shared/ui/dialog-focus'
import {
  useCaptureRecognitionReviewSession,
  type CaptureRecognitionReviewFilter,
} from '../composables/useCaptureRecognitionReviewSession'

const props = defineProps<{
  job: CaptureRecognitionJob
  previews: Record<string, string>
  busy?: boolean | undefined
  operationBusy?: boolean | undefined
}>()

const emit = defineEmits<{
  review: [input: {
    jobId: string
    suggestionId: string
    decision: 'accepted' | 'rejected'
    editedRegions: null
  }]
  reviewMany: [inputs: Array<{
    jobId: string
    suggestionId: string
    decision: 'accepted'
    editedRegions: null
  }>]
  edit: [suggestion: CaptureRecognitionSuggestion]
  preview: [itemId: string]
  applyAccepted: [suggestionIds: string[]]
  close: []
}>()

const announcement = ref('')
const confirmApply = ref(false)
const reviewRoot = ref<HTMLElement>()
const applyButton = ref<HTMLButtonElement>()
const impactDialog = ref<HTMLElement>()
let announcementToken = 0
let releaseDialogBoundary: (() => void) | undefined

const reasonCopy: Record<CaptureRecognitionReasonCode, string> = {
  clear_question_anchor: '检测到清晰题号',
  matched_question_answer_anchor: '题答编号匹配',
  consistent_reading_order: '版面顺序清晰',
  weak_anchor: '题号不够清晰',
  ambiguous_columns: '分栏顺序需要检查',
  possible_content_cut: '边界附近可能还有内容',
}

const {
  filter,
  currentIndex,
  counts,
  filtered,
  current,
  acceptedIds,
  decisionState,
  recordDecision,
  recordAcceptedMany,
  move: moveSession,
  selectFilter: selectSessionFilter,
} = useCaptureRecognitionReviewSession(() => props.job)
const currentQuestions = computed(() => current.value?.regions.filter(region => region.role === 'question').length ?? 0)
const currentAnswers = computed(() => current.value?.regions.filter(region => region.role === 'answer').length ?? 0)
const acceptedSuggestions = computed(() => props.job.suggestions.filter(item => acceptedIds.value.includes(item.id)))
const impact = computed(() => {
  const regions = acceptedSuggestions.value.flatMap(item => item.regions)
  return {
    sources: acceptedSuggestions.value.length,
    questions: regions.filter(region => region.role === 'question').length,
    answers: regions.filter(region => region.role === 'answer').length,
    unchanged: props.job.suggestions.length - acceptedSuggestions.value.length,
  }
})

watch(
  () => current.value?.itemId,
  (itemId) => {
    if (itemId && !props.previews[itemId]) emit('preview', itemId)
  },
  { immediate: true },
)

onMounted(async () => {
  if (reviewRoot.value) releaseDialogBoundary = acquireDialogDocumentBoundary(reviewRoot.value)
  await nextTick()
  reviewRoot.value?.focus()
})

onBeforeUnmount(() => {
  releaseDialogBoundary?.()
})

function canAccept(suggestion: CaptureRecognitionSuggestion) {
  return suggestion.reviewBand !== 'low' && suggestion.state !== 'stale'
}

function announce(message: string) {
  const token = ++announcementToken
  // Clearing first ensures Narrator announces identical consecutive decisions.
  announcement.value = ''
  void nextTick(() => {
    if (token === announcementToken) announcement.value = message
  })
}

function review(suggestion: CaptureRecognitionSuggestion, decision: 'accepted' | 'rejected') {
  if (props.operationBusy) return
  if (decision === 'accepted' && !canAccept(suggestion)) return
  const reviewedPosition = currentIndex.value + 1
  recordDecision(suggestion.id, decision)
  announce(
    `${decision === 'accepted' ? '已接受' : '已跳过'}第 ${reviewedPosition} 条建议，共 ${filtered.value.length} 条`,
  )
  emit('review', {
    jobId: props.job.id,
    suggestionId: suggestion.id,
    decision,
    editedRegions: null,
  })
  move(1, false)
}

function acceptHighConfidence() {
  if (props.operationBusy) return
  const inputs: Array<{
    jobId: string
    suggestionId: string
    decision: 'accepted'
    editedRegions: null
  }> = []
  const acceptedSuggestionIds: string[] = []
  for (const suggestion of props.job.suggestions) {
    if (
      suggestion.reviewBand === 'high'
      && suggestion.state !== 'stale'
      && decisionState(suggestion) !== 'accepted'
    ) {
      acceptedSuggestionIds.push(suggestion.id)
      inputs.push({
        jobId: props.job.id,
        suggestionId: suggestion.id,
        decision: 'accepted',
        editedRegions: null,
      })
    }
  }
  recordAcceptedMany(acceptedSuggestionIds)
  if (inputs.length) emit('reviewMany', inputs)
  announce(`已接受 ${inputs.length} 条高可信建议；其他建议保持不变`)
}

function move(offset: number, announcePosition = true) {
  if (!filtered.value.length) return
  const nextIndex = moveSession(offset)
  if (announcePosition) announce(`第 ${nextIndex + 1} 条，共 ${filtered.value.length} 条`)
}

function selectFilter(nextFilter: CaptureRecognitionReviewFilter) {
  selectSessionFilter(nextFilter)
  announce(`已打开${{
    review: '需要检查',
    high: '可快速确认',
    low: '无法安全切分',
    stale: '已过期',
  }[nextFilter]}，${counts.value[nextFilter]} 条`)
}

function ignoreStale() {
  announce('已忽略这条过期建议')
  move(1, false)
}

async function openImpact() {
  confirmApply.value = true
  await nextTick()
  impactDialog.value?.focus()
}

async function closeImpact() {
  confirmApply.value = false
  await nextTick()
  applyButton.value?.focus()
}

function confirmAccepted() {
  emit('applyAccepted', acceptedIds.value)
  confirmApply.value = false
}

function handleImpactKeydown(event: KeyboardEvent) {
  if (event.key === 'Escape') {
    event.preventDefault()
    void closeImpact()
    return
  }
  trapDialogFocus(event, impactDialog.value)
}

function handleKeydown(event: KeyboardEvent) {
  if (event.key === 'Escape' && !confirmApply.value) {
    event.preventDefault()
    emit('close')
    return
  }
  if (event.key === 'Tab') {
    trapDialogFocus(event, reviewRoot.value)
    return
  }
  const target = event.target
  if (
    target instanceof HTMLInputElement
    || target instanceof HTMLTextAreaElement
    || target instanceof HTMLSelectElement
    || (target instanceof HTMLElement && target.isContentEditable)
  ) return
  if (event.key.toLowerCase() === 'j') {
    event.preventDefault()
    move(1)
  }
  else if (event.key.toLowerCase() === 'k') {
    event.preventDefault()
    move(-1)
  }
  else if (
    event.key === 'Enter'
    && !props.operationBusy
    && current.value
    && canAccept(current.value)
  ) {
    event.preventDefault()
    review(current.value, 'accepted')
  }
  else if (event.key.toLowerCase() === 's' && !props.operationBusy && current.value) {
    event.preventDefault()
    review(current.value, 'rejected')
  }
  else if (
    event.key.toLowerCase() === 'e'
    && !props.busy
    && !props.operationBusy
    && current.value
    && canAccept(current.value)
  ) {
    event.preventDefault()
    emit('edit', current.value)
  }
}

function asPercent(value: number | null) {
  return `${(value ?? 0) * 100}%`
}
</script>

<template>
  <section
    ref="reviewRoot"
    class="recognition-review"
    role="dialog"
    aria-modal="true"
    tabindex="-1"
    aria-labelledby="recognition-review-title"
    aria-describedby="recognition-review-help"
    @keydown="handleKeydown"
  >
    <header>
      <div>
        <p class="eyebrow">
          智能切图建议
        </p>
        <h2 id="recognition-review-title">
          快速确认，不替你做决定
        </h2>
        <p id="recognition-review-help">
          J / K 切换，Enter 接受，E 调整边界，S 跳过。
        </p>
      </div>
      <button
        type="button"
        class="icon-button"
        aria-label="关闭识别建议"
        @click="emit('close')"
      >
        <X :size="18" />
      </button>
    </header>

    <nav aria-label="识别建议分类">
      <button
        v-for="option in ([
          ['review', '需要检查'],
          ['high', '可快速确认'],
          ['low', '无法安全切分'],
          ['stale', '已过期'],
        ] as const)"
        :key="option[0]"
        type="button"
        :class="{ active: filter === option[0] }"
        :aria-pressed="filter === option[0]"
        @click="selectFilter(option[0])"
      >
        {{ option[1] }} <strong>{{ counts[option[0]] }}</strong>
      </button>
    </nav>

    <p
      v-if="busy && !operationBusy"
      class="review-save-state"
      aria-live="polite"
    >
      正在后台保存审核决定；你可以继续确认下一条，应用切图需等待保存完成。
    </p>

    <div
      v-if="current"
      class="review-card"
    >
      <div
        class="preview-frame"
        role="group"
        :aria-label="`候选区域预览：${current.regions.filter(region => region.role === 'question').length} 个题目区域，${current.regions.filter(region => region.role === 'answer').length} 个答案区域`"
      >
        <img
          v-if="previews[current.itemId]"
          :src="previews[current.itemId]"
          alt="原始题图预览"
        >
        <div
          v-else
          class="preview-placeholder"
        >
          <span>正在加载原图预览</span>
          <button
            type="button"
            @click="emit('preview', current.itemId)"
          >
            重新加载
          </button>
        </div>
        <span
          v-for="(region, index) in current.regions"
          :key="index"
          class="region-overlay"
          :class="`is-${region.role}`"
          aria-hidden="true"
          :style="{
            left: asPercent(region.rect.x),
            top: asPercent(region.rect.y),
            width: asPercent(region.rect.width),
            height: asPercent(region.rect.height),
          }"
        >{{ region.role === 'question' ? '题' : '答' }}</span>
      </div>

      <div class="review-copy">
        <div class="review-position">
          <button
            type="button"
            aria-label="上一条建议"
            :disabled="currentIndex === 0"
            @click="move(-1)"
          >
            <ChevronLeft :size="17" />
          </button>
          <span>{{ currentIndex + 1 }} / {{ filtered.length }}</span>
          <button
            type="button"
            aria-label="下一条建议"
            :disabled="currentIndex >= filtered.length - 1"
            @click="move(1)"
          >
            <ChevronRight :size="17" />
          </button>
        </div>
        <span class="confidence">{{ Math.round(current.confidenceBasisPoints / 100) }}% 参考置信度</span>
        <h3>
          {{ current.reviewBand === 'low'
            ? '无法确认安全边界'
            : currentQuestions > 1
              ? `这张原图将拆成 ${currentQuestions} 张题图`
              : `${current.regions.length} 个候选区域` }}
        </h3>
        <p class="split-note">
          原图始终保留；应用后每个区域都会成为素材牌库里可单独整理的图片。
          <template v-if="currentAnswers">
            同时识别到 {{ currentAnswers }} 个答案区域。
          </template>
        </p>
        <ul>
          <li
            v-for="reason in current.reasonCodes"
            :key="reason"
          >
            {{ reasonCopy[reason] }}
          </li>
        </ul>
        <p
          v-if="current.reviewBand === 'low'"
          class="caution"
        >
          依据不足，已默认跳过。你仍可继续手工整理。
        </p>
        <p
          v-if="current.state === 'stale'"
          class="caution"
        >
          图片已被移动或修改，这条建议不会再应用。
        </p>
        <div
          v-if="current.state === 'stale'"
          class="review-actions"
        >
          <button
            type="button"
            :disabled="operationBusy"
            @click="ignoreStale"
          >
            <SkipForward :size="15" /> 忽略已过期建议
          </button>
        </div>
        <div
          v-else-if="current.reviewBand === 'low'"
          class="review-actions"
        >
          <button
            type="button"
            class="primary-action"
            :disabled="operationBusy"
            @click="review(current, 'rejected')"
          >
            <Check :size="16" /> 保留原图
          </button>
          <button
            type="button"
            :disabled="busy || operationBusy"
            :data-recognition-edit-suggestion-id="current.id"
            @click="emit('edit', current)"
          >
            <Pencil :size="15" /> 手工裁剪
          </button>
        </div>
        <div
          v-else
          class="review-actions"
        >
          <button
            type="button"
            class="primary-action"
            :disabled="operationBusy"
            @click="review(current, 'accepted')"
          >
            <Check :size="16" /> 接受建议
          </button>
          <button
            type="button"
            :disabled="busy || operationBusy"
            :data-recognition-edit-suggestion-id="current.id"
            @click="emit('edit', current)"
          >
            <Pencil :size="15" /> 调整边界
          </button>
          <button
            type="button"
            :disabled="operationBusy"
            @click="review(current, 'rejected')"
          >
            <SkipForward :size="15" /> 跳过
          </button>
        </div>
        <p
          v-if="decisionState(current) === 'accepted'"
          class="decision"
        >
          已接受
        </p>
        <p
          v-else-if="decisionState(current) === 'rejected'"
          class="decision"
        >
          已跳过
        </p>
      </div>
    </div>

    <div
      v-else
      class="empty-filter"
    >
      这一类没有待处理建议。
    </div>

    <footer>
      <button
        v-if="counts.high > 0"
        type="button"
        class="secondary-action"
        :disabled="operationBusy"
        @click="acceptHighConfidence"
      >
        <Sparkles :size="15" /> 仅接受全部高可信建议
      </button>
      <button
        v-if="acceptedIds.length > 0"
        ref="applyButton"
        type="button"
        class="primary-action"
        :disabled="busy || operationBusy"
        @click="openImpact"
      >
        把切图放入素材牌库（{{ acceptedIds.length }}）
      </button>
      <p
        v-else
        class="apply-hint"
      >
        接受至少一条建议后，切图才会进入素材牌库。
      </p>
    </footer>

    <div
      v-if="confirmApply"
      ref="impactDialog"
      class="impact"
      role="alertdialog"
      aria-modal="true"
      aria-labelledby="recognition-impact-title"
      tabindex="-1"
      @keydown.stop="handleImpactKeydown"
    >
      <h3 id="recognition-impact-title">
        确认本次改动
      </h3>
      <ul>
        <li>{{ impact.sources }} 张原图会保留</li>
        <li>{{ impact.questions }} 张题图会进入素材牌库</li>
        <li>{{ impact.answers }} 张答案图会进入素材牌库</li>
        <li>不会自动新建或改动题卡</li>
        <li>{{ impact.unchanged }} 条过期或跳过建议保持不变</li>
      </ul>
      <div class="recognition-actions">
        <button
          type="button"
          class="primary-action"
          @click="confirmAccepted"
        >
          放入素材牌库
        </button>
        <button
          type="button"
          @click="closeImpact"
        >
          返回检查
        </button>
      </div>
    </div>

    <p
      class="sr-only"
      role="status"
      aria-live="polite"
      aria-atomic="true"
    >
      {{ announcement }}
    </p>
  </section>
</template>

<style scoped>
.recognition-review {
  position: fixed;
  z-index: 90;
  top: 24px;
  left: 50%;
  display: grid;
  width: min(1080px, calc(100vw - 48px));
  max-height: calc(100vh - 48px);
  padding: 22px;
  overflow: hidden;
  box-sizing: border-box;
  border: 1px solid rgba(33, 51, 45, .2);
  border-radius: 18px;
  background: rgba(255, 253, 247, .94);
  box-shadow: 0 0 0 9999px rgba(25, 35, 31, .46), 0 24px 70px rgba(20, 31, 27, .28);
  transform: translateX(-50%);
  grid-template-rows: auto auto minmax(0, 1fr) auto;
  backdrop-filter: blur(12px);
}

header,
footer,
.review-actions,
.recognition-actions,
.review-position,
nav {
  display: flex;
  gap: 10px;
  align-items: center;
}

header,
footer {
  justify-content: space-between;
}

h2,
h3,
p {
  margin: 0;
}

.eyebrow {
  color: #4f806e;
  font-size: 12px;
  font-weight: 850;
  letter-spacing: .1em;
}

header p:not(.eyebrow) {
  margin-top: 4px;
  color: var(--ink-muted);
  font-size: 12px;
}

.icon-button {
  display: grid;
  width: 44px;
  height: 44px;
  padding: 0;
  place-items: center;
  border-radius: 50%;
}

nav {
  margin: 18px 0;
  overflow-x: auto;
}

nav button {
  white-space: nowrap;
}

nav button.active {
  color: var(--paper);
  border-color: var(--green-deep);
  background: var(--green-deep);
}

.review-card {
  display: grid;
  grid-template-columns: minmax(260px, 42%) 1fr;
  gap: 24px;
  min-height: 0;
  overflow: auto;
}

.preview-frame {
  position: relative;
  min-height: 280px;
  overflow: hidden;
  border-radius: 14px;
  background: #e9e4d8;
}

.preview-frame img {
  display: block;
  width: 100%;
  height: auto;
}

.preview-placeholder {
  display: grid;
  min-height: 280px;
  place-items: center;
  align-content: center;
  gap: 10px;
  color: var(--ink-muted);
}

.preview-placeholder button {
  min-height: 44px;
}

.region-overlay {
  position: absolute;
  display: grid;
  place-items: start;
  padding: 3px;
  color: white;
  border: 2px solid #3d7764;
  background: rgba(61, 119, 100, .12);
  font-size: 12px;
  font-weight: 900;
}

.region-overlay.is-answer {
  border-color: #bd694f;
  background: rgba(189, 105, 79, .12);
}

.review-copy {
  min-width: 0;
}

.review-position {
  justify-content: flex-end;
}

.review-position button {
  display: grid;
  width: 44px;
  height: 44px;
  padding: 0;
  place-items: center;
}

.confidence {
  display: inline-block;
  margin: 12px 0 5px;
  color: var(--ink-muted);
  font-size: 12px;
}

.review-copy ul,
.impact ul {
  padding-left: 20px;
  color: var(--ink-muted);
  font-size: 12px;
}

.split-note {
  margin-top: 7px;
  color: var(--ink-muted);
  font-size: 12px;
  line-height: 1.55;
}

.caution {
  margin: 12px 0;
  padding: 10px 12px;
  color: #7b493b;
  border-radius: 10px;
  background: rgba(246, 226, 216, .65);
  font-size: 12px;
}

.decision {
  margin-top: 10px;
  color: #315f50;
  font-size: 12px;
  font-weight: 800;
}

button {
  min-height: 44px;
  padding: 0 13px;
  border: 1px solid rgba(33, 51, 45, .2);
  border-radius: 999px;
  background: rgba(255, 253, 247, .82);
  cursor: pointer;
}

.primary-action,
.secondary-action,
.review-actions button {
  display: inline-flex;
  gap: 7px;
  align-items: center;
}

.primary-action {
  color: var(--paper);
  border-color: var(--green-deep);
  background: var(--green-deep);
}

button:disabled {
  opacity: .48;
  cursor: not-allowed;
}

footer {
  margin-top: 20px;
  padding-top: 16px;
  border-top: 1px solid rgba(33, 51, 45, .12);
}

.apply-hint {
  max-width: 320px;
  color: var(--ink-muted);
  font-size: 12px;
  text-align: right;
}

.review-save-state {
  margin: 12px 0 0;
  padding: 9px 12px;
  color: var(--green-deep);
  border: 1px solid rgba(79, 128, 110, .22);
  border-radius: 10px;
  background: rgba(225, 235, 229, .58);
  font-size: 12px;
}

.impact {
  margin-top: 16px;
  padding: 18px;
  border: 1px solid rgba(79, 128, 110, .3);
  border-radius: 14px;
  background: rgba(230, 239, 233, .72);
}

.empty-filter {
  padding: 32px;
  color: var(--ink-muted);
  text-align: center;
}

.sr-only {
  position: absolute;
  width: 1px;
  height: 1px;
  padding: 0;
  overflow: hidden;
  clip: rect(0, 0, 0, 0);
  white-space: nowrap;
  border: 0;
}

@media (max-width: 760px) {
  .recognition-review {
    top: 10px;
    width: calc(100vw - 20px);
    max-height: calc(100vh - 20px);
    padding: 16px;
  }

  .review-card {
    grid-template-columns: 1fr;
  }

  footer,
  .review-actions {
    align-items: stretch;
    flex-direction: column;
  }
}
</style>
