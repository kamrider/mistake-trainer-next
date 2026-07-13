<script setup lang="ts">
import { Archive, BookOpenCheck, Image, LoaderCircle, Pencil, Play, RotateCcw, Save, Trash2, X } from '@lucide/vue'
import { computed, nextTick, onBeforeUnmount, onMounted, ref, watch } from 'vue'
import type { ProblemDetail } from '../../../shared/api/bindings'

const props = defineProps<{
  detail: ProblemDetail | undefined
  loading: boolean
  saving?: boolean
  errorMessage?: string
}>()

const emit = defineEmits<{
  close: []
  train: [problemId: string]
  update: [input: { problemId: string; subject: string; note: string }]
  status: [problemId: string, status: 'active' | 'archived' | 'trashed']
}>()

const questionAssets = computed(() => props.detail?.assets.filter(asset => asset.role === 'question') ?? [])
const answerAssets = computed(() => props.detail?.assets.filter(asset => asset.role === 'answer') ?? [])
const editing = ref(false)
const editSubject = ref('')
const editNote = ref('')
const drawer = ref<HTMLElement>()
let previouslyFocused: HTMLElement | null = null
const dirty = computed(() => Boolean(props.detail && editing.value
  && (editSubject.value !== props.detail.subject || editNote.value !== props.detail.note)))

watch(() => props.detail, (detail) => {
  editSubject.value = detail?.subject ?? ''
  editNote.value = detail?.note ?? ''
  editing.value = false
}, { immediate: true })

function confirmDiscard() {
  return !dirty.value || window.confirm('当前修改还没有保存，确定要放弃吗？')
}

function requestClose() {
  if (confirmDiscard()) emit('close')
}

function requestStatus(status: 'active' | 'archived' | 'trashed') {
  if (props.detail && confirmDiscard()) emit('status', props.detail.id, status)
}

function focusableElements() {
  return drawer.value
    ? [...drawer.value.querySelectorAll<HTMLElement>('button:not(:disabled), input:not(:disabled), textarea:not(:disabled), [tabindex="0"]')]
    : []
}

function handleKeydown(event: KeyboardEvent) {
  if (event.key === 'Escape') {
    event.preventDefault()
    requestClose()
    return
  }
  if (event.key !== 'Tab' || !drawer.value) return
  const focusable = focusableElements()
  if (focusable.length === 0) return
  const first = focusable[0]
  const last = focusable[focusable.length - 1]
  if (event.shiftKey && (document.activeElement === first || document.activeElement === drawer.value)) {
    event.preventDefault()
    last?.focus()
  }
  else if (!event.shiftKey && document.activeElement === last) {
    event.preventDefault()
    first?.focus()
  }
}

onMounted(async () => {
  previouslyFocused = document.activeElement as HTMLElement | null
  await nextTick()
  focusableElements()[0]?.focus()
  if (!drawer.value?.contains(document.activeElement)) drawer.value?.focus()
})
onBeforeUnmount(() => previouslyFocused?.focus())
</script>

