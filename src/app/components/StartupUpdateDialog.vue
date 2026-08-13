<script setup lang="ts">
import { ArchiveRestore, ShieldCheck, X } from '@lucide/vue'
import { nextTick, onBeforeUnmount, onMounted, ref, useId, watch } from 'vue'
import type { WindowsUpdateCheckReport } from '../../shared/api/bindings'
import { acquireDialogDocumentBoundary } from '@/shared/ui/dialog-document-boundary'
import { trapDialogFocus } from '@/shared/ui/dialog-focus'

const props = defineProps<{
  report: WindowsUpdateCheckReport
  installing: boolean
  message: string
  publicationLabel: string
}>()

const emit = defineEmits<{
  dismiss: []
  install: []
}>()

const panel = ref<HTMLElement>()
const laterButton = ref<HTMLButtonElement>()
const instanceId = useId()
const titleId = `startup-update-title-${instanceId}`
const descriptionId = `startup-update-description-${instanceId}`
let previouslyFocused: HTMLElement | null = null
let releaseDialogBoundary: (() => void) | undefined

function dismiss() {
  if (!props.installing) emit('dismiss')
}

function handleKeydown(event: KeyboardEvent) {
  if (event.key === 'Escape') {
    event.preventDefault()
    dismiss()
    return
  }
  trapDialogFocus(event, panel.value)
}

onMounted(async () => {
  previouslyFocused = document.activeElement as HTMLElement | null
  if (panel.value) releaseDialogBoundary = acquireDialogDocumentBoundary(panel.value)
  await nextTick()
  laterButton.value?.focus()
})

onBeforeUnmount(() => {
  releaseDialogBoundary?.()
  previouslyFocused?.focus()
})

watch(() => props.installing, async (installing) => {
  if (!installing) return
  await nextTick()
  panel.value?.focus()
})
</script>

<template>
  <Teleport to="body">
    <div
      class="startup-update-backdrop"
      @mousedown.self="dismiss"
    >
      <section
        ref="panel"
        class="startup-update-dialog"
        role="dialog"
        aria-modal="true"
        :aria-labelledby="titleId"
        :aria-describedby="descriptionId"
        tabindex="-1"
        @keydown.stop="handleKeydown"
      >
        <button
          class="close-button"
          type="button"
          aria-label="稍后更新"
          :disabled="installing"
          @click="dismiss"
        >
          <X :size="18" />
        </button>

        <div class="dialog-mark">
          <ShieldCheck
            :size="26"
            aria-hidden="true"
          />
        </div>
        <p class="eyebrow">
          签名更新 · 由你确认
        </p>
        <h2 :id="titleId">
          新版本 {{ report.version }} 已准备好
        </h2>
        <p
          :id="descriptionId"
          class="description"
        >
          当前版本 {{ report.currentVersion }}。应用只会安装通过签名验证的 Windows 更新；下载完成后安装程序会关闭应用。
        </p>

        <div class="update-details">
          <ArchiveRestore :size="20" />
          <span>
            <strong>可选更新 {{ report.version }}</strong>
            <small v-if="publicationLabel">发布时间 {{ publicationLabel }}</small>
            <small v-else>你可以现在安装，也可以稍后在设置中手动检查。</small>
          </span>
        </div>

        <p
          v-if="message"
          class="update-message"
          role="status"
          aria-live="polite"
        >
          {{ message }}
        </p>

        <div class="dialog-actions">
          <button
            ref="laterButton"
            type="button"
            :disabled="installing"
            @click="dismiss"
          >
            稍后
          </button>
          <button
            class="install-button"
            type="button"
            :disabled="installing"
            @click="emit('install')"
          >
            <ArchiveRestore :size="17" />
            {{ installing ? '正在下载并验证…' : `立即更新至 ${report.version}` }}
          </button>
        </div>
      </section>
    </div>
  </Teleport>
</template>

