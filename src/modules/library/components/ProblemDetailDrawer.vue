<script setup lang="ts">
import { Archive, ArrowLeft, ArrowRight, BookOpenCheck, Image, LoaderCircle, MoreHorizontal, Pencil, Play, RotateCcw, Save, Trash2, X } from '@lucide/vue'
import { computed, nextTick, onBeforeUnmount, onMounted, ref, watch } from 'vue'
import ActionConfirmDialog from '../../../app/components/ActionConfirmDialog.vue'
import { type NavigationAttempt, useUnsavedChangesGuard } from '../../../app/composables/useUnsavedChangesGuard'
import { useMenuButton } from '../../../app/composables/useMenuButton'
import { acquireDialogDocumentBoundary } from '../../../app/dialog-document-boundary'
import { getDialogFocusableElements, trapDialogFocus } from '../../../app/dialog-focus'
import type { ProblemDetail } from '../../../shared/api/bindings'
import { useProblemDetailEditor } from '../composables/useProblemDetailEditor'
import { MISTAKE_REASON_TAGS } from '../domain/mistakeReasons'
import ProblemTagEditor from './ProblemTagEditor.vue'

const props = defineProps<{
  detail: ProblemDetail | undefined
  loading: boolean
  saving?: boolean
  errorMessage?: string
  previousProblemId?: string | null
  nextProblemId?: string | null
  registerNavigation?: (attempt: NavigationAttempt) => () => void
  registerContextTransition?: ((attempt: NavigationAttempt) => () => void) | undefined
}>()

const emit = defineEmits<{
  close: []
  train: [problemId: string]
  update: [input: { problemId: string; subject: string; note: string; tags: string[]; timeLimitSeconds: number | null }]
  status: [problemId: string, status: 'active' | 'archived' | 'trashed']
  navigate: [problemId: string]
}>()

const questionAssets = computed(() => props.detail?.assets.filter(asset => asset.role === 'question') ?? [])
const answerAssets = computed(() => props.detail?.assets.filter(asset => asset.role === 'answer') ?? [])
const {
  editing,
  editSubject,
  editNote,
  editTags,
  editTimeLimit,
  dirty,
  startEditing,
  prepareSubmission,
} = useProblemDetailEditor(() => props.detail)
const detailMenuKey = 'problem-detail-actions'
const {
  activeMenuKey: activeDetailMenu,
  closeMenu: closeDetailMenu,
  toggleMenu: toggleDetailMenu,
  handleMenuButtonKeydown: handleDetailMenuButtonKeydown,
  handleMenuKeydown: handleDetailMenuKeydown,
} = useMenuButton()
const drawer = ref<HTMLElement>()
const leaveBlockedMessage = ref('')
let previouslyFocused: HTMLElement | null = null
let releaseDialogBoundary: (() => void) | undefined
const timeLimitError = computed(() => {
  if (editTimeLimit.value === '') return ''
  const value = Number(editTimeLimit.value)
  return Number.isInteger(value) && value >= 1 && value <= 86_400
    ? ''
    : '请输入 1 到 86400 之间的整数，留空表示不限时。'
})
const {
  current: discardConfirmation,
  confirm: confirmDiscard,
  cancel: cancelDiscard,
  attemptLeave,
} = useUnsavedChangesGuard({
  dirty: () => dirty.value,
  busy: () => Boolean(props.saving),
  onBusy: () => {
    leaveBlockedMessage.value = '题目操作正在完成，请等待完成后再离开。'
  },
  ...(props.registerNavigation ? { registerNavigation: props.registerNavigation } : {}),
  ...(props.registerContextTransition
    ? { registerContextTransition: props.registerContextTransition }
    : {}),
  confirmation: {
    eyebrow: '未保存修改 · 离开确认',
    title: '放弃尚未保存的修改？',
    description: '刚才修改的科目、笔记、标签和答题时限都不会保存；继续编辑可保留当前输入。',
    confirmLabel: '放弃修改',
    cancelLabel: '继续编辑',
    tone: 'warning',
  },
})

