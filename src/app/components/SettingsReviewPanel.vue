<script setup lang="ts">
import type { ReviewFocusPolicy, ReviewPreferences } from '../../shared/api/bindings'

export interface SettingsReviewOption {
  value: ReviewFocusPolicy
  title: string
  hint: string
}

defineProps<{
  preferences: ReviewPreferences
  options: readonly SettingsReviewOption[]
  saving: boolean
  message: string
}>()

const emit = defineEmits<{
  updateFocusPolicy: [policy: ReviewFocusPolicy]
  save: []
}>()

function updateFocusPolicy(event: Event) {
  emit('updateFocusPolicy', (event.target as HTMLInputElement).value as ReviewFocusPolicy)
}
</script>

<template>
  <section
    id="settings-review"
    class="review-rhythm-panel"
    aria-labelledby="review-rhythm-title"
  >
    <header>
      <div>
        <p>训练偏好 · 当前学习档案</p>
        <h2 id="review-rhythm-title">
          训练间的专注插曲
        </h2>
        <span>舒尔特 5×5 只改变普通训练节奏；每轮都能跳过，模拟考试不会插入专注环节。</span>
      </div>
    </header>

    <div class="rhythm-options">
      <label
        v-for="option in options"
        :key="option.value"
        :class="{ selected: preferences.focusPolicy === option.value }"
      >
        <input
          type="radio"
          name="review-focus-policy"
          :value="option.value"
          :checked="preferences.focusPolicy === option.value"
          @change="updateFocusPolicy"
        >
        <span
          class="rhythm-mark"
          aria-hidden="true"
        />
        <span><strong>{{ option.title }}</strong><small>{{ option.hint }}</small></span>
      </label>
    </div>

    <footer class="rhythm-actions">
      <p
        v-if="message"
        role="status"
      >
        {{ message }}
      </p>
      <span v-else>正在进行的训练保持原节奏，避免中途突然改变。</span>
      <button
        type="button"
        :disabled="saving"
        @click="emit('save')"
      >
        {{ saving ? '保存中…' : '保存训练节奏' }}
      </button>
    </footer>
  </section>
</template>

<style scoped>
.review-rhythm-panel {
  margin-top: 16px;
  padding: 26px;
  border: 1px solid var(--line);
  border-radius: 17px;
  background: rgba(255, 253, 247, .78);
  box-shadow: 0 16px 48px rgba(34, 48, 43, .05);
}

.review-rhythm-panel > header {
  margin-bottom: 18px;
}

.review-rhythm-panel > header p {
  margin: 0 0 8px;
  color: var(--cinnabar);
  font-size: 12px;
  font-weight: 800;
  letter-spacing: .14em;
}

.review-rhythm-panel h2 {
  margin: 0;
  color: var(--green-deep);
  font-family: Georgia, 'Microsoft YaHei', serif;
  font-size: 21px;
}

.review-rhythm-panel > header span {
  display: block;
  max-width: 720px;
  margin-top: 8px;
  color: var(--ink-muted);
  font-size: 12px;
  line-height: 1.7;
}

button {
  display: inline-flex;
  min-height: 44px;
  gap: 7px;
  align-items: center;
  padding: 10px 14px;
  border: 1px solid var(--line);
  border-radius: 10px;
  background: var(--paper-raised);
  cursor: pointer;
}

button:disabled {
  opacity: .5;
}

.rhythm-options {
  display: grid;
  grid-template-columns: repeat(3, minmax(0, 1fr));
  gap: 10px;
}

.rhythm-options label {
  position: relative;
  display: grid;
  min-height: 104px;
  grid-template-columns: 18px 1fr;
  gap: 10px;
  align-content: start;
  padding: 16px;
  border: 1px solid var(--line);
  border-radius: 7px 17px 17px;
  background: rgba(255, 253, 247, .68);
  cursor: pointer;
  transition:
    transform var(--motion-feedback) var(--ease-standard),
    border-color var(--motion-standard),
    background var(--motion-standard),
    box-shadow var(--motion-standard);
}

.rhythm-options label:hover {
  transform: translateY(-2px);
}

.rhythm-options label.selected {
  border-color: rgba(33, 51, 45, .4);
  background: var(--green-soft);
  box-shadow: 0 12px 28px rgba(33, 51, 45, .08);
}

.rhythm-options input {
  position: absolute;
  width: 1px;
  height: 1px;
  opacity: 0;
}

.rhythm-options input:focus-visible ~ .rhythm-mark {
  outline: 3px solid rgba(185, 88, 63, .35);
  outline-offset: 3px;
}

.rhythm-mark {
  width: 16px;
  height: 16px;
  margin-top: 2px;
  border: 1px solid var(--sand-deep);
  border-radius: 50%;
  background: var(--paper-raised);
  box-shadow: inset 0 0 0 4px var(--paper-raised);
}

.selected .rhythm-mark {
  border-color: var(--green-deep);
  background: var(--green-deep);
}

.rhythm-options label > span:last-child {
  display: grid;
}

.rhythm-options strong {
  color: var(--green-deep);
  font-size: 13px;
}

.rhythm-options small {
  margin-top: 7px;
  color: var(--ink-muted);
  font-size: 12px;
  line-height: 1.6;
}

.rhythm-actions {
  display: flex;
  gap: 16px;
  align-items: center;
  justify-content: space-between;
  margin-top: 16px;
}

.rhythm-actions p,
.rhythm-actions span {
  margin: 0;
  color: #557263;
  font-size: 12px;
}

.rhythm-actions button {
  color: var(--paper);
  border-color: var(--green-deep);
  background: var(--green-deep);
}

@media (max-width: 760px) {
  .review-rhythm-panel {
    padding: 22px;
  }

  .rhythm-options {
    grid-template-columns: 1fr;
  }

  .rhythm-options small,
  .rhythm-actions p,
  .rhythm-actions span {
    font-size: 12px;
  }

  .rhythm-actions {
    align-items: stretch;
    flex-direction: column;
  }

  .rhythm-actions button {
    min-height: 44px;
    justify-content: center;
  }
}

@media (prefers-reduced-motion: reduce) {
  .rhythm-options label {
    transition: none;
  }

  .rhythm-options label:hover {
    transform: none;
  }
}
</style>