<style scoped>
.startup-update-backdrop {
  position: fixed;
  z-index: 120;
  inset: 0;
  display: grid;
  padding: 24px;
  place-items: center;
  background: rgba(24, 34, 30, .6);
  backdrop-filter: blur(8px);
  animation: startup-update-backdrop-in var(--motion-standard) var(--ease-standard) both;
}

.startup-update-dialog {
  position: relative;
  width: min(560px, 100%);
  max-height: calc(100dvh - 48px);
  box-sizing: border-box;
  padding: 30px;
  overflow: auto;
  color: var(--ink);
  border: 1px solid rgba(33, 51, 45, .22);
  border-radius: 22px;
  outline: none;
  background: var(--paper-raised);
  box-shadow: 0 30px 84px rgba(22, 32, 28, .32);
  animation: startup-update-dialog-in var(--motion-page) var(--ease-standard) both;
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
  color: var(--green-deep);
  border: 0;
  border-radius: 50%;
  background: rgba(33, 51, 45, .07);
  cursor: pointer;
}

.dialog-mark {
  display: grid;
  width: 52px;
  height: 52px;
  margin-bottom: 17px;
  place-items: center;
  color: var(--paper-raised);
  border-radius: 17px 7px 17px 17px;
  background: var(--green-deep);
}

.eyebrow {
  margin: 0 0 7px;
  color: var(--cinnabar);
  font-size: 12px;
  font-weight: 850;
  letter-spacing: .14em;
}

h2 {
  margin: 0;
  padding-right: 44px;
  color: var(--green-deep);
  font-family: var(--font-serif);
  font-size: clamp(25px, 5vw, 30px);
}

.description {
  margin: 12px 0 0;
  color: var(--ink-muted);
  line-height: 1.75;
}

.update-details {
  display: flex;
  gap: 12px;
  align-items: flex-start;
  margin-top: 20px;
  padding: 15px;
  color: #557263;
  border: 1px solid rgba(85, 114, 99, .22);
  border-radius: 13px;
  background: rgba(85, 114, 99, .08);
}

.update-details > svg { flex: 0 0 auto; margin-top: 1px; }
.update-details strong, .update-details small { display: block; }
.update-details strong { font-size: 13px; }
.update-details small { margin-top: 4px; color: var(--ink-muted); line-height: 1.5; }

.update-message {
  margin: 12px 0 0;
  padding: 11px 13px;
  color: #713d30;
  border: 1px solid rgba(185, 88, 63, .24);
  border-radius: 11px;
  background: rgba(185, 88, 63, .07);
  font-size: 12px;
  line-height: 1.6;
}

.dialog-actions {
  display: flex;
  justify-content: flex-end;
  gap: 9px;
  margin-top: 22px;
}

.dialog-actions button {
  display: inline-flex;
  min-height: 44px;
  gap: 7px;
  align-items: center;
  justify-content: center;
  padding: 0 16px;
  color: var(--green-deep);
  border: 1px solid var(--line);
  border-radius: 11px;
  background: var(--paper-raised);
  font-weight: 750;
  cursor: pointer;
}

.dialog-actions .install-button {
  color: var(--paper-raised);
  border-color: var(--green-deep);
  background: var(--green-deep);
}

button:disabled { opacity: .52; cursor: default; }
button:focus-visible { outline: 3px solid rgba(185, 88, 63, .3); outline-offset: 2px; }

@keyframes startup-update-backdrop-in { from { opacity: 0; } }
@keyframes startup-update-dialog-in { from { opacity: 0; transform: translateY(12px) scale(.975); } }

@media (max-width: 580px) {
  .startup-update-backdrop { padding: 12px; }
  .startup-update-dialog { max-height: calc(100dvh - 24px); padding: 24px 18px; }
  .dialog-actions { flex-direction: column-reverse; }
  .dialog-actions button { width: 100%; }
}

@media (prefers-reduced-motion: reduce) {
  .startup-update-backdrop,
  .startup-update-dialog { animation: none; }
}
</style>
