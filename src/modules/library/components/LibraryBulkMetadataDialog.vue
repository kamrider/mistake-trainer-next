<script setup lang="ts">
import { computed, nextTick, onBeforeUnmount, ref, watch } from 'vue'
import { acquireDialogDocumentBoundary } from '../../../app/dialog-document-boundary'
import { trapDialogFocus } from '../../../app/dialog-focus'

const props = withDefaults(defineProps<{
  open: boolean
  selectedCount: number
  busy?: boolean
}>(), { busy: false })

const emit = defineEmits<{
  close: []
  submit: [change: { subject: string | null; addTags: string[]; removeTags: string[] }]
}>()

const replaceSubject = ref(false)
const subject = ref('')
const addTagsText = ref('')
const removeTagsText = ref('')
const validationMessage = ref('')
const dialog = ref<HTMLElement>()
let releaseDialogBoundary: (() => void) | undefined
const normalizedAddTags = computed(() => normalizeTags(addTagsText.value))
const normalizedRemoveTags = computed(() => normalizeTags(removeTagsText.value))

function releaseBoundary() {
  releaseDialogBoundary?.()
  releaseDialogBoundary = undefined
}

watch(() => props.open, async (open) => {
  releaseBoundary()
  if (!open) return
  replaceSubject.value = false
  subject.value = ''
  addTagsText.value = ''
  removeTagsText.value = ''
  validationMessage.value = ''
  await nextTick()
  if (!dialog.value || !props.open) return
  releaseDialogBoundary = acquireDialogDocumentBoundary(dialog.value)
  dialog.value.querySelector<HTMLInputElement>('input:not(:disabled)')?.focus()
}, { immediate: true })

function close() {
  if (!props.busy) emit('close')
}

function handleKeydown(event: KeyboardEvent) {
  if (event.key === 'Escape' && !props.busy) {
    event.preventDefault()
    close()
  }
  if (event.key === 'Tab') trapDialogFocus(event, dialog.value)
}

onBeforeUnmount(releaseBoundary)

function normalizeTags(value: string) {
  return [...new Set(value.split(/[,，\n]/u).map(tag => tag.trim()).filter(Boolean))]
}

function submit() {
  if (!replaceSubject.value && !normalizedAddTags.value.length && !normalizedRemoveTags.value.length) {
    validationMessage.value = '请至少填写一项要修改的内容。'
    return
  }
  validationMessage.value = ''
  emit('submit', {
    subject: replaceSubject.value ? subject.value.trim() : null,
    addTags: normalizedAddTags.value,
    removeTags: normalizedRemoveTags.value,
  })
}
</script>

<template>
  <div
    v-if="open"
    class="dialog-backdrop"
    @mousedown.self="close"
  >
    <section
      ref="dialog"
      class="dialog"
      role="dialog"
      aria-modal="true"
      aria-labelledby="bulk-metadata-title"
      @keydown="handleKeydown"
    >
      <p class="eyebrow">
        原子批量修改
      </p>
      <h2 id="bulk-metadata-title">
        修改已选 {{ selectedCount }} 道题
      </h2>
      <p class="intro">
        所有题目会一起成功或一起保持原样，不会留下改到一半的状态。
      </p>
      <label class="subject-toggle">
        <input
          v-model="replaceSubject"
          type="checkbox"
          :disabled="busy"
        >
        统一修改科目
      </label>
      <label v-if="replaceSubject">
        新科目
        <input
          v-model="subject"
          type="text"
          maxlength="40"
          :disabled="busy"
          placeholder="留空可清除科目"
        >
      </label>
      <label>
        添加标签
        <textarea
          v-model="addTagsText"
          :disabled="busy"
          placeholder="用逗号或换行分隔，最多 20 个"
        />
      </label>
      <label>
        移除标签
        <textarea
          v-model="removeTagsText"
          :disabled="busy"
          placeholder="用逗号或换行分隔"
        />
      </label>
      <p
        v-if="validationMessage"
        role="alert"
        class="validation"
      >
        {{ validationMessage }}
      </p>
      <div class="actions">
        <button
          type="button"
          :disabled="busy"
          @click="close"
        >
          取消
        </button>
        <button
          type="button"
          class="primary"
          :disabled="busy"
          @click="submit"
        >
          {{ busy ? '正在批量修改…' : '确认批量修改' }}
        </button>
      </div>
    </section>
  </div>
</template>

<style scoped>
.dialog-backdrop { position: fixed; z-index: 80; inset: 0; display: grid; padding: 24px; place-items: center; background: rgba(27,36,33,.38); backdrop-filter: blur(6px); }
.dialog { display: grid; width: min(520px,100%); gap: 15px; padding: 26px; border: 1px solid var(--line); border-radius: 5px 22px 22px 22px; background: #fffdf7; box-shadow: 0 28px 80px rgba(22,31,28,.28); }
.eyebrow { margin: 0; color: var(--cinnabar); font-size: 12px; font-weight: 760; letter-spacing: .1em; }
h2 { margin: -4px 0 0; }
.intro { margin: 0; color: var(--ink-muted); line-height: 1.55; }
label { display: grid; gap: 7px; font-size: 13px; font-weight: 720; }
.subject-toggle { display: flex; align-items: center; }
input[type="text"], textarea { width: 100%; padding: 10px 12px; border: 1px solid var(--line); border-radius: 10px; background: white; font: inherit; }
textarea { min-height: 68px; resize: vertical; }
.validation { margin: 0; color: #7f3829; }
.actions { display: flex; gap: 9px; justify-content: flex-end; }
.actions button { min-height: 42px; padding: 0 15px; border: 1px solid var(--line); border-radius: 999px; background: white; cursor: pointer; }
.actions .primary { color: var(--paper); border-color: var(--green-deep); background: var(--green-deep); }
</style>
