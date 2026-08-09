<script setup lang="ts">
import { ArrowLeft, ChevronLeft, ChevronRight, ClipboardCheck, Clock3, Eye, Expand, RotateCcw, Sparkles } from '@lucide/vue'
import { computed, nextTick, onBeforeUnmount, onMounted, ref, watch } from 'vue'
import type { FsrsRating, SimpleRating } from '../domain/rating'
import ReviewMediaLightbox from './ReviewMediaLightbox.vue'

const props = defineProps<{
  subject: string
  prompt: string
  answer: string
  questionImages?: string[]
  answerImages?: string[]
  current: number
  total: number
  elapsedText: string
  mode?: 'due' | 'manual' | string
  examPhase?: 'answering' | 'grading' | null
  timeLimitSeconds?: number | null
  expired?: boolean
  resumed?: boolean
  submitting?: boolean
}>()

const emit = defineEmits<{
  exit: []
  reveal: []
  rate: [rating: SimpleRating | FsrsRating]
  previous: []
  next: []
  beginGrading: []
}>()

const revealed = ref(props.mode === 'exam' && props.examPhase === 'grading')
const advanced = ref(false)
const cardDirection = ref<'forward' | 'backward'>('forward')
const headingElement = ref<HTMLElement>()
const lightbox = ref<{ images: string[]; index: number; label: string }>()
type HeadingFocusRequest = { stageKey: string; previousActive: Element | null }
const pendingHeadingFocus = ref<HeadingFocusRequest>()
const isExamAnswering = computed(() => props.mode === 'exam' && props.examPhase === 'answering')
const isExamGrading = computed(() => props.mode === 'exam' && props.examPhase === 'grading')
const stageKey = computed(() => `${props.current}-${props.examPhase || 'review'}`)
const progress = computed(() => {
  if (props.total <= 0) return '0%'
  return `${Math.round((Math.min(props.current, props.total) / props.total) * 100)}%`
})
const modeLabel = computed(() => {
  if (isExamAnswering.value) return '模拟考试 · 独立作答'
  if (isExamGrading.value) return '模拟考试 · 核对答案'
  return props.mode === 'manual' ? '自选训练' : '到期复习'
})
const heading = computed(() => {
  if (isExamAnswering.value) return '先独立完成整组，再统一看答案'
  if (isExamGrading.value) return '对照答案，诚实判定'
  return '先想一遍，再看答案'
})
const cardTransitionName = computed(() => cardDirection.value === 'backward'
  ? 'card-shift-backward'
  : 'card-shift-forward')

function resolveHeadingFocus() {
  const request = pendingHeadingFocus.value
  const heading = headingElement.value
  if (!request || !heading) return
  const mountedStageKey = heading.closest<HTMLElement>('[data-review-stage-key]')?.dataset.reviewStageKey
  if (mountedStageKey !== request.stageKey) return

  const active = document.activeElement
  if (
    active !== request.previousActive
    && active !== document.body
    && active !== document.documentElement
  ) {
    pendingHeadingFocus.value = undefined
    return
  }
  heading.focus({ preventScroll: true })
  pendingHeadingFocus.value = undefined
}

function requestHeadingFocus() {
  pendingHeadingFocus.value = {
    stageKey: stageKey.value,
    previousActive: document.activeElement,
  }
  void nextTick(resolveHeadingFocus)
}

watch([() => props.current, () => props.examPhase], () => {
  revealed.value = isExamGrading.value
  lightbox.value = undefined
  requestHeadingFocus()
})

function revealAnswer() {
  if (revealed.value || props.submitting) return
  revealed.value = true
  emit('reveal')
}

function submit(rating: SimpleRating | FsrsRating) {
  if (!revealed.value || props.submitting) return
  emit('rate', rating)
}

function previousCard() {
  if (!isExamAnswering.value || props.current <= 1 || props.submitting) return
  cardDirection.value = 'backward'
  emit('previous')
}

function nextCard() {
  if (!isExamAnswering.value || props.current >= props.total || props.submitting) return
  cardDirection.value = 'forward'
  emit('next')
}

