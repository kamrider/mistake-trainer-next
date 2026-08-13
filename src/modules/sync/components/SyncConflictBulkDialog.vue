<script setup lang="ts">
import { Cloud, Laptop, ShieldAlert, X } from '@lucide/vue'
import { computed, nextTick, onBeforeUnmount, onMounted, ref } from 'vue'
import { acquireDialogDocumentBoundary } from '@/shared/ui/dialog-document-boundary'
import { trapDialogFocus } from '@/shared/ui/dialog-focus'
import type { SyncConflictChoice } from '../../../shared/api/bindings'

const props = defineProps<{
  entityLabel: string
  conflictCount: number
  choice: SyncConflictChoice
  includesRemoteDeletion: boolean
}>()

const emit = defineEmits<{
  cancel: []
  confirm: []
}>()

const panel = ref<HTMLElement>()
const cancelButton = ref<HTMLButtonElement>()
let releaseDialogBoundary: (() => void) | undefined
const title = computed(() => `确认${props.entityLabel}的批量选择`)
const summary = computed(() => (
  `${props.conflictCount} 处冲突全部采用${props.choice === 'local' ? '本机' : '云端'}版本`
))
const description = computed(() => {
  if (props.choice === 'local') {
    return '云端在这些字段上的改动会被本机版本覆盖。你仍可取消并逐项比较，尚未确认前不会写入任何内容。'
  }
  if (props.includesRemoteDeletion) {
    return '云端记录的是删除状态；确认后，本机这条内容将被删除。尚未同步的本机修改不会保留。'
  }
  return '本机在这些字段上的改动会被云端版本覆盖。你仍可取消并逐项比较，尚未确认前不会写入任何内容。'
})
const confirmLabel = computed(() => {
  if (props.choice === 'local') return '确认全部采用本机版本'
  return props.includesRemoteDeletion
    ? '确认采用云端并删除本机内容'
    : '确认全部采用云端版本'
})

onMounted(async () => {
  if (panel.value) releaseDialogBoundary = acquireDialogDocumentBoundary(panel.value)
  await nextTick()
  cancelButton.value?.focus()
})

onBeforeUnmount(() => {
  releaseDialogBoundary?.()
})

function cancel() {
  emit('cancel')
}

function handleKeydown(event: KeyboardEvent) {
  if (event.key === 'Escape') {
    event.preventDefault()
    cancel()
    return
  }
  trapDialogFocus(event, panel.value)
}
</script>

<template>
  <div
    class="bulk-backdrop"
    @mousedown.self="cancel"
  >
    <section
      ref="panel"
      class="bulk-dialog"
      tabindex="-1"
      role="dialog"
      aria-modal="true"
      aria-labelledby="sync-conflict-bulk-title"
      aria-describedby="sync-conflict-bulk-description"
      @keydown="handleKeydown"
    >
      <button
        class="close-button"
        type="button"
        aria-label="关闭批量选择确认"
        @click="cancel"
      >
        <X :size="18" />
      </button>

      <div
        class="dialog-mark"
        :class="{ remote: choice === 'remote' }"
      >
        <Laptop
          v-if="choice === 'local'"
          :size="25"
        />
        <Cloud
          v-else
          :size="25"
        />
      </div>
      <p class="eyebrow">
        同步冲突 · 最后确认
      </p>
      <h2 id="sync-conflict-bulk-title">
        {{ title }}
      </h2>
      <strong class="summary">{{ summary }}</strong>
      <p
        id="sync-conflict-bulk-description"
        class="description"
      >
        {{ description }}
      </p>

      <aside
        v-if="choice === 'remote' && includesRemoteDeletion"
        class="deletion-warning"
      >
        <ShieldAlert :size="18" />
        <span><strong>这是删除决定</strong><small>确认后，本机只能通过其他备份或另一台尚未同步的设备找回这条内容。</small></span>
      </aside>

      <div class="dialog-actions">
        <button
          ref="cancelButton"
          type="button"
          @click="cancel"
        >
          取消，逐项确认
        </button>
        <button
          class="confirm-button"
          :class="{ destructive: choice === 'remote' && includesRemoteDeletion }"
          type="button"
          @click="emit('confirm')"
        >
          {{ confirmLabel }}
        </button>
      </div>
    </section>
  </div>