watch(() => props.saving, (saving) => {
  if (!saving) leaveBlockedMessage.value = ''
})
watch(() => props.detail?.id, (problemId, previousProblemId) => {
  if (problemId !== previousProblemId) closeDetailMenu()
})

async function requestClose() {
  if (await attemptLeave()) emit('close')
}

async function requestStatus(status: 'active' | 'archived' | 'trashed') {
  const problemId = props.detail?.id
  if (problemId && await attemptLeave() && props.detail?.id === problemId) {
    closeDetailMenu()
    emit('status', problemId, status)
  }
}

async function requestNavigate(problemId: string) {
  const sourceProblemId = props.detail?.id
  if (sourceProblemId && await attemptLeave() && props.detail?.id === sourceProblemId) {
    emit('navigate', problemId)
  }
}

function saveChanges() {
  if (timeLimitError.value) return
  const input = prepareSubmission()
  if (input) emit('update', input)
}

function handleKeydown(event: KeyboardEvent) {
  if (event.key === 'Escape') {
    event.preventDefault()
    requestClose()
    return
  }
  trapDialogFocus(event, drawer.value)
}

onMounted(async () => {
  if (drawer.value) releaseDialogBoundary = acquireDialogDocumentBoundary(drawer.value)
  previouslyFocused = document.activeElement as HTMLElement | null
  await nextTick()
  getDialogFocusableElements(drawer.value)[0]?.focus()
  if (!drawer.value?.contains(document.activeElement)) drawer.value?.focus()
})
onBeforeUnmount(() => {
  releaseDialogBoundary?.()
  previouslyFocused?.focus()
})
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
        <div class="header-actions">
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
          <button
            v-if="detail && !loading && !editing"
            type="button"
            class="edit-header-button"
            @click="startEditing"
          >
            <Pencil :size="14" />
            编辑题目
          </button>
        </div>
      </header>

      <p
        v-if="leaveBlockedMessage"
        class="leave-blocked-message"
        role="alert"
      >
        {{ leaveBlockedMessage }}
      </p>

      <p
        v-if="errorMessage"
        class="detail-error"
        role="alert"
      >
        {{ errorMessage }}
      </p>
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
          <div class="edit-tags-field">
            <span>标签</span>
            <ProblemTagEditor
              v-model="editTags"
              :suggestions="MISTAKE_REASON_TAGS"
            />
          </div>
          <label>答题时限（秒）<input
            v-model="editTimeLimit"
            type="number"
            inputmode="numeric"
            min="1"
            max="86400"
            step="1"
            placeholder="留空表示不限时"
            :aria-invalid="Boolean(timeLimitError)"
            :aria-describedby="timeLimitError ? 'time-limit-error' : undefined"
          ></label>
          <small
            v-if="timeLimitError"
            id="time-limit-error"
            class="field-error"
            role="alert"
          >{{ timeLimitError }}</small>
        </section>
        <section
          v-else
          class="note-paper"
        >
          <span>复盘笔记</span>
          <p>{{ detail.note || '这道题还没有补充笔记。' }}</p>
          <div
            v-if="detail.tags.length"
            class="detail-tags"
            aria-label="题目标签"
          >
            <span
              v-for="tag in detail.tags"
              :key="tag"
            >{{ tag }}</span>
          </div>
          <small class="time-limit-copy">
            {{ detail.timeLimitSeconds ? `建议 ${detail.timeLimitSeconds} 秒内完成` : '不限制答题时间' }}
          </small>
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
        <div
          v-if="previousProblemId || nextProblemId"
          class="neighbor-actions"
          aria-label="浏览相邻题目"
        >
          <button
            type="button"
            :disabled="saving || !previousProblemId"
            @click="previousProblemId && requestNavigate(previousProblemId)"
          >
            <ArrowLeft :size="15" />
            上一题
          </button>
          <button
            type="button"
            :disabled="saving || !nextProblemId"
            @click="nextProblemId && requestNavigate(nextProblemId)"
          >
            下一题
            <ArrowRight :size="15" />
          </button>
        </div>
        <button
          v-if="!editing"
          type="button"
          class="more-actions-button"
          aria-haspopup="menu"
          aria-controls="problem-detail-actions-menu"
          :aria-expanded="activeDetailMenu === detailMenuKey"
          @click="toggleDetailMenu($event, detailMenuKey)"
          @keydown="handleDetailMenuButtonKeydown($event, detailMenuKey)"
        >
          <MoreHorizontal :size="16" />
          更多题目操作
        </button>
        <div
          v-if="activeDetailMenu === detailMenuKey && !editing"
          id="problem-detail-actions-menu"
          class="status-actions"
          role="menu"
          aria-label="更多题目操作"
          @keydown="handleDetailMenuKeydown"
        >
          <button
            v-if="detail.status === 'active'"
            type="button"
            role="menuitem"
            tabindex="-1"
            :disabled="saving"
            @click="requestStatus('archived')"
          >
            <Archive :size="15" />归档
          </button>
          <button
            v-if="detail.status !== 'trashed'"
            type="button"
            role="menuitem"
            tabindex="-1"
            :disabled="saving"
            @click="requestStatus('trashed')"
          >
            <Trash2 :size="15" />移入回收站
          </button>
          <button
            v-else
            type="button"
            role="menuitem"
            tabindex="-1"
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
          :disabled="saving || Boolean(timeLimitError)"
          @click="saveChanges"
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
  <ActionConfirmDialog
    v-if="discardConfirmation"
    :request="discardConfirmation"
    @cancel="cancelDiscard"
    @confirm="confirmDiscard"
  />