function beginGrading() {
  if (!isExamAnswering.value || props.current !== props.total || props.submitting) return
  cardDirection.value = 'forward'
  emit('beginGrading')
}

function inspect(images: string[], index: number, label: string) {
  if (images.length === 0) return
  lightbox.value = { images, index, label }
}

function isEditableTarget(target: EventTarget | null) {
  return target instanceof HTMLElement
    && (target.isContentEditable || target.matches('input, textarea, select, [role="textbox"]'))
}

function handleShortcut(event: KeyboardEvent) {
  if (lightbox.value || event.defaultPrevented || event.ctrlKey || event.metaKey || event.altKey)
    return
  if (isEditableTarget(event.target)) return
  if (event.key === 'Escape') {
    event.preventDefault()
    if (props.submitting) return
    emit('exit')
    return
  }
  if (isExamAnswering.value) {
    if (event.key === 'ArrowLeft' && props.current > 1) {
      event.preventDefault()
      previousCard()
    }
    else if (event.key === 'ArrowRight' && props.current < props.total) {
      event.preventDefault()
      nextCard()
    }
    else if ((event.key === 'Enter' || event.key === ' ') && props.current === props.total) {
      event.preventDefault()
      beginGrading()
    }
    return
  }
  if (!revealed.value) {
    if (event.key === ' ' || event.key === 'Enter') {
      event.preventDefault()
      revealAnswer()
    }
    return
  }
  if (props.submitting) return

  if (!advanced.value) {
    const rating: Record<string, SimpleRating> = { 1: 'forgot', 2: 'remembered' }
    const selected = rating[event.key]
    if (selected) {
      event.preventDefault()
      submit(selected)
    }
    return
  }

  const rating: Record<string, FsrsRating> = {
    1: 'again',
    2: 'hard',
    3: 'good',
    4: 'easy',
    a: 'again',
    s: 'hard',
    d: 'good',
    f: 'easy',
  }
  const selected = rating[event.key.toLowerCase()]
  if (selected) {
    event.preventDefault()
    submit(selected)
  }
}

onMounted(() => {
  window.addEventListener('keydown', handleShortcut)
  requestHeadingFocus()
})
onBeforeUnmount(() => window.removeEventListener('keydown', handleShortcut))
</script>

