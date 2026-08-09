<script setup lang="ts">
import { ShieldAlert } from '@lucide/vue'
import { nextTick, onBeforeUnmount, onMounted, ref, useId } from 'vue'
import type { ActionConfirmationRequest } from '../composables/useActionConfirmation'
import { acquireDialogDocumentBoundary } from '../dialog-document-boundary'
import { trapDialogFocus } from '../dialog-focus'

defineProps<{
  request: ActionConfirmationRequest
}>()

const emit = defineEmits<{
  cancel: []
  confirm: []
}>()

const panel = ref<HTMLElement>()
const cancelButton = ref<HTMLButtonElement>()
const instanceId = useId()
const titleId = `action-confirm-title-${instanceId}`
const descriptionId = `action-confirm-description-${instanceId}`
let previouslyFocused: HTMLElement | null = null
let releaseDialogBoundary: (() => void) | undefined

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

onMounted(async () => {
  previouslyFocused = document.activeElement as HTMLElement | null
  if (panel.value) releaseDialogBoundary = acquireDialogDocumentBoundary(panel.value)
  await nextTick()
  cancelButton.value?.focus()
})

onBeforeUnmount(() => {
  releaseDialogBoundary?.()
  previouslyFocused?.focus()
})
</script>

<template>
  <Teleport to="body">
    <div
      class="action-confirm-backdrop"
      @mousedown.self="cancel"
    >
      <section
        ref="panel"
        class="action-confirm-dialog"
        :class="request.tone ?? 'warning'"
        role="alertdialog"
        aria-modal="true"
        :aria-labelledby="titleId"
        :aria-describedby="descriptionId"
        tabindex="-1"
        @keydown.stop="handleKeydown"
      >
        <div class="dialog-mark">
          <ShieldAlert
            :size="25"
            aria-hidden="true"
          />
        </div>
        <p class="eyebrow">
          {{ request.eyebrow ?? '操作确认' }}
        </p>
        <h2 :id="titleId">
          {{ request.title }}
        </h2>
        <p
          :id="descriptionId"
          class="description"
        >
          {{ request.description }}
        </p>
        <div class="dialog-actions">
          <button
            ref="cancelButton"
            type="button"
            @click="cancel"
          >
            {{ request.cancelLabel ?? '取消，保持现状' }}
          </button>
          <button
            type="button"
            class="confirm-button"
            :class="{ danger: request.tone === 'danger' }"
            @click="emit('confirm')"
          >
            {{ request.confirmLabel }}
          </button>
        </div>
      </section>
    </div>
  </Teleport>
</template>

<style scoped>
.action-confirm-backdrop {
  position: fixed;
  z-index: 110;
  inset: 0;
  display: grid;
  padding: 24px;
  place-items: center;
  background: rgba(24, 34, 30, .58);
  backdrop-filter: blur(8px);
  animation: action-confirm-backdrop-in var(--motion-standard) var(--ease-standard) both;
}

.action-confirm-dialog {
  width: min(520px, 100%);
  max-height: calc(100dvh - 48px);
  box-sizing: border-box;
  padding: 30px;
  overflow: auto;
  border: 1px solid rgba(185, 88, 63, .28);
  border-radius: 22px;
  outline: none;
  background: var(--paper-raised);
  box-shadow: 0 30px 84px rgba(22, 32, 28, .3);
  animation: action-confirm-dialog-in var(--motion-page) var(--ease-standard) both;
}

.dialog-mark {
  display: grid;
  width: 50px;
  height: 50px;
  margin-bottom: 16px;
  place-items: center;
  color: var(--paper-raised);
  border-radius: 16px;
  background: var(--cinnabar);
}

.warning .dialog-mark {
  background: var(--green-deep);
}

.eyebrow {
  margin: 0 0 7px;
  color: var(--cinnabar);
  font-size: 12px;
  font-weight: 800;
  letter-spacing: .13em;
}

h2 {
  margin: 0;
  color: var(--green-deep);
  font-family: var(--font-serif);
  font-size: clamp(24px, 5vw, 29px);
}

.description {
  margin: 12px 0 0;
  color: var(--ink-muted);
  line-height: 1.75;
}

.dialog-actions {
  display: flex;
  justify-content: flex-end;
  gap: 9px;
  margin-top: 22px;
}

.dialog-actions button {
  min-height: 44px;
  padding: 0 16px;
  color: var(--green-deep);
  border: 1px solid var(--line);
  border-radius: 11px;
  background: var(--paper-raised);
  font-weight: 750;
  cursor: pointer;
}

.dialog-actions .confirm-button {
  color: var(--paper-raised);
  border-color: var(--green-deep);
  background: var(--green-deep);
}

.dialog-actions .confirm-button.danger {
  border-color: var(--cinnabar);
  background: var(--cinnabar);
}

.dialog-actions button:focus-visible {
  outline: 3px solid rgba(185, 88, 63, .3);
  outline-offset: 2px;
}

@keyframes action-confirm-backdrop-in {
  from { opacity: 0; }
}

@keyframes action-confirm-dialog-in {
  from { opacity: 0; transform: translateY(12px) scale(.975); }
}

@media (max-width: 560px) {
  .action-confirm-backdrop { padding: 12px; }
  .action-confirm-dialog { max-height: calc(100dvh - 24px); padding: 24px 18px; }
  .dialog-actions { flex-direction: column-reverse; }
  .dialog-actions button { width: 100%; }
}

@media (prefers-reduced-motion: reduce) {
  .action-confirm-backdrop,
  .action-confirm-dialog { animation: none; }
}
</style>
