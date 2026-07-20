<script setup lang="ts">
import { isTauri } from '@tauri-apps/api/core'
import { computed, onMounted, ref } from 'vue'
import { useRoute, useRouter } from 'vue-router'
import ReviewRoom from '@/modules/review/components/ReviewRoom.vue'
import { useReviewClock } from '@/modules/review/composables/useReviewClock'
import { mapSimpleRating, type FsrsRating, type SimpleRating } from '@/modules/review/domain/rating'
import { commands, type ProblemDetail, type ReviewQueueOverview } from '@/shared/api/bindings'
import { normalizeAppResult } from '@/shared/api/normalize-result'

const route = useRoute()
const router = useRouter()
const overview = ref<ReviewQueueOverview>({
  sessionId: null,
  mode: 'due',
  resumed: false,
  completedCount: 0,
  totalCount: 0,
  examPhase: null,
  examQuestionIndex: 0,
  examCorrectCount: 0,
  examWrongCount: 0,
  items: [],
})
const currentIndex = ref(0)
const currentProblem = ref<ProblemDetail>()
const loading = ref(true)
const itemLoading = ref(false)
const submitting = ref(false)
const transitioning = ref(false)
const completed = ref(false)
const successfulCount = ref(0)
const examCorrectCount = ref(0)
const examWrongCount = ref(0)
const errorMessage = ref('')
const clock = useReviewClock()

const queue = computed(() => overview.value.items)
const currentItem = computed(() => queue.value[currentIndex.value])
const isExam = computed(() => overview.value.mode === 'exam')
const isExamAnswering = computed(() => isExam.value && overview.value.examPhase === 'answering')
const currentNumber = computed(() => isExamAnswering.value
  ? currentIndex.value + 1
  : overview.value.completedCount + currentIndex.value + 1)
const questionImages = computed(() => currentProblem.value?.assets
  .filter(asset => asset.role === 'question')
  .map(asset => asset.dataUrl) ?? [])
const answerImages = computed(() => currentProblem.value?.assets
  .filter(asset => asset.role === 'answer')
  .map(asset => asset.dataUrl) ?? [])
const isManual = computed(() => overview.value.mode === 'manual')
const completionHeading = computed(() => {
  if (isExam.value) return '这场模拟考试已经核对完。'
  return isManual.value ? '这组自选卡已经练完。' : '把今天该记住的，认真看完了。'
})
const completionSummary = computed(() => {
  if (isExam.value) return '整组题已完成独立作答与统一核对，结果也已写入复习计划。'
  return isManual.value
    ? '你挑出的题已全部复盘，进度会在下次打开时保持一致。'
    : '今天到期的内容已经完成，新的到期题会按计划出现。'
})
const examAccuracy = computed(() => {
  const graded = examCorrectCount.value + examWrongCount.value
  return graded === 0 ? 0 : Math.round((examCorrectCount.value / graded) * 100)
})

