<script setup lang="ts">
import { isTauri } from '@tauri-apps/api/core'
import { computed, onMounted, ref } from 'vue'
import { useRoute, useRouter } from 'vue-router'
import ReviewRoom from '@/modules/review/components/ReviewRoom.vue'
import { mapSimpleRating, type FsrsRating, type SimpleRating } from '@/modules/review/domain/rating'
import { commands, type ProblemDetail, type ReviewQueueItem } from '@/shared/api/bindings'
import { normalizeAppResult } from '@/shared/api/normalize-result'

const route = useRoute()
const router = useRouter()
const queue = ref<ReviewQueueItem[]>([])
const currentIndex = ref(0)
const currentProblem = ref<ProblemDetail>()
const loading = ref(true)
const itemLoading = ref(false)
const submitting = ref(false)
const completed = ref(false)
const errorMessage = ref('')
let startedAt = performance.now()

const currentItem = computed(() => queue.value[currentIndex.value])
const questionImages = computed(() => currentProblem.value?.assets
  .filter(asset => asset.role === 'question')
  .map(asset => asset.dataUrl) ?? [])
const answerImages = computed(() => currentProblem.value?.assets
  .filter(asset => asset.role === 'answer')
  .map(asset => asset.dataUrl) ?? [])

async function loadQueue() {
  loading.value = true
  errorMessage.value = ''
  try {
    if (!isTauri()) return
    const queryId = Array.isArray(route.query.problemId) ? route.query.problemId[0] : route.query.problemId
    const result = normalizeAppResult(await commands.reviewQueue(queryId || null))
    if (result.ok) {
      queue.value = result.data
      if (queue.value.length > 0) await loadCurrentProblem()
    }
    else errorMessage.value = result.error.userMessage
  }
  catch {
    errorMessage.value = '训练队列暂时无法打开，请回到题库后重试。'
  }
  finally {
    loading.value = false
  }
}

async function loadCurrentProblem() {
  const item = currentItem.value
  currentProblem.value = undefined
  if (!item) return
  itemLoading.value = true
  try {
    const result = normalizeAppResult(await commands.problemDetail(item.problemId))
    if (result.ok) {
      currentProblem.value = result.data
      startedAt = performance.now()
    }
    else errorMessage.value = result.error.userMessage
  }
  catch {
    errorMessage.value = '当前题目图片无法解密，请回到题库后重试。'
  }
  finally {
    itemLoading.value = false
  }
}

async function submitRating(rating: SimpleRating | FsrsRating) {
  const item = currentItem.value
  if (!item || submitting.value) return
  submitting.value = true
  errorMessage.value = ''
  try {
    const durationMs = Math.max(0, Math.min(86_400_000, Math.round(performance.now() - startedAt)))
    const result = normalizeAppResult(await commands.reviewSubmit({
      problemId: item.problemId,
      rating: rating === 'forgot' || rating === 'remembered' ? mapSimpleRating(rating) : rating,
      durationMs,
    }))
    if (!result.ok) {
      errorMessage.value = result.error.userMessage
      return
    }
    if (currentIndex.value + 1 >= queue.value.length) completed.value = true
    else {
      currentIndex.value += 1
      await loadCurrentProblem()
    }
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
    <p>正在解密今日训练队列…</p>
  </main>
  <main
    v-else-if="completed"
    class="review-message review-complete"
  >
    <p>本轮完成</p>
    <h1>把今天该记住的，认真看完了。</h1>
    <button
      type="button"
      @click="router.push({ name: 'dashboard' })"
    >
      回到训练台
    </button>
  </main>
  <main
    v-else-if="errorMessage && !currentProblem"
    class="review-message review-error"
    role="alert"
  >
    <p>{{ errorMessage }}</p>
    <button
      type="button"
      @click="router.push({ name: 'library' })"
    >
      回到题库
    </button>
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
      :key="currentProblem.id"
      :subject="currentProblem.subject || '未分类'"
      :prompt="currentProblem.note || '请观察题图，在心里完整走一遍解题过程。'"
      answer="请对照答案图片，确认关键步骤和易错点。"
      :question-images="questionImages"
      :answer-images="answerImages"
      :current="currentIndex + 1"
      :total="queue.length"
      :submitting="submitting"
      @exit="router.push({ name: 'dashboard' })"
      @rate="submitRating"
    />
  </template>
</template>

<style scoped>
.review-message { display: grid; min-height: 100vh; padding: 32px; place-content: center; justify-items: center; background: linear-gradient(180deg, rgba(232,221,199,.5), transparent 50%); text-align: center; }
.review-message p { margin: 0; color: var(--cinnabar); font-size: 12px; font-weight: 760; letter-spacing: .12em; }
.review-message h1 { max-width: 660px; margin: 12px 0; font-size: clamp(32px,5vw,54px); letter-spacing: -.05em; }
.review-message span { color: var(--ink-muted); }
.review-message button { min-height: 44px; margin-top: 26px; padding: 0 20px; color: var(--paper); border: 0; border-radius: 999px; background: var(--green-deep); cursor: pointer; font-weight: 720; }
.review-error p { max-width: 520px; padding: 14px 18px; border: 1px solid rgba(185,88,63,.25); border-radius: 10px; background: rgba(185,88,63,.08); letter-spacing: 0; }
.floating-error { position: fixed; z-index: 20; top: 18px; left: 50%; margin: 0; padding: 11px 16px; color: #7f3829; border: 1px solid rgba(185,88,63,.25); border-radius: 999px; background: #fbefe8; box-shadow: var(--shadow-soft); transform: translateX(-50%); }
</style>
