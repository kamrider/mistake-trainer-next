<script setup lang="ts">
import { ref, watch } from 'vue'
import type { QuickReviewPreset, ReviewQuickStartInput } from '../../../shared/api/bindings'

const props = withDefaults(defineProps<{
  open: boolean
  busy?: boolean
  errorMessage?: string
}>(), { busy: false, errorMessage: '' })

const emit = defineEmits<{
  close: []
  start: [input: ReviewQuickStartInput]
}>()

const preset = ref<QuickReviewPreset>('five_minutes')
const subject = ref('')
const tag = ref('')
const presets: Array<{ value: QuickReviewPreset; title: string; detail: string }> = [
  { value: 'five_minutes', title: '五分钟热身', detail: '最多 8 道，适合课间快速回看' },
  { value: 'ten_problems', title: '十道题专注', detail: '最多 10 道，完成一个清晰小目标' },
  { value: 'recently_forgotten', title: '最近遗忘', detail: '最多 20 道，重看近 30 天答错的题' },
]

watch(() => props.open, (open) => {
  if (!open) return
  preset.value = 'five_minutes'
  subject.value = ''
  tag.value = ''
})

function start() {
  emit('start', {
    preset: preset.value,
    subject: subject.value.trim() || null,
    tag: tag.value.trim() || null,
  })
}
</script>

<template>
  <div
    v-if="open"
    class="dialog-backdrop"
    @mousedown.self="!busy && $emit('close')"
  >
    <section
      class="dialog"
      role="dialog"
      aria-modal="true"
      aria-labelledby="quick-session-title"
    >
      <p class="eyebrow">
        小步开始
      </p>
      <h2 id="quick-session-title">
        快速训练
      </h2>
      <p class="intro">
        到期题会排在前面；没有到期题时，再补入符合条件的新题。
      </p>
      <fieldset>
        <legend>选择训练方式</legend>
        <label
          v-for="option in presets"
          :key="option.value"
          class="preset-card"
          :class="{ selected: preset === option.value }"
        >
          <input
            v-model="preset"
            type="radio"
            name="quick-preset"
            :value="option.value"
            :disabled="busy"
          >
          <span><strong>{{ option.title }}</strong><small>{{ option.detail }}</small></span>
        </label>
      </fieldset>
      <div class="optional-filters">
        <label>
          科目（可选）
          <input
            v-model="subject"
            type="text"
            maxlength="40"
            :disabled="busy"
            placeholder="例如：数学"
          >
        </label>
        <label>
          标签（可选）
          <input
            v-model="tag"
            type="text"
            maxlength="30"
            :disabled="busy"
            placeholder="例如：错因·计算失误"
          >
        </label>
      </div>
      <p
        v-if="errorMessage"
        class="error"
        role="alert"
      >
        {{ errorMessage }}
      </p>
      <div class="actions">
        <button
          type="button"
          :disabled="busy"
          @click="$emit('close')"
        >
          取消
        </button>
        <button
          type="button"
          class="primary"
          :disabled="busy"
          @click="start"
        >
          {{ busy ? '正在准备训练…' : '开始这轮训练' }}
        </button>
      </div>
    </section>
  </div>
</template>

<style scoped>
.dialog-backdrop { position: fixed; z-index: 80; inset: 0; display: grid; padding: 24px; place-items: center; background: rgba(27,36,33,.38); backdrop-filter: blur(6px); }
.dialog { display: grid; width: min(620px,100%); gap: 15px; padding: 27px; border: 1px solid var(--line); border-radius: 5px 22px 22px 22px; background: #fffdf7; box-shadow: 0 28px 80px rgba(22,31,28,.28); }
.eyebrow { margin: 0; color: var(--cinnabar); font-size: 12px; font-weight: 760; letter-spacing: .1em; }
h2 { margin: -5px 0 0; }
.intro { margin: 0; color: var(--ink-muted); line-height: 1.55; }
fieldset { display: grid; grid-template-columns: repeat(3,minmax(0,1fr)); gap: 9px; padding: 0; border: 0; }
legend { margin-bottom: 9px; font-size: 13px; font-weight: 760; }
.preset-card { display: flex; min-height: 94px; gap: 9px; align-items: flex-start; padding: 13px; border: 1px solid var(--line); border-radius: 13px; background: white; cursor: pointer; }
.preset-card.selected { border-color: var(--green-deep); background: var(--green-soft); }
.preset-card span { display: grid; gap: 5px; }
.preset-card small { color: var(--ink-muted); line-height: 1.4; }
.optional-filters { display: grid; grid-template-columns: 1fr 1fr; gap: 10px; }
.optional-filters label { display: grid; gap: 7px; font-size: 13px; font-weight: 720; }
.optional-filters input { min-height: 42px; padding: 0 11px; border: 1px solid var(--line); border-radius: 10px; background: white; }
.error { margin: 0; color: #7f3829; }
.actions { display: flex; gap: 9px; justify-content: flex-end; }
.actions button { min-height: 42px; padding: 0 15px; border: 1px solid var(--line); border-radius: 999px; background: white; cursor: pointer; }
.actions .primary { color: var(--paper); border-color: var(--green-deep); background: var(--green-deep); }
@media (max-width: 650px) { fieldset, .optional-filters { grid-template-columns: 1fr; } }
</style>