function loadDevelopmentPreview() {
  const previewMode = typeof route.query.preview === 'string' ? route.query.preview : 'review'
  const examPreview = previewMode.startsWith('exam-')
  const examPhase = previewMode === 'exam-grading' ? 'grading' : examPreview ? 'answering' : null
  const previewImage = (label: string, accent: string) => `data:image/svg+xml;charset=utf-8,${encodeURIComponent(`
    <svg xmlns="http://www.w3.org/2000/svg" width="1200" height="820" viewBox="0 0 1200 820">
      <rect width="1200" height="820" fill="#fffdf7"/>
      <path d="M90 130h1020M90 220h1020M90 310h1020M90 400h1020M90 490h1020M90 580h1020M90 670h1020" stroke="#e8ddc7" stroke-width="2"/>
      <text x="90" y="88" font-family="serif" font-size="34" fill="${accent}">${label}</text>
      <text x="90" y="185" font-family="serif" font-size="30" fill="#22302b">已知椭圆 C：x²/4 + y² = 1，求焦点坐标并说明离心率。</text>
      <ellipse cx="600" cy="470" rx="270" ry="150" fill="none" stroke="#21332d" stroke-width="7"/>
      <line x1="250" y1="470" x2="950" y2="470" stroke="#b9583f" stroke-width="3"/>
      <circle cx="370" cy="470" r="9" fill="#b9583f"/><circle cx="830" cy="470" r="9" fill="#b9583f"/>
    </svg>
  `)}`
  overview.value = {
    sessionId: 'development-preview',
    mode: previewMode === 'manual-review' ? 'manual' : examPreview ? 'exam' : 'due',
    resumed: true,
    completedCount: 2,
    totalCount: 5,
    examPhase,
    examQuestionIndex: 0,
    examCorrectCount: previewMode === 'exam-complete' ? 4 : 0,
    examWrongCount: previewMode === 'exam-complete' ? 1 : 0,
    items: [{ problemId: 'development-preview-problem', dueAtUtcMs: Date.now() - 1_000, reviewCount: 3 }],
  }
  currentProblem.value = {
    id: 'development-preview-problem',
    subject: '数学 · 圆锥曲线',
    note: '先独立写出焦点关系，再检查长短轴是否混淆。',
    status: 'active',
    timeLimitSeconds: 75,
    updatedAtUtcMs: Date.now(),
    assets: [
      { id: 'preview-q1', role: 'question', position: 0, mediaType: 'image/svg+xml', dataUrl: previewImage('题面 · 1 / 2', '#b9583f') },
      { id: 'preview-q2', role: 'question', position: 1, mediaType: 'image/svg+xml', dataUrl: previewImage('题面 · 2 / 2', '#b9583f') },
      { id: 'preview-a1', role: 'answer', position: 0, mediaType: 'image/svg+xml', dataUrl: previewImage('答案 · 焦点 (±√3, 0)，e = √3 / 2', '#21332d') },
    ],
  }
  clock.reset(currentProblem.value.timeLimitSeconds)
  clock.start()
  if (previewMode === 'exam-complete') {
    examCorrectCount.value = 4
    examWrongCount.value = 1
    successfulCount.value = 5
    completed.value = true
  }
}

async function loadQueue() {
  loading.value = true
  completed.value = false
  currentIndex.value = 0
  successfulCount.value = 0
  examCorrectCount.value = 0
  examWrongCount.value = 0
  currentProblem.value = undefined
  errorMessage.value = ''
  clock.reset()
  try {
    if (!isTauri()) {
      // Explicit development-only component gallery; Vite removes it from release builds.
      if (import.meta.env.DEV && ['review', 'manual-review', 'exam-answering', 'exam-grading', 'exam-complete'].includes(String(route.query.preview)))
        loadDevelopmentPreview()
      return
    }
    const result = normalizeAppResult(await commands.reviewQueue())
    if (!result.ok) {
      errorMessage.value = result.error.userMessage
      return
    }
    overview.value = result.data
    examCorrectCount.value = result.data.examCorrectCount
    examWrongCount.value = result.data.examWrongCount
    currentIndex.value = result.data.mode === 'exam' && result.data.examPhase === 'answering'
      ? Math.max(0, Math.min(result.data.examQuestionIndex, result.data.items.length - 1))
      : 0
    completed.value = Boolean(
      overview.value.sessionId
      && overview.value.totalCount === overview.value.completedCount,
    )
    if (overview.value.items.length > 0)
      await loadCurrentProblem()
  }
  catch {
    errorMessage.value = '训练队列暂时无法打开，请稍后重试。'
  }
  finally {
    loading.value = false
  }
}

async function loadCurrentProblem() {
  const item = currentItem.value
  currentProblem.value = undefined
  clock.reset()
  if (!item) return
  itemLoading.value = true
  errorMessage.value = ''
  try {
    const result = normalizeAppResult(await commands.reviewCurrentProblem())
    if (!result.ok) {
      errorMessage.value = result.error.userMessage
      return
    }
    if (result.data.id !== item.problemId) {
      errorMessage.value = '训练进度已经变化，请重新读取训练。'
      return
    }
    currentProblem.value = result.data
    clock.reset(result.data.timeLimitSeconds)
    clock.start()
  }
  catch {
    errorMessage.value = '当前题目图片无法解密，请稍后重试。'
  }
  finally {
    itemLoading.value = false
  }
}

async function moveExam(direction: -1 | 1) {
  if (!isExamAnswering.value || transitioning.value) return
  const position = currentIndex.value + direction
  if (position < 0 || position >= queue.value.length) return
  transitioning.value = true
  errorMessage.value = ''
  clock.stop()
  try {
    const result = normalizeAppResult(await commands.reviewExamNavigate({ position }))
    if (!result.ok) {
      errorMessage.value = result.error.userMessage
      clock.start()
      return
    }
    overview.value = result.data
    currentIndex.value = result.data.examQuestionIndex
    await loadCurrentProblem()
  }
  catch {
    errorMessage.value = '题目位置没有保存，请保持当前页面并重试。'
    clock.start()
  }
  finally {
    transitioning.value = false
  }
}