<template>
  <main
    class="review-room"
    aria-labelledby="review-heading"
  >
    <header class="review-header">
      <button
        class="icon-action"
        type="button"
        :aria-label="submitting ? '正在保存训练进度' : '退出训练'"
        :disabled="submitting"
        @click="emit('exit')"
      >
        <ArrowLeft :size="19" />
      </button>

      <div class="review-progress">
        <span>{{ current }} / {{ total }}</span>
        <div
          class="progress-track"
          aria-hidden="true"
        >
          <i :style="{ width: progress }" />
        </div>
      </div>

      <div class="header-chips">
        <span
          v-if="resumed"
          class="resume-chip"
        >已恢复上次进度</span>
        <span
          class="clock-chip"
          :class="{ expired }"
        >
          <Clock3 :size="14" />
          {{ timeLimitSeconds ? '剩余' : '用时' }} {{ elapsedText }}
        </span>
        <span class="subject-chip">{{ subject }}</span>
      </div>
    </header>

    <Transition
      :name="cardTransitionName"
      mode="out-in"
      @after-enter="resolveHeadingFocus"
    >
      <section
        :key="stageKey"
        class="review-stage"
        :data-review-stage-key="stageKey"
      >
        <p class="stage-kicker">
          <Sparkles
            :size="15"
            aria-hidden="true"
          />
          {{ modeLabel }}
        </p>
        <h1
          id="review-heading"
          ref="headingElement"
          tabindex="-1"
        >
          {{ heading }}
        </h1>

        <article class="problem-paper">
          <span class="paper-label">题目</span>
          <div
            v-if="questionImages?.length"
            class="review-images"
          >
            <button
              v-for="(source, index) in questionImages"
              :key="`${source}-${index}`"
              type="button"
              class="media-button"
              :aria-label="`放大题目图片 ${index + 1}`"
              @click="inspect(questionImages, index, '题目图片')"
            >
              <img
                :src="source"
                :alt="`训练题图 ${index + 1}`"
              >
              <span><Expand :size="15" />查看大图</span>
            </button>
          </div>
          <p v-else>
            {{ prompt }}
          </p>
        </article>

        <p
          v-if="expired && !revealed"
          class="expired-notice"
          role="status"
        >
          已到建议时限，仍可继续作答
        </p>

        <div
          v-if="isExamAnswering"
          class="exam-navigation"
          aria-label="考试题目导航"
        >
          <button
            type="button"
            class="exam-nav-action"
            aria-label="上一题"
            :disabled="current <= 1 || submitting"
            @click="previousCard"
          >
            <ChevronLeft
              :size="19"
              aria-hidden="true"
            />
            上一题
            <kbd>←</kbd>
          </button>
          <button
            v-if="current < total"
            type="button"
            class="exam-nav-action exam-next-action"
            aria-label="下一题"
            :disabled="submitting"
            @click="nextCard"
          >
            下一题
            <ChevronRight
              :size="19"
              aria-hidden="true"
            />
            <kbd>→</kbd>
          </button>
          <button
            v-else
            type="button"
            class="exam-nav-action begin-grading-action"
            aria-label="开始核对答案"
            :disabled="submitting"
            @click="beginGrading"
          >
            <ClipboardCheck
              :size="19"
              aria-hidden="true"
            />
            {{ submitting ? '正在切换…' : '开始核对答案' }}
            <kbd>Enter</kbd>
          </button>
        </div>

        <button
          v-else-if="!revealed"
          class="reveal-action"
          type="button"
          aria-label="显示答案"
          :disabled="submitting"
          @click="revealAnswer"
        >
          <Eye
            :size="19"
            aria-hidden="true"
          />
          显示答案
          <kbd>Space</kbd>
        </button>

        <Transition
          name="paper-turn"
          appear
        >
          <div
            v-if="revealed"
            class="answer-area"
          >
            <article
              class="answer-paper"
              aria-live="polite"
            >
              <span class="paper-label">答案</span>
              <div
                v-if="answerImages?.length"
                class="review-images"
              >
                <button
                  v-for="(source, index) in answerImages"
                  :key="`${source}-${index}`"
                  type="button"
                  class="media-button"
                  :aria-label="`放大答案图片 ${index + 1}`"
                  @click="inspect(answerImages, index, '答案图片')"
                >
                  <img
                    :src="source"
                    :alt="`训练答案图 ${index + 1}`"
                  >
                  <span><Expand :size="15" />查看大图</span>
                </button>
              </div>
              <p v-else>
                {{ answer }}
              </p>
            </article>

            <div class="rating-heading">
              <div>
                <p>{{ isExamGrading ? '这道题实际答对了吗？' : '这次你记得怎么样？' }}</p>
                <small>{{ isExamGrading ? '判定会计入本场正确率，并更新复习计划' : '评分成功后会自动进入下一题' }}</small>
              </div>
              <button
                v-if="!isExamGrading"
                type="button"
                class="mode-toggle"
                :aria-pressed="advanced"
                @click="advanced = !advanced"
              >
                {{ advanced ? '返回双按钮' : '使用 FSRS 四档评分' }}
              </button>
            </div>

            <div
              v-if="!advanced || isExamGrading"
              class="rating-actions"
            >
              <button
                class="forgot-action"
                type="button"
                :aria-label="isExamGrading ? '答错' : '忘记了'"
                :disabled="submitting"
                @click="submit('forgot')"
              >
                <RotateCcw :size="18" />
                <span>{{ isExamGrading ? '答错' : '忘记了' }}</span>
                <kbd>1</kbd>
              </button>
              <button
                class="remember-action"
                type="button"
                :aria-label="isExamGrading ? '答对' : '记住了'"
                :disabled="submitting"
                @click="submit('remembered')"
              >
                <Sparkles :size="18" />
                <span>{{ submitting ? '正在保存' : isExamGrading ? '答对' : '记住了' }}</span>
                <kbd>2</kbd>
              </button>
            </div>

            <div
              v-else-if="!isExamGrading"
              class="rating-actions advanced-actions"
            >
              <button
                type="button"
                aria-label="没想起来"
                :disabled="submitting"
                @click="submit('again')"
              >
                没想起来 <kbd>A</kbd>
              </button>
              <button
                type="button"
                aria-label="有点吃力"
                :disabled="submitting"
                @click="submit('hard')"
              >
                有点吃力 <kbd>S</kbd>
              </button>
              <button
                type="button"
                aria-label="记住了"
                :disabled="submitting"
                @click="submit('good')"
              >
                记住了 <kbd>D</kbd>
              </button>
              <button
                type="button"
                aria-label="太简单了"
                :disabled="submitting"
                @click="submit('easy')"
              >
                太简单了 <kbd>F</kbd>
              </button>
            </div>
          </div>
        </Transition>
      </section>
    </Transition>
  </main>

  <ReviewMediaLightbox
    v-if="lightbox"
    :images="lightbox.images"
    :initial-index="lightbox.index"
    :label="lightbox.label"
    @close="lightbox = undefined"
  />
