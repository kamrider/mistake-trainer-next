<script setup lang="ts">
import { computed, ref, watch } from 'vue'
import { Check, Clock3, Pencil, Target, X } from '@lucide/vue'
import type { DailyPlanOverview, LearningGoalInput } from '@/shared/api/bindings'

const props = defineProps<{
  plan: DailyPlanOverview
  busy?: boolean | undefined
  errorMessage?: string | undefined
}>()

const emit = defineEmits<{
  save: [input: LearningGoalInput]
}>()

const editing = ref(false)
const dailyReviewTarget = ref(props.plan.reviewTarget)
const dailyMinutesTarget = ref(props.plan.minutesTarget)
const localError = ref('')

watch(() => [props.plan.reviewTarget, props.plan.minutesTarget] as const, ([reviews, minutes]) => {
  if (editing.value) return
  dailyReviewTarget.value = reviews
  dailyMinutesTarget.value = minutes
})

const progressValue = computed(() => Math.min(props.plan.completedReviews, props.plan.reviewTarget))
const progressPercent = computed(() => Math.round(progressValue.value / props.plan.reviewTarget * 100))
const workloadCopy = computed(() => {
  if (props.plan.suggestedReviews === 0) return '今天的计划已经完成，可以安心收尾。'
  if (props.plan.dueReviews > props.plan.remainingReviews) {
    return `有 ${props.plan.dueReviews} 道到期题，超过目标时优先处理到期内容。`
  }
  return `建议再完成 ${props.plan.suggestedReviews} 道，预计约 ${props.plan.estimatedMinutes} 分钟。`
})

function beginEditing() {
  dailyReviewTarget.value = props.plan.reviewTarget
  dailyMinutesTarget.value = props.plan.minutesTarget
  localError.value = ''
  editing.value = true
}

function cancelEditing() {
  if (props.busy) return
  editing.value = false
  localError.value = ''
}

function submit() {
  const reviews = Number(dailyReviewTarget.value)
  const minutes = Number(dailyMinutesTarget.value)
  if (!Number.isInteger(reviews) || reviews < 1 || reviews > 200
    || !Number.isInteger(minutes) || minutes < 5 || minutes > 240) {
    localError.value = '每日复习题数需为 1–200，每日学习时间需为 5–240 分钟。'
    return
  }
  localError.value = ''
  emit('save', { dailyReviewTarget: reviews, dailyMinutesTarget: minutes })
}

watch(() => props.busy, (busy, previous) => {
  if (previous && !busy && !props.errorMessage) editing.value = false
})
</script>

<template>
  <section
    class="learning-plan"
    aria-labelledby="learning-plan-title"
  >
    <div class="plan-summary">
      <div class="plan-heading">
        <span class="plan-icon"><Target
          :size="20"
          aria-hidden="true"
        /></span>
        <div>
          <p class="eyebrow">
            每日计划
          </p>
          <h2 id="learning-plan-title">
            今日完成 {{ plan.completedReviews }} / {{ plan.reviewTarget }} 道
          </h2>
        </div>
      </div>
      <button
        v-if="!editing"
        type="button"
        class="edit-button"
        @click="beginEditing"
      >
        <Pencil
          :size="15"
          aria-hidden="true"
        />调整学习目标
      </button>
    </div>

    <div class="progress-row">
      <progress
        :value="progressValue"
        :max="plan.reviewTarget"
      >
        {{ progressPercent }}%
      </progress>
      <span>{{ progressPercent }}%</span>
    </div>
    <div class="plan-details">
      <p>{{ workloadCopy }}</p>
      <span><Clock3
        :size="15"
        aria-hidden="true"
      />时间目标 {{ plan.minutesTarget }} 分钟</span>
    </div>

    <form
      v-if="editing"
      class="goal-form"
      novalidate
      @submit.prevent="submit"
      @keydown.esc="cancelEditing"
    >
      <label>
        <span>每日复习题数</span>
        <input
          v-model.number="dailyReviewTarget"
          type="number"
          min="1"
          max="200"
          step="1"
          :disabled="busy"
        >
      </label>
      <label>
        <span>每日学习时间（分钟）</span>
        <input
          v-model.number="dailyMinutesTarget"
          type="number"
          min="5"
          max="240"
          step="5"
          :disabled="busy"
        >
      </label>
      <div class="goal-actions">
        <button
          type="button"
          :disabled="busy"
          @click="cancelEditing"
        >
          <X :size="15" />取消
        </button>
        <button
          type="submit"
          class="save-button"
          :disabled="busy"
        >
          <Check :size="15" />{{ busy ? '正在保存' : '保存目标' }}
        </button>
      </div>
      <p
        v-if="localError || errorMessage"
        class="goal-error"
        role="alert"
      >
        {{ localError || errorMessage }}
      </p>
    </form>
  </section>
</template>

<style scoped>
.learning-plan{display:grid;gap:15px;padding:25px 30px;border:1px solid var(--line);border-radius:var(--radius-md);background:rgba(255,253,247,.72)}.plan-summary,.plan-heading,.plan-details,.goal-actions{display:flex;align-items:center}.plan-summary,.plan-details{justify-content:space-between;gap:18px}.plan-heading{gap:12px}.plan-icon{display:grid;width:40px;height:40px;place-items:center;color:var(--cinnabar);border-radius:12px;background:rgba(185,88,63,.1)}.eyebrow{margin:0 0 5px;color:var(--cinnabar);font-size:11px;font-weight:800;letter-spacing:.15em}.plan-heading h2{margin:0;font-size:21px}.edit-button,.goal-actions button{display:inline-flex;min-height:40px;align-items:center;gap:7px;padding:0 13px;color:var(--green-deep);border:1px solid var(--line);border-radius:999px;background:var(--paper-raised);font-weight:700;cursor:pointer}.progress-row{display:grid;grid-template-columns:1fr auto;gap:12px;align-items:center}.progress-row progress{width:100%;height:9px;accent-color:var(--cinnabar)}.progress-row span{color:var(--ink-muted);font-size:12px;font-variant-numeric:tabular-nums}.plan-details p{margin:0;color:var(--ink);font-weight:650}.plan-details>span{display:inline-flex;align-items:center;gap:6px;color:var(--ink-muted);font-size:13px;white-space:nowrap}.goal-form{display:grid;grid-template-columns:1fr 1fr auto;gap:12px;align-items:end;padding-top:15px;border-top:1px solid var(--line)}.goal-form label{display:grid;gap:6px;color:var(--ink-muted);font-size:12px}.goal-form input{min-height:42px;padding:8px 11px;color:var(--ink);border:1px solid var(--line);border-radius:10px;background:var(--paper-raised);font:inherit;font-size:15px}.goal-actions{gap:8px}.goal-actions .save-button{color:var(--white);border-color:var(--green-deep);background:var(--green-deep)}.goal-error{grid-column:1/-1;margin:0;color:var(--cinnabar);font-size:13px}.goal-actions button:disabled,.goal-form input:disabled{opacity:.55;cursor:wait}@media(max-width:720px){.plan-summary,.plan-details{align-items:flex-start;flex-direction:column}.goal-form{grid-template-columns:1fr}.goal-actions{justify-content:flex-end}.plan-details>span{white-space:normal}}@media(prefers-reduced-motion:reduce){.edit-button,.goal-actions button{transition:none}}
</style>