<template>
  <div
    class="detail-layer"
    @click.self="requestClose"
  >
    <aside
      ref="drawer"
      class="detail-drawer"
      role="dialog"
      aria-modal="true"
      aria-labelledby="problem-detail-title"
      tabindex="-1"
      @keydown="handleKeydown"
    >
      <header class="detail-header">
        <div>
          <p>错题原页</p>
          <h2 id="problem-detail-title">
            {{ detail?.subject || '未分类' }}
          </h2>
        </div>
        <button
          type="button"
          class="icon-button"
          aria-label="关闭题目详情"
          @click="requestClose"
        >
          <X
            :size="20"
            aria-hidden="true"
          />
        </button>
      </header>

      <div
        v-if="loading"
        class="detail-loading"
        aria-live="polite"
      >
        <LoaderCircle
          :size="24"
          aria-hidden="true"
        />
        正在解密题目图片…
      </div>
      <p
        v-else-if="errorMessage"
        class="detail-error"
        role="alert"
      >
        {{ errorMessage }}
      </p>
      <template v-else-if="detail">
        <section
          v-if="editing"
          class="edit-paper"
        >
          <label>科目<input
            v-model="editSubject"
            maxlength="40"
          ></label>
          <label>复盘笔记<textarea
            v-model="editNote"
            maxlength="2000"
            rows="5"
          /></label>
        </section>
        <section
          v-else
          class="note-paper"
        >
          <span>复盘笔记</span>
          <p>{{ detail.note || '这道题还没有补充笔记。' }}</p>
          <button
            type="button"
            class="text-button"
            @click="editing = true"
          >
            <Pencil :size="14" />编辑题目
          </button>
        </section>

        <section class="asset-section">
          <h3>
            <Image
              :size="16"
              aria-hidden="true"
            />题目
          </h3>
          <div class="asset-stack">
            <img
              v-for="(asset, index) in questionAssets"
              :key="asset.id"
              :src="asset.dataUrl"
              :alt="`题目图片 ${index + 1}`"
            >
          </div>
        </section>

        <section class="asset-section answer-section">
          <h3>
            <BookOpenCheck
              :size="16"
              aria-hidden="true"
            />答案
          </h3>
          <div class="asset-stack">
            <img
              v-for="(asset, index) in answerAssets"
              :key="asset.id"
              :src="asset.dataUrl"
              :alt="`答案图片 ${index + 1}`"
            >
          </div>
          <p
            v-if="answerAssets.length === 0"
            class="missing-copy"
          >
            尚未录入答案图片。
          </p>
        </section>
      </template>

      <footer
        v-if="detail && !loading"
        class="detail-footer"
      >
        <div class="status-actions">
          <button
            v-if="detail.status === 'active'"
            type="button"
            :disabled="saving"
            @click="requestStatus('archived')"
          >
            <Archive :size="15" />归档
          </button>
          <button
            v-if="detail.status !== 'trashed'"
            type="button"
            :disabled="saving"
            @click="requestStatus('trashed')"
          >
            <Trash2 :size="15" />移入回收站
          </button>
          <button
            v-else
            type="button"
            :disabled="saving"
            @click="requestStatus('active')"
          >
            <RotateCcw :size="15" />恢复学习
          </button>
        </div>
        <button
          v-if="editing"
          type="button"
          class="train-button"
          :disabled="saving"
          @click="$emit('update', { problemId: detail.id, subject: editSubject, note: editNote })"
        >
          <Save
            :size="17"
            aria-hidden="true"
          />
          保存修改
        </button>
        <button
          v-else
          type="button"
          class="train-button"
          :disabled="saving || detail.status !== 'active'"
          @click="$emit('train', detail.id)"
        >
          <Play
            :size="17"
            aria-hidden="true"
          />
          用这道题开始训练
        </button>
      </footer>
    </aside>
  </div>
</template>

