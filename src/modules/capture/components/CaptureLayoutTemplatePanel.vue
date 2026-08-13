<script setup lang="ts">
import { LayoutGrid, Sparkles } from '@lucide/vue'
import { computed, nextTick, onBeforeUnmount, ref, watch } from 'vue'
import { acquireDialogDocumentBoundary } from '@/shared/ui/dialog-document-boundary'
import { trapDialogFocus } from '@/shared/ui/dialog-focus'
import type { CaptureLayoutMode } from '../../../shared/api/bindings'

const props = defineProps<{
  itemCount: number
  draftCount: number
  affectedNoteCount: number
  busy: boolean
}>()

const emit = defineEmits<{
  apply: [mode: CaptureLayoutMode, questions: number, answers: number, splitIndex: number | null]
}>()

const layoutMode = ref<CaptureLayoutMode>('alternating')
const questionImages = ref<number | ''>(1)
const answerImages = ref<number | ''>(1)
const splitIndex = ref<number | ''>(0)
const confirmOpen = ref(false)
const impactDialog = ref<HTMLElement>()
const returnButton = ref<HTMLButtonElement>()
const launcher = ref<HTMLButtonElement>()
let focusReturn: HTMLElement | null = null
let releaseDocumentBoundary: (() => void) | undefined

function normalizedInteger(value: number | '') {
  return typeof value === 'number' && Number.isInteger(value) ? value : null
}

const normalizedQuestionImages = computed(() => normalizedInteger(questionImages.value))
const normalizedAnswerImages = computed(() => normalizedInteger(answerImages.value))
const normalizedSplitIndex = computed(() => normalizedInteger(splitIndex.value))
const questionImagesValid = computed(() => {
  const value = normalizedQuestionImages.value
  return value !== null && value >= 1 && value <= 10
})
const answerImagesValid = computed(() => {
  const value = normalizedAnswerImages.value
  return value !== null && value >= 1 && value <= 10
})
const splitIndexValid = computed(() => {
  const value = normalizedSplitIndex.value
  return value !== null && value >= 1 && value <= props.itemCount
})
const layoutValidationMessage = computed(() => {
  if (layoutMode.value === 'alternating' && (!questionImagesValid.value || !answerImagesValid.value)) {
    return '题图/题和答案/题必须是 1–10 的整数。'
  }
  if (layoutMode.value === 'split' && props.itemCount > 0 && !splitIndexValid.value) {
    return `分开位置必须是 1–${props.itemCount} 的整数。`
  }
  return ''
})
const canApply = computed(() => props.itemCount > 0 && !props.busy && !layoutValidationMessage.value)

watch(
  () => props.itemCount,
  itemCount => {
    splitIndex.value = Math.ceil(itemCount / 2)
  },
  { immediate: true },
)

function releaseBoundary() {
  releaseDocumentBoundary?.()
  releaseDocumentBoundary = undefined
}

function requestApply() {
  if (!canApply.value) return
  if (!props.draftCount) {
    applyRequestedLayout()
    return
  }

  focusReturn = document.activeElement instanceof HTMLElement
    ? document.activeElement
    : launcher.value ?? null
  releaseBoundary()
  confirmOpen.value = true
  void nextTick(() => {
    if (!confirmOpen.value || !impactDialog.value) return
    releaseDocumentBoundary = acquireDialogDocumentBoundary(impactDialog.value)
    returnButton.value?.focus()
  })
}

async function closeConfirmation() {
  if (props.busy) return
  confirmOpen.value = false
  releaseBoundary()
  const target = focusReturn
  focusReturn = null
  await nextTick()
  ;(target?.isConnected ? target : launcher.value)?.focus()
}

function applyRequestedLayout() {
  if (!canApply.value) return
  if (confirmOpen.value) void closeConfirmation()
  const nextQuestionImages = normalizedQuestionImages.value ?? 1
  const nextAnswerImages = normalizedAnswerImages.value ?? 1
  const nextSplitIndex = layoutMode.value === 'split' ? normalizedSplitIndex.value : null
  if (layoutMode.value === 'split' && nextSplitIndex === null) return
  emit(
    'apply',
    layoutMode.value,
    nextQuestionImages,
    nextAnswerImages,
    nextSplitIndex,
  )
}

function handleImpactKeydown(event: KeyboardEvent) {
  if (event.key === 'Escape' && !props.busy) {
    event.preventDefault()
    void closeConfirmation()
    return
  }
  trapDialogFocus(event, impactDialog.value)
}

onBeforeUnmount(releaseBoundary)
</script>

