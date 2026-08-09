<script setup lang="ts">
import { X } from '@lucide/vue'
import { ref } from 'vue'

const props = withDefaults(defineProps<{
  modelValue: string[]
  disabled?: boolean
}>(), {
  disabled: false,
})

const emit = defineEmits<{
  'update:modelValue': [tags: string[]]
}>()

const draft = ref('')
const errorMessage = ref('')

function addDraft() {
  const tag = draft.value.trim()
  if (!tag) {
    draft.value = ''
    return
  }
  if ([...tag].length > 30) {
    errorMessage.value = '每个标签最多 30 个字。'
    return
  }
  if (props.modelValue.includes(tag)) {
    draft.value = ''
    errorMessage.value = ''
    return
  }
  if (props.modelValue.length >= 20) {
    errorMessage.value = '每道题最多添加 20 个标签。'
    return
  }
  emit('update:modelValue', [...props.modelValue, tag])
  draft.value = ''
  errorMessage.value = ''
}

function removeTag(index: number) {
  emit('update:modelValue', props.modelValue.filter((_, tagIndex) => tagIndex !== index))
  errorMessage.value = ''
}

function handleKeydown(event: KeyboardEvent) {
  if (event.key === 'Enter' || event.key === ',' || event.key === '，') {
    event.preventDefault()
    addDraft()
    return
  }
  if (event.key === 'Backspace' && draft.value === '' && props.modelValue.length > 0) {
    event.preventDefault()
    removeTag(props.modelValue.length - 1)
  }
}
</script>

<template>
  <div class="tag-editor">
    <TransitionGroup
      name="tag-chip"
      tag="div"
      class="tag-editor__chips"
    >
      <span
        v-for="(tag, index) in modelValue"
        :key="tag"
        class="tag-chip"
      >
        {{ tag }}
        <button
          type="button"
          :aria-label="`删除标签 ${tag}`"
          :disabled="disabled"
          @click="removeTag(index)"
        >
          <X
            :size="12"
            aria-hidden="true"
          />
        </button>
      </span>
    </TransitionGroup>
    <input
      v-model="draft"
      aria-label="标签"
      :disabled="disabled"
      maxlength="31"
      placeholder="输入后按回车添加"
      @keydown="handleKeydown"
    >
    <small>回车或逗号添加，最多 20 个</small>
    <p
      v-if="errorMessage"
      class="tag-editor__error"
      role="alert"
    >
      {{ errorMessage }}
    </p>
  </div>
</template>

<style scoped>
.tag-editor { display: grid; gap: 8px; }
.tag-editor__chips { display: flex; flex-wrap: wrap; gap: 7px; min-height: 0; }
.tag-chip { display: inline-flex; gap: 5px; align-items: center; max-width: 100%; min-height: 44px; padding: 0 2px 0 12px; overflow-wrap: anywhere; color: var(--green-deep); border: 1px solid rgba(33,51,45,.14); border-radius: 999px; background: rgba(255,253,247,.82); font-size: 12px; font-weight: 720; }
.tag-chip button { display: grid; width: 44px; height: 44px; padding: 0; place-items: center; color: inherit; border: 0; border-radius: 50%; background: rgba(33,51,45,.07); cursor: pointer; transition: transform var(--motion-feedback) var(--ease-standard), background var(--motion-feedback) var(--ease-standard); }
.tag-chip button:hover:not(:disabled) { background: rgba(185,88,63,.16); transform: scale(1.08); }
.tag-editor > input { width: 100%; min-height: 44px; padding: 10px 12px; color: var(--ink); border: 1px solid rgba(33,51,45,.18); border-radius: 9px; outline: none; background: rgba(255,253,247,.8); font: inherit; font-size: 14px; font-weight: 500; }
.tag-editor > input:focus { border-color: var(--green-deep); box-shadow: 0 0 0 3px rgba(33,51,45,.09); }
.tag-editor small { color: var(--ink-muted); font-size: 12px; font-weight: 500; letter-spacing: 0; }
.tag-editor__error { margin: 0; color: #8d3f2f; font-size: 12px; font-weight: 600; letter-spacing: 0; }
.tag-chip-enter-active, .tag-chip-leave-active { transition: opacity var(--motion-standard) var(--ease-standard), transform var(--motion-standard) var(--ease-standard); }
.tag-chip-enter-from, .tag-chip-leave-to { opacity: 0; transform: translateY(4px) scale(.92); }
@media (prefers-reduced-motion: reduce) { .tag-chip-enter-active, .tag-chip-leave-active, .tag-chip button { transition: none; } }
</style>