<style scoped>
.detail-layer { position: fixed; z-index: 60; inset: 0; display: flex; justify-content: flex-end; background: rgba(34,48,43,.22); backdrop-filter: blur(3px); animation: fade-in var(--motion-standard) var(--ease-standard); }
.detail-drawer { overflow-y: auto; width: min(680px,92vw); height: 100%; padding: 30px 34px 38px; border-left: 1px solid rgba(34,48,43,.12); background: var(--paper); box-shadow: -22px 0 60px rgba(34,48,43,.16); animation: drawer-in var(--motion-page) var(--ease-standard); }
.detail-header { display: flex; justify-content: space-between; gap: 24px; align-items: flex-start; padding-bottom: 22px; border-bottom: 1px solid var(--line); }
.detail-header p { margin: 0 0 5px; color: var(--cinnabar); font-size: 11px; font-weight: 760; letter-spacing: .14em; }
.detail-header h2 { margin: 0; font-size: 34px; letter-spacing: -.04em; }
.icon-button { display: grid; width: 40px; height: 40px; place-items: center; color: var(--ink); border: 1px solid var(--line); border-radius: 50%; background: rgba(255,253,247,.7); cursor: pointer; }
.detail-loading { display: flex; gap: 10px; align-items: center; justify-content: center; min-height: 280px; color: var(--ink-muted); }
.detail-loading svg { animation: spin .9s linear infinite; }
.detail-error { margin-top: 24px; padding: 14px; color: #7f3829; border: 1px solid rgba(185,88,63,.25); border-radius: 10px; background: rgba(185,88,63,.08); }
.note-paper { margin: 24px 0 30px; padding: 18px 20px; border-radius: 3px 14px 14px 14px; background: var(--green-soft); }
.note-paper span { color: #567064; font-size: 11px; font-weight: 760; letter-spacing: .1em; }
.note-paper p { margin: 8px 0 0; line-height: 1.65; }
.text-button { display: inline-flex; gap: 6px; align-items: center; margin-top: 14px; padding: 0; color: var(--green-deep); border: 0; background: transparent; cursor: pointer; font-weight: 700; }
.edit-paper { display: grid; gap: 14px; margin: 24px 0 30px; padding: 18px 20px; border-radius: 3px 14px 14px 14px; background: var(--green-soft); }
.edit-paper label { display: grid; gap: 7px; color: #567064; font-size: 11px; font-weight: 760; letter-spacing: .08em; }
.edit-paper input, .edit-paper textarea { width: 100%; padding: 10px 12px; color: var(--ink); border: 1px solid rgba(33,51,45,.18); border-radius: 9px; outline: none; background: rgba(255,253,247,.8); font: inherit; font-size: 14px; font-weight: 500; letter-spacing: 0; resize: vertical; }
.asset-section { margin-top: 28px; }
.asset-section h3 { display: flex; gap: 8px; align-items: center; margin: 0 0 12px; font-size: 14px; letter-spacing: .05em; }
.answer-section { padding-top: 24px; border-top: 1px solid var(--line); }
.asset-stack { display: grid; gap: 12px; }
.asset-stack img { display: block; width: 100%; max-height: 680px; object-fit: contain; border: 1px solid var(--line); border-radius: 3px 14px 14px 14px; background: white; }
.missing-copy { color: var(--ink-muted); font-size: 13px; }
.detail-footer { position: sticky; bottom: -38px; margin: 34px -34px -38px; padding: 18px 34px 24px; border-top: 1px solid var(--line); background: rgba(246,241,231,.94); backdrop-filter: blur(12px); }
.status-actions { display: flex; gap: 9px; margin-bottom: 12px; }
.status-actions button { display: inline-flex; gap: 6px; align-items: center; min-height: 36px; padding: 0 12px; color: var(--ink-muted); border: 1px solid var(--line); border-radius: 999px; background: rgba(255,253,247,.74); cursor: pointer; }
.status-actions button:disabled, .train-button:disabled { cursor: wait; opacity: .5; }
.train-button { display: flex; gap: 9px; align-items: center; justify-content: center; width: 100%; min-height: 48px; color: var(--paper); border: 0; border-radius: 999px; background: var(--green-deep); font-weight: 740; cursor: pointer; }
@keyframes fade-in { from { opacity: 0; } }
@keyframes drawer-in { from { transform: translateX(28px); opacity: .6; } }
@keyframes spin { to { transform: rotate(360deg); } }
@media (max-width: 620px) { .detail-drawer { width: 100%; padding: 24px 20px 32px; } .detail-footer { bottom: -32px; margin: 30px -20px -32px; padding: 16px 20px 22px; } }
@media (prefers-reduced-motion: reduce) { .detail-layer, .detail-drawer, .detail-loading svg { animation: none; } }
</style>