async function beginExamGrading() {
  if (!isExamAnswering.value || transitioning.value) return
  transitioning.value = true
  errorMessage.value = ''
  clock.stop()
  try {
    const result = normalizeAppResult(await commands.reviewExamBeginGrading())
    if (!result.ok) {
      errorMessage.value = result.error.userMessage
      clock.start()
      return
    }
    overview.value = result.data
    currentIndex.value = 0
    await loadCurrentProblem()
  }
  catch {
    errorMessage.value = '暂时无法进入答案核对，请保持当前页面并重试。'
    clock.start()
  }
  finally {
    transitioning.value = false
  }
}

async function retryCurrent() {
  if (currentItem.value)
    await loadCurrentProblem()
  else
    await loadQueue()
}

async function submitRating(rating: SimpleRating | FsrsRating) {
  const item = currentItem.value
  if (!item || submitting.value) return
  clock.stop()
  submitting.value = true
  errorMessage.value = ''
  try {
    const result = normalizeAppResult(await commands.reviewSubmit({
      problemId: item.problemId,
      rating: rating === 'forgot' || rating === 'remembered' ? mapSimpleRating(rating) : rating,
      durationMs: Math.max(0, Math.min(86_400_000, Math.round(clock.elapsedMs.value))),
    }))
    if (!result.ok) {
      errorMessage.value = result.error.userMessage
      return
    }

    successfulCount.value += 1
    if (isExam.value) {
      if (rating === 'remembered' || rating === 'good' || rating === 'easy')
        examCorrectCount.value += 1
      else
        examWrongCount.value += 1
    }
    currentIndex.value += 1
    if (currentIndex.value >= queue.value.length)
      completed.value = true
    else
      await loadCurrentProblem()
  }
  catch {
    errorMessage.value = '这次评分没有保存，请保持页面打开并重试。'
  }
  finally {
    submitting.value = false
  }
}

onMounted(loadQueue)
</script>

<template>
  <main
    v-if="loading || itemLoading"
    class="review-message"
    aria-live="polite"
  >
    <span class="loading-mark" />
    <p>正在解密今日训练队列…</p>
  </main>

  <main
    v-else-if="completed"
    class="review-message review-complete"
  >
    <span class="complete-seal">完</span>
    <p>本轮完成</p>
    <h1>{{ completionHeading }}</h1>
    <span>
      {{ completionSummary }}<br>
      <template v-if="isExam">
        答对 {{ examCorrectCount }} 道 · 答错 {{ examWrongCount }} 道 · 正确率 {{ examAccuracy }}%
      </template>
      <template v-else>
        本次完成 {{ successfulCount }} 道 · 会话进度 {{ overview.completedCount + successfulCount }} / {{ overview.totalCount }}
      </template>
    </span>
    <div class="message-actions">
      <button
        type="button"
        @click="router.push({ name: 'dashboard' })"
      >
        回到训练台
      </button>
      <button
        type="button"
        class="secondary-action"
        @click="router.push({ name: 'library' })"
      >
        查看题库
      </button>
    </div>
  </main>

  <main
    v-else-if="errorMessage && !currentProblem"
    class="review-message review-error"
    role="alert"
  >
    <p>{{ errorMessage }}</p>
    <div class="message-actions">
      <button
        type="button"
        @click="retryCurrent"
      >
        重新读取训练
      </button>
      <button
        type="button"
        class="secondary-action"
        @click="router.push({ name: 'library' })"
      >
        回到题库
      </button>
    </div>
  </main>

  <main
    v-else-if="!currentItem || !currentProblem"
    class="review-message review-empty"
  >
    <p>今日已清</p>
    <h1>现在没有到期的错题。</h1>
    <span>新录入的题和到期题目会自动出现在这里。</span>
    <button
      type="button"
      @click="router.push({ name: 'library' })"
    >
      去题库看看
    </button>
  </main>

  <template v-else>
    <p
      v-if="errorMessage"
      class="floating-error"
      role="alert"
    >
      {{ errorMessage }}
    </p>
    <ReviewRoom
      :subject="currentProblem.subject || '未分类'"
      :mode="overview.mode"
      :exam-phase="overview.examPhase === 'answering' || overview.examPhase === 'grading' ? overview.examPhase : null"
      :prompt="currentProblem.note || '请观察题图，在心里完整走一遍解题过程。'"
      answer="请对照答案图片，确认关键步骤和易错点。"
      :question-images="questionImages"
      :answer-images="answerImages"
      :current="currentNumber"
      :total="overview.totalCount"
      :elapsed-text="clock.displayText.value"
      :time-limit-seconds="currentProblem.timeLimitSeconds"
      :expired="clock.expired.value"
      :resumed="overview.resumed && (isExamAnswering || currentIndex === 0)"
      :submitting="submitting || transitioning"
      @reveal="clock.stop"
      @exit="router.push({ name: 'dashboard' })"
      @previous="moveExam(-1)"
      @next="moveExam(1)"
      @begin-grading="beginExamGrading"
      @rate="submitRating"
    />
  </template>