<template>
  <section
    id="capture-layout-templates"
    class="layout-bar"
  >
    <div class="layout-heading">
      <Sparkles :size="19" />
      <div>
        <h2>顺序模板</h2>
        <p>模板只按图片顺序分组，可撤销重做，不识别图片内容。</p>
      </div>
    </div>
    <select
      v-model="layoutMode"
      aria-label="整理模板"
    >
      <option value="alternating">
        题1、答1、题2、答2
      </option>
      <option value="split">
        前半题图、后半答案
      </option>
      <option value="questions_only">
        每张图是一道题
      </option>
      <option value="manual">
        全部手工整理
      </option>
    </select>
    <label v-if="layoutMode === 'alternating'">题图/题<input
      v-model.number="questionImages"
      type="number"
      min="1"
      max="10"
      :aria-invalid="!questionImagesValid"
      :aria-describedby="layoutValidationMessage ? 'layout-validation-message' : undefined"
    ></label>
    <label v-if="layoutMode === 'alternating'">答案/题<input
      v-model.number="answerImages"
      type="number"
      min="1"
      max="10"
      :aria-invalid="!answerImagesValid"
      :aria-describedby="layoutValidationMessage ? 'layout-validation-message' : undefined"
    ></label>
    <label v-if="layoutMode === 'split'">从第几张分开<input
      v-model.number="splitIndex"
      type="number"
      min="1"
      :max="itemCount"
      :aria-invalid="!splitIndexValid"
      :aria-describedby="layoutValidationMessage ? 'layout-validation-message' : undefined"
    ></label>
    <button
      ref="launcher"
      type="button"
      :disabled="!canApply"
      @click="requestApply"
    >
      <LayoutGrid :size="16" />{{ draftCount ? '重新分组全部图片' : '按模板生成题卡' }}
    </button>
    <p
      v-if="layoutValidationMessage"
      id="layout-validation-message"
      class="layout-validation"
      role="alert"
    >
      {{ layoutValidationMessage }}
    </p>
  </section>

  <section
    v-if="confirmOpen"
    ref="impactDialog"
    class="layout-impact"
    role="alertdialog"
    aria-modal="true"
    aria-labelledby="layout-impact-title"
    tabindex="-1"
    @keydown.stop="handleImpactKeydown"
  >
    <h2 id="layout-impact-title">
      确认重新分组全部图片
    </h2>
    <p>{{ draftCount }} 张题卡会被重新生成；{{ affectedNoteCount }} 张含标签或笔记的题卡会清空这些逐题信息。</p>
    <strong>{{ itemCount }} 张图片都会保留，不会删除原图或切图。</strong>
    <div>
      <button
        type="button"
        :disabled="!canApply"
        @click="applyRequestedLayout"
      >
        确认重新分组
      </button>
      <button
        ref="returnButton"
        type="button"
        :disabled="busy"
        @click="closeConfirmation"
      >
        返回
      </button>
    </div>
  </section>
</template>

<style scoped>
.layout-bar { display: flex; gap: 11px; align-items: end; flex-wrap: wrap; margin-top: 25px; padding: 17px; border: 1px solid var(--line); border-radius: 14px; background: rgba(255,253,247,.66); }
.layout-heading { display: flex; flex: 1; gap: 10px; align-items: flex-start; }
.layout-heading h2,.layout-heading p { margin: 0; }
.layout-heading h2 { font-size: 16px; }
.layout-heading p { margin-top: 3px; color: var(--ink-muted); font-size: 12px; }
.layout-bar label { display: grid; gap: 6px; color: var(--ink-muted); font-size: 12px; font-weight: 720; }
.layout-bar select { min-width: 210px; }
.layout-bar input { width: 78px; }
.layout-bar input,.layout-bar select { min-height: 44px; box-sizing: border-box; padding: 10px 12px; color: var(--ink); border: 1px solid var(--line); border-radius: 10px; outline: none; background: rgba(246,241,231,.66); font: inherit; }
.layout-bar input[aria-invalid="true"] { border-color: var(--cinnabar); box-shadow: 0 0 0 3px rgba(185,88,63,.12); }
.layout-bar>button { display: inline-flex; gap: 8px; align-items: center; justify-content: center; min-height: 44px; padding: 0 17px; color: var(--paper); border: 0; border-radius: 999px; background: var(--green-deep); font-weight: 740; cursor: pointer; }
.layout-bar button:disabled { cursor: not-allowed; opacity: .4; }
.layout-validation { flex-basis: 100%; margin: 0; color: #873d2f; font-size: 12px; font-weight: 720; line-height: 1.45; }
.layout-impact { position: fixed; z-index: 95; top: 50%; left: 50%; width: min(520px,calc(100vw - 40px)); padding: 24px; box-sizing: border-box; border: 1px solid rgba(33,51,45,.18); border-radius: 18px; background: var(--paper); box-shadow: 0 0 0 9999px rgba(25,35,31,.46),0 24px 70px rgba(20,31,27,.28); transform: translate(-50%,-50%); }
.layout-impact h2,.layout-impact p,.layout-impact strong { display: block; margin: 0; }
.layout-impact h2 { font-size: 23px; }
.layout-impact p { margin-top: 12px; color: var(--ink-muted); font-size: 12px; line-height: 1.6; }
.layout-impact strong { margin-top: 9px; color: var(--green-deep); font-size: 12px; }
.layout-impact>div { display: flex; gap: 9px; justify-content: flex-end; margin-top: 20px; }
.layout-impact button { min-height: 44px; padding: 0 15px; border: 1px solid var(--line); border-radius: 999px; background: var(--paper); font-weight: 760; cursor: pointer; }
.layout-impact button:first-child { color: var(--paper); border-color: var(--green-deep); background: var(--green-deep); }
.layout-impact button:disabled { cursor: not-allowed; opacity: .4; }

@media (max-width: 980px) { .layout-bar { align-items: stretch; } }
@media (max-width: 720px) { .layout-bar select { min-width: 100%; } }
</style>