</template>

<style scoped>
.bulk-backdrop {
  position: fixed;
  inset: 0;
  z-index: 88;
  display: grid;
  padding: 24px;
  place-items: center;
  background: rgba(24, 34, 30, .58);
  backdrop-filter: blur(8px);
  animation: bulk-backdrop-in var(--motion-standard) var(--ease-standard) both;
}

.bulk-dialog {
  position: relative;
  width: min(560px, 100%);
  max-height: calc(100dvh - 48px);
  padding: 30px;
  overflow: auto;
  border: 1px solid rgba(33, 51, 45, .2);
  border-radius: 22px;
  background: var(--paper-raised);
  box-shadow: 0 30px 84px rgba(22, 32, 28, .3);
  animation: bulk-dialog-in var(--motion-page) var(--ease-standard) both;
}

.close-button {
  position: absolute;
  top: 18px;
  right: 18px;
  display: grid;
  width: 44px;
  height: 44px;
  padding: 0;
  place-items: center;
  border: 0;
  border-radius: 50%;
  background: rgba(33, 51, 45, .06);
  cursor: pointer;
}

.dialog-mark {
  display: grid;
  width: 50px;
  height: 50px;
  margin-bottom: 16px;
  place-items: center;
  color: var(--paper-raised);
  border-radius: 16px 4px 16px 16px;
  background: var(--green-deep);
}

.dialog-mark.remote {
  background: var(--cinnabar);
}

.eyebrow {
  margin: 0 0 7px;
  color: var(--cinnabar);
  font-size: 12px;
  font-weight: 800;
  letter-spacing: .14em;
}

h2 {
  margin: 0;
  color: var(--green-deep);
  font-family: var(--font-serif);
  font-size: 27px;
}

.summary {
  display: block;
  margin-top: 15px;
  color: var(--ink);
  font-size: 14px;
}

.description {
  margin: 8px 0 0;
  color: var(--ink-muted);
  font-size: 13px;
  line-height: 1.75;
}

.deletion-warning {
  display: grid;
  grid-template-columns: auto 1fr;
  gap: 10px;
  align-items: start;
  margin-top: 16px;
  padding: 13px 14px;
  color: #843d2c;
  border: 1px solid rgba(185, 88, 63, .25);
  border-radius: 12px;
  background: rgba(185, 88, 63, .07);
}

.deletion-warning span {
  display: grid;
  gap: 4px;
}

.deletion-warning strong {
  font-size: 13px;
}

.deletion-warning small {
  color: #74594d;
  font-size: 12px;
  line-height: 1.6;
}

.dialog-actions {
  display: flex;
  gap: 9px;
  justify-content: flex-end;
  margin-top: 22px;
}

.dialog-actions button {
  min-height: 44px;
  padding: 0 16px;
  border: 1px solid var(--line);
  border-radius: 11px;
  background: var(--paper-raised);
  cursor: pointer;
}

.dialog-actions .confirm-button {
  color: var(--paper-raised);
  border-color: var(--green-deep);
  background: var(--green-deep);
}

.dialog-actions .confirm-button.destructive {
  border-color: var(--cinnabar);
  background: var(--cinnabar);
}

button:focus-visible {
  outline: 3px solid rgba(185, 88, 63, .3);
  outline-offset: 2px;
}

@keyframes bulk-backdrop-in {
  from { opacity: 0; }
}

@keyframes bulk-dialog-in {
  from { opacity: 0; transform: translateY(10px) scale(.98); }
}

@media (max-width: 560px) {
  .bulk-backdrop { padding: 12px; }
  .bulk-dialog { max-height: calc(100dvh - 24px); padding: 25px 18px 20px; }
  .dialog-actions { display: grid; }
}

@media (prefers-reduced-motion: reduce) {
  .bulk-backdrop,
  .bulk-dialog {
    animation: none;
  }
}
</style>