</template>

<style scoped>
.review-message {
  display: grid;
  min-height: 100vh;
  padding: 32px;
  place-content: center;
  justify-items: center;
  background:
    radial-gradient(circle at 50% 34%, rgba(255, 253, 247, .92), transparent 34%),
    linear-gradient(180deg, rgba(232, 221, 199, .55), transparent 58%);
  text-align: center;
}
.review-message p { margin: 0; color: var(--cinnabar); font-size: 12px; font-weight: 760; letter-spacing: .12em; }
.review-message h1 { max-width: 680px; margin: 12px 0; font-size: clamp(32px, 5vw, 54px); letter-spacing: -.05em; }
.review-message > span:not(.complete-seal, .loading-mark) { color: var(--ink-muted); }
.review-message button { min-height: 44px; padding: 0 20px; color: var(--paper); border: 1px solid var(--green-deep); border-radius: 999px; background: var(--green-deep); cursor: pointer; font-weight: 720; }
.message-actions { display: flex; gap: 10px; margin-top: 26px; }
.review-message .secondary-action { color: var(--green-deep); background: transparent; }
.complete-seal { display: grid; width: 54px; height: 54px; margin-bottom: 18px; place-items: center; color: var(--paper); border-radius: 12px 2px 12px 12px; background: var(--cinnabar); box-shadow: 0 12px 28px rgba(185, 88, 63, .2); font-family: var(--font-serif); font-size: 22px; transform: rotate(-3deg); }
.review-complete > :not(.complete-seal) { animation: completion-in var(--motion-page) var(--ease-standard) both; }
.review-complete > .complete-seal { animation: completion-seal-in var(--motion-page) var(--ease-standard) both; }
.review-complete > :nth-child(2) { animation-delay: 45ms; }
.review-complete > :nth-child(3) { animation-delay: 80ms; }
.review-complete > :nth-child(4) { animation-delay: 115ms; }
.review-complete > :nth-child(5) { animation-delay: 150ms; }
.loading-mark { width: 28px; height: 28px; margin-bottom: 14px; border: 2px solid var(--sand); border-top-color: var(--cinnabar); border-radius: 50%; animation: spin .8s linear infinite; }
.review-error p { max-width: 540px; padding: 14px 18px; color: #7f3829; border: 1px solid rgba(185, 88, 63, .25); border-radius: 10px; background: rgba(185, 88, 63, .08); letter-spacing: 0; }
.floating-error { position: fixed; z-index: 20; top: 18px; left: 50%; max-width: min(680px, calc(100vw - 40px)); margin: 0; padding: 11px 16px; color: #7f3829; border: 1px solid rgba(185, 88, 63, .25); border-radius: 999px; background: #fbefe8; box-shadow: var(--shadow-soft); transform: translateX(-50%); }

@keyframes spin { to { transform: rotate(360deg); } }
@keyframes completion-in { from { opacity: 0; transform: translateY(10px); } }
@keyframes completion-seal-in { from { opacity: 0; transform: translateY(10px) rotate(-8deg) scale(.94); } to { transform: rotate(-3deg); } }
@media (prefers-reduced-motion: reduce) { .loading-mark, .review-complete > * { animation: none; } }
@media (max-width: 560px) { .message-actions { width: 100%; flex-direction: column; } }
</style>