</template>

<style scoped>
.review-room {
  min-height: 100vh;
  padding: 24px 42px 72px;
  overflow-x: hidden;
  background:
    radial-gradient(circle at 50% 0, rgba(255, 253, 247, .9), transparent 38%),
    linear-gradient(180deg, rgba(232, 221, 199, .48), transparent 260px);
}

.review-header {
  display: grid;
  max-width: 1100px;
  grid-template-columns: 44px minmax(180px, 1fr) auto;
  gap: 18px;
  align-items: center;
  margin: 0 auto 36px;
}

.icon-action {
  display: grid;
  width: 44px;
  height: 44px;
  place-items: center;
  color: var(--ink);
  border: 1px solid var(--line);
  border-radius: 50%;
  background: rgba(255, 253, 247, .76);
  cursor: pointer;
  transition: transform var(--motion-feedback) var(--ease-standard), background var(--motion-standard) var(--ease-standard);
}
.icon-action:hover:not(:disabled) { background: var(--paper-raised); transform: translateX(-2px); }
.icon-action:disabled { cursor: wait; opacity: .52; }

.review-progress { display: flex; gap: 14px; align-items: center; color: var(--ink-muted); font-family: var(--font-serif); font-size: 13px; }
.progress-track { overflow: hidden; height: 5px; flex: 1; border-radius: 999px; background: var(--sand); }
.progress-track i { display: block; height: 100%; border-radius: inherit; background: var(--cinnabar); transition: width var(--motion-page) var(--ease-standard); }