</template>

<style scoped>
.detail-layer { position: fixed; z-index: 60; inset: 0; display: flex; justify-content: flex-end; background: rgba(34,48,43,.22); backdrop-filter: blur(3px); animation: fade-in var(--motion-standard) var(--ease-standard); }
.detail-drawer { overflow-y: auto; width: min(680px,92vw); height: 100%; padding: 30px 34px 38px; border-left: 1px solid rgba(34,48,43,.12); background: var(--paper); box-shadow: -22px 0 60px rgba(34,48,43,.16); animation: drawer-in var(--motion-page) var(--ease-standard); }
.detail-header { display: flex; justify-content: space-between; gap: 24px; align-items: flex-start; padding-bottom: 22px; border-bottom: 1px solid var(--line); }
.detail-header p { margin: 0 0 5px; color: var(--cinnabar); font-size: 12px; font-weight: 760; letter-spacing: .14em; }
.detail-header h2 { margin: 0; font-size: 34px; letter-spacing: -.04em; }
.header-actions { display: flex; gap: 9px; align-items: center; }
.icon-button { display: grid; width: 44px; height: 44px; place-items: center; color: var(--ink); border: 1px solid var(--line); border-radius: 50%; background: rgba(255,253,247,.7); cursor: pointer; }
.edit-header-button, .more-actions-button { display: inline-flex; gap: 6px; align-items: center; min-height: 44px; padding: 0 13px; color: var(--green-deep); border: 1px solid var(--line); border-radius: 999px; background: rgba(255,253,247,.74); cursor: pointer; font-weight: 700; }
.detail-loading { display: flex; gap: 10px; align-items: center; justify-content: center; min-height: 280px; color: var(--ink-muted); }
.detail-loading svg { animation: spin .9s linear infinite; }
.detail-error { margin-top: 24px; padding: 14px; color: #7f3829; border: 1px solid rgba(185,88,63,.25); border-radius: 10px; background: rgba(185,88,63,.08); }
.leave-blocked-message { margin: 18px 0 0; padding: 12px 14px; color: #7f3829; border: 1px solid rgba(185,88,63,.25); border-radius: 10px; background: rgba(185,88,63,.08); font-size: 13px; font-weight: 700; }
.note-paper { margin: 24px 0 30px; padding: 18px 20px; border-radius: 3px 14px 14px 14px; background: var(--green-soft); }
.note-paper span { color: #567064; font-size: 12px; font-weight: 760; letter-spacing: .1em; }
.note-paper p { margin: 8px 0 0; line-height: 1.65; }
.time-limit-copy { display: inline-block; margin-top: 10px; color: var(--ink-muted); }
.edit-paper { display: grid; gap: 14px; margin: 24px 0 30px; padding: 18px 20px; border-radius: 3px 14px 14px 14px; background: var(--green-soft); }
.edit-paper label { display: grid; gap: 7px; color: #567064; font-size: 12px; font-weight: 760; letter-spacing: .08em; }
.edit-tags-field { display: grid; gap: 7px; color: #567064; font-size: 12px; font-weight: 760; letter-spacing: .08em; }
.detail-tags { display: flex; flex-wrap: wrap; gap: 7px; margin-top: 13px; }
.detail-tags span { padding: 5px 9px; color: var(--green-deep); border: 1px solid rgba(33,51,45,.12); border-radius: 999px; background: rgba(255,253,247,.62); font-size: 12px; font-weight: 720; letter-spacing: 0; }
.field-error { color: #8d3f2f; font-size: 12px; letter-spacing: 0; }
.edit-paper input, .edit-paper textarea { width: 100%; min-height: 44px; padding: 10px 12px; color: var(--ink); border: 1px solid rgba(33,51,45,.18); border-radius: 9px; outline: none; background: rgba(255,253,247,.8); font: inherit; font-size: 14px; font-weight: 500; letter-spacing: 0; resize: vertical; }
.asset-section { margin-top: 28px; }
.asset-section h3 { display: flex; gap: 8px; align-items: center; margin: 0 0 12px; font-size: 14px; letter-spacing: .05em; }
.answer-section { padding-top: 24px; border-top: 1px solid var(--line); }
.asset-stack { display: grid; gap: 12px; }
.asset-stack img { display: block; width: 100%; max-height: 680px; object-fit: contain; border: 1px solid var(--line); border-radius: 3px 14px 14px 14px; background: white; }
.missing-copy { color: var(--ink-muted); font-size: 13px; }
.detail-footer { position: sticky; bottom: -38px; margin: 34px -34px -38px; padding: 18px 34px 24px; border-top: 1px solid var(--line); background: rgba(246,241,231,.94); backdrop-filter: blur(12px); }
.neighbor-actions { display: flex; justify-content: space-between; gap: 9px; margin-bottom: 10px; }
.neighbor-actions button { display: inline-flex; gap: 6px; align-items: center; min-height: 44px; padding: 0 11px; color: var(--green-deep); border: 1px solid var(--line); border-radius: 999px; background: rgba(255,253,247,.74); cursor: pointer; }
.neighbor-actions button:disabled { opacity: .4; cursor: default; }
.more-actions-button { min-height: 44px; margin-bottom: 10px; color: var(--ink-muted); }
.status-actions { display: flex; gap: 9px; margin-bottom: 12px; }
.status-actions button { display: inline-flex; gap: 6px; align-items: center; min-height: 44px; padding: 0 12px; color: var(--ink-muted); border: 1px solid var(--line); border-radius: 999px; background: rgba(255,253,247,.74); cursor: pointer; }
.status-actions button:disabled, .train-button:disabled { cursor: wait; opacity: .5; }
.train-button { display: flex; gap: 9px; align-items: center; justify-content: center; width: 100%; min-height: 48px; color: var(--paper); border: 0; border-radius: 999px; background: var(--green-deep); font-weight: 740; cursor: pointer; }
@keyframes fade-in { from { opacity: 0; } }
@keyframes drawer-in { from { transform: translateX(28px); opacity: .6; } }
@keyframes spin { to { transform: rotate(360deg); } }
@media (max-width: 620px) { .detail-drawer { width: 100%; padding: 24px 20px 32px; } .detail-footer { bottom: -32px; margin: 30px -20px -32px; padding: 16px 20px 22px; } }
@media (prefers-reduced-motion: reduce) { .detail-layer, .detail-drawer, .detail-loading svg { animation: none; } }
</style>