.header-chips { display: flex; gap: 8px; align-items: center; justify-content: flex-end; }
.header-chips > span { display: inline-flex; gap: 6px; align-items: center; min-height: 32px; padding: 0 11px; border: 1px solid var(--line); border-radius: 999px; white-space: nowrap; font-size: 12px; }
.subject-chip { color: var(--ink-muted); }
.resume-chip { color: #48665a; background: var(--green-soft); }
.clock-chip { color: var(--ink); background: rgba(255, 253, 247, .72); font-family: var(--font-serif); font-variant-numeric: tabular-nums; }
.clock-chip.expired { color: #8d3f2f; border-color: rgba(185, 88, 63, .34); background: #fbebe3; }

.review-stage { max-width: 860px; margin: 0 auto; text-align: center; }
.stage-kicker { display: inline-flex; gap: 7px; align-items: center; margin: 0; color: var(--cinnabar); font-size: 12px; font-weight: 760; letter-spacing: .1em; }
h1 { margin: 10px 0 26px; font-size: clamp(30px, 4vw, 46px); font-weight: 650; letter-spacing: -.045em; }

.problem-paper,
.answer-paper {
  position: relative;
  min-height: 190px;
  padding: 42px;
  border: 1px solid var(--line);
  border-radius: 3px 20px 20px 20px;
  background: var(--paper-raised);
  box-shadow: var(--shadow-soft);
  text-align: left;
}
.problem-paper p,
.answer-paper p { margin: 0; font-family: var(--font-serif); font-size: clamp(20px, 2.6vw, 28px); line-height: 1.8; }
.paper-label { position: absolute; top: 0; left: 0; padding: 7px 14px; color: var(--ink-muted); border-radius: 0 0 11px; background: var(--sand); font-size: 12px; font-weight: 760; letter-spacing: .12em; }

.review-images { display: grid; gap: 14px; }
.media-button { position: relative; overflow: hidden; width: 100%; padding: 0; border: 1px solid var(--line); border-radius: 4px 14px 14px; background: white; cursor: zoom-in; }
.media-button img { display: block; width: 100%; max-height: 66vh; object-fit: contain; }
.media-button span { position: absolute; right: 10px; bottom: 10px; display: inline-flex; gap: 6px; align-items: center; padding: 7px 10px; color: white; border-radius: 999px; background: rgba(33, 51, 45, .86); font-size: 12px; font-weight: 720; opacity: 0; transform: translateY(4px); transition: opacity var(--motion-standard) var(--ease-standard), transform var(--motion-standard) var(--ease-standard); }
.media-button:hover span,
.media-button:focus-visible span { opacity: 1; transform: translateY(0); }

.expired-notice { margin: 16px 0 -10px; color: #8d3f2f; font-size: 12px; font-weight: 700; }
.reveal-action { display: inline-flex; gap: 9px; align-items: center; min-height: 50px; margin-top: 26px; padding: 0 22px; color: var(--white); border: 0; border-radius: 999px; background: var(--green-deep); box-shadow: 0 10px 24px rgba(33, 51, 45, .16); cursor: pointer; font-weight: 740; transition: transform var(--motion-feedback) var(--ease-standard), box-shadow var(--motion-standard) var(--ease-standard); }
.reveal-action:hover { transform: translateY(-2px); box-shadow: 0 14px 30px rgba(33, 51, 45, .2); }
.reveal-action:active,
.rating-actions button:active { transform: scale(.985); }
.exam-navigation { display: grid; max-width: 560px; grid-template-columns: 1fr 1fr; gap: 12px; margin: 26px auto 0; }
.exam-nav-action { display: inline-flex; gap: 9px; align-items: center; justify-content: center; min-height: 50px; padding: 0 18px; color: var(--green-deep); border: 1px solid rgba(33,51,45,.22); border-radius: 999px; background: rgba(255,253,247,.82); cursor: pointer; font-weight: 740; transition: transform var(--motion-feedback) var(--ease-standard), background var(--motion-standard) var(--ease-standard), box-shadow var(--motion-standard) var(--ease-standard); }
.exam-nav-action:hover:not(:disabled) { transform: translateY(-2px); background: var(--paper-raised); box-shadow: 0 10px 22px rgba(33,51,45,.1); }
.exam-nav-action:disabled { cursor: not-allowed; opacity: .42; }
.exam-next-action { color: var(--paper); border-color: var(--green-deep); background: var(--green-deep); }
.exam-next-action:hover:not(:disabled) { background: #182923; }
.begin-grading-action { color: var(--paper); border-color: var(--cinnabar); background: var(--cinnabar); box-shadow: 0 10px 24px rgba(185,88,63,.16); }
.begin-grading-action:hover:not(:disabled) { background: #a84b35; }

kbd { min-width: 24px; padding: 2px 6px; color: inherit; border: 1px solid currentColor; border-radius: 6px; font-family: inherit; font-size: 12px; line-height: 1.35; opacity: .62; }
.answer-area { margin-top: 18px; }
.answer-paper { min-height: 140px; border-color: rgba(185, 88, 63, .28); background: #fbf5ea; box-shadow: 0 14px 34px rgba(91, 61, 42, .1); }
.answer-paper .paper-label { color: #8b4634; background: #efd8cd; }

.rating-heading { display: flex; gap: 18px; align-items: flex-end; justify-content: space-between; max-width: 680px; margin: 24px auto 14px; text-align: left; }
.rating-heading p { margin: 0; color: var(--ink); font-weight: 760; }
.rating-heading small { color: var(--ink-muted); }
.mode-toggle { min-height: 44px; padding: 5px 0; color: var(--green-deep); border: 0; border-bottom: 1px solid currentColor; background: transparent; cursor: pointer; font-size: 12px; white-space: nowrap; }

.rating-actions { display: grid; max-width: 480px; grid-template-columns: 1fr 1fr; gap: 12px; margin: 0 auto; }
.rating-actions button { display: inline-flex; gap: 9px; align-items: center; justify-content: center; min-height: 50px; padding: 0 16px; border-radius: 999px; cursor: pointer; font-weight: 740; transition: transform var(--motion-feedback) var(--ease-standard), background var(--motion-standard) var(--ease-standard); }
.rating-actions button:disabled { cursor: wait; opacity: .52; }
.rating-actions kbd { margin-left: auto; }
.forgot-action { color: var(--ink); border: 1px solid var(--line); background: rgba(255, 253, 247, .78); }
.remember-action { color: var(--white); border: 1px solid var(--cinnabar); background: var(--cinnabar); }
.advanced-actions { max-width: 760px; grid-template-columns: repeat(4, 1fr); }
.advanced-actions button { color: var(--ink); border: 1px solid var(--line); background: rgba(255, 253, 247, .8); }
.advanced-actions button:nth-child(3),
.advanced-actions button:nth-child(4) { color: var(--white); border-color: var(--green-deep); background: var(--green-deep); }

.card-shift-forward-enter-active,
.card-shift-forward-leave-active,
.card-shift-backward-enter-active,
.card-shift-backward-leave-active,
.paper-turn-enter-active,
.paper-turn-leave-active { transition: opacity var(--motion-page) var(--ease-standard), transform var(--motion-page) var(--ease-standard); }
.card-shift-forward-enter-from { opacity: 0; transform: translateX(22px) scale(.985); }
.card-shift-forward-leave-to { opacity: 0; transform: translateX(-16px) scale(.99); }
.card-shift-backward-enter-from { opacity: 0; transform: translateX(-22px) scale(.985); }
.card-shift-backward-leave-to { opacity: 0; transform: translateX(16px) scale(.99); }
.paper-turn-enter-from { opacity: 0; transform: translateY(-10px) scale(.985); }
.paper-turn-leave-to { opacity: 0; transform: translateY(-5px) scale(.99); }

button:focus-visible { outline: 3px solid rgba(185, 88, 63, .32); outline-offset: 3px; }

@media (max-width: 780px) {
  .review-room { padding: 18px 18px 48px; }
  .review-header { grid-template-columns: 44px 1fr; margin-bottom: 26px; }
  .header-chips { grid-column: 1 / -1; justify-content: flex-start; overflow-x: auto; }
  .problem-paper,
  .answer-paper { padding: 36px 22px 24px; }
  .rating-heading { align-items: flex-start; flex-direction: column; }
  .advanced-actions { grid-template-columns: 1fr 1fr; }
  .exam-navigation { grid-template-columns: 1fr; }
}

@media (prefers-reduced-motion: reduce) {
  .icon-action,
  .progress-track i,
  .media-button span,
  .reveal-action,
  .exam-nav-action,
  .rating-actions button,
  .card-shift-forward-enter-active,
  .card-shift-forward-leave-active,
  .card-shift-backward-enter-active,
  .card-shift-backward-leave-active,
  .paper-turn-enter-active,
  .paper-turn-leave-active { transition: none; }
  .card-shift-forward-enter-from,
  .card-shift-forward-leave-to,
  .card-shift-backward-enter-from,
  .card-shift-backward-leave-to,
  .paper-turn-enter-from,
  .paper-turn-leave-to { transform: none; }
}
</style>
