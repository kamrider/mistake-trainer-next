<script setup lang="ts">
import { CloudOff, DatabaseZap, LockKeyhole, RadioTower, RefreshCw, ShieldCheck, X } from '@lucide/vue'
import { computed, nextTick, onBeforeUnmount, onMounted, ref, watch } from 'vue'
import { acquireDialogDocumentBoundary } from './dialog-document-boundary'
import { trapDialogFocus } from './dialog-focus'

const props = defineProps<{
  mode: 'lock' | 'sign-out'
  busy: boolean
  errorMessage?: string
}>()

const emit = defineEmits<{
  cancel: []
  confirm: []
}>()

const panel = ref<HTMLElement>()
const cancelButton = ref<HTMLButtonElement>()
let releaseDialogBoundary: (() => void) | undefined
const copy = computed(() => props.mode === 'sign-out'
  ? {
      eyebrow: '云端账户 · 本机保护',
      title: '退出云端并锁定本机？',
      description: '应用只会退出这台电脑的云端会话，再关闭本地资料库并安全重启；其他设备保持登录。断网也不会阻止本机清除登录凭据。',
      action: '退出并锁定',
    }
  : {
      eyebrow: '本地资料库 · 立即保护',
      title: '现在锁定本地资料库？',
      description: '应用会停止正在进行的手机采集，关闭本地数据库并安全重启。题目、图片和训练记录都不会被删除。',
      action: '立即锁定',
    },
)

onMounted(async () => {
  if (panel.value) releaseDialogBoundary = acquireDialogDocumentBoundary(panel.value)
  await nextTick()
  cancelButton.value?.focus()
})

onBeforeUnmount(() => {
  releaseDialogBoundary?.()
})

watch(() => props.busy, async (busy) => {
  if (!busy) return
  await nextTick()
  panel.value?.focus()
})

function close() {
  if (!props.busy) emit('cancel')
}

function handleKeydown(event: KeyboardEvent) {
  if (event.key === 'Escape') {
    event.preventDefault()
    close()
    return
  }
  trapDialogFocus(event, panel.value)
}
</script>

<template>
  <div
    class="lock-backdrop"
    @mousedown.self="close"
  >
    <section
      ref="panel"
      class="lock-dialog"
      tabindex="-1"
      role="dialog"
      aria-modal="true"
      aria-labelledby="library-lock-title"
      aria-describedby="library-lock-description"
      @keydown="handleKeydown"
    >
      <button
        class="close-button"
        type="button"
        aria-label="关闭锁定确认"
        :disabled="busy"
        @click="close"
      >
        <X :size="18" />
      </button>

      <div class="lock-mark">
        <CloudOff
          v-if="mode === 'sign-out'"
          :size="27"
        />
        <LockKeyhole
          v-else
          :size="27"
        />
      </div>
      <p class="eyebrow">
        {{ copy.eyebrow }}
      </p>
      <h2 id="library-lock-title">
        {{ copy.title }}
      </h2>
      <p
        id="library-lock-description"
        class="description"
      >
        {{ copy.description }}
      </p>

      <ol class="lock-steps">
        <li>
          <RadioTower :size="18" />
          <span><strong>结束临时入口</strong><small>手机扫码会话立即停止</small></span>
        </li>
        <li>
          <DatabaseZap :size="18" />
          <span><strong>卸载解密密钥</strong><small>新进程不会打开 SQLCipher</small></span>
        </li>
        <li>
          <ShieldCheck :size="18" />
          <span><strong>资料保持原样</strong><small>不移动、不删除任何题目图片</small></span>
        </li>
      </ol>

      <p
        v-if="errorMessage"
        class="lock-error"
        role="alert"
      >
        {{ errorMessage }}
      </p>

      <div class="dialog-actions">
        <button
          ref="cancelButton"
          type="button"
          :disabled="busy"
          @click="close"
        >
          取消，继续使用
        </button>
        <button
          class="confirm-button"
          type="button"
          :disabled="busy"
          @click="emit('confirm')"
        >
          <RefreshCw
            v-if="busy"
            :size="16"
            class="spinning"
          />
          <LockKeyhole
            v-else
            :size="16"
          />
          {{ busy ? '正在锁定…' : copy.action }}
        </button>
      </div>
    </section>
  </div>
</template>

<style scoped>
.lock-backdrop {
  position: fixed;
  inset: 0;
  z-index: 84;
  display: grid;
  padding: 24px;
  place-items: center;
  background: rgba(24, 34, 30, .56);
  backdrop-filter: blur(8px);
  animation: lock-backdrop-in var(--motion-standard) var(--ease-standard) both;
}

.lock-dialog {
  position: relative;
  width: min(590px, 100%);
  max-height: calc(100vh - 48px);
  padding: 30px;
  overflow: auto;
  border: 1px solid rgba(185, 88, 63, .3);
  border-radius: 22px;
  background: var(--paper-raised);
  box-shadow: 0 30px 84px rgba(22, 32, 28, .3);
  animation: lock-dialog-in var(--motion-page) var(--ease-standard) both;
}

.lock-dialog::before {
  position: absolute;
  inset: 9px;
  border: 1px solid rgba(185, 88, 63, .08);
  border-radius: 15px;
  content: "";
  pointer-events: none;
}

.close-button {
  position: absolute;
  z-index: 1;
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

.lock-mark {
  display: grid;
  width: 52px;
  height: 52px;
  margin-bottom: 17px;
  place-items: center;
  color: var(--paper-raised);
  border-radius: 16px;
  background: var(--cinnabar);
  box-shadow: 0 10px 24px rgba(185, 88, 63, .2);
}

.eyebrow {
  margin: 0 0 7px;
  color: var(--cinnabar);
  font-size: 12px;
  font-weight: 800;
  letter-spacing: .15em;
}

h2 {
  margin: 0;
  color: var(--green-deep);
  font-family: var(--font-serif);
  font-size: 28px;
}

.description {
  margin: 12px 0 0;
  color: var(--ink-muted);
  line-height: 1.75;
}

.lock-steps {
  display: grid;
  grid-template-columns: repeat(3, 1fr);
  gap: 9px;
  margin: 20px 0 0;
  padding: 0;
  list-style: none;
}

.lock-steps li {
  display: flex;
  gap: 9px;
  min-width: 0;
  padding: 14px;
  color: var(--cinnabar);
  border-radius: 13px;
  background: rgba(232, 221, 199, .32);
}

.lock-steps span { display: grid; gap: 5px; min-width: 0; }
.lock-steps strong { color: var(--green-deep); font-size: 12px; }
.lock-steps small { color: var(--ink-muted); font-size: 12px; line-height: 1.5; }

.lock-error {
  margin: 14px 0 0;
  padding: 11px 13px;
  color: #843d2c;
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
  padding: 0 16px;
  border: 1px solid var(--line);
  border-radius: 11px;
  background: var(--paper-raised);
  cursor: pointer;
  transition: transform var(--motion-feedback) var(--ease-standard);
}

.dialog-actions button:hover:not(:disabled) { transform: translateY(-1px); }
.dialog-actions .confirm-button {
  color: var(--paper-raised);
  border-color: var(--cinnabar);
  background: var(--cinnabar);
  box-shadow: 0 8px 22px rgba(185, 88, 63, .16);
}

.dialog-actions button:disabled { opacity: .5; cursor: wait; }
.spinning { animation: lock-spin var(--motion-standard) var(--ease-standard) both; }

@keyframes lock-backdrop-in { from { opacity: 0; } }
@keyframes lock-dialog-in { from { opacity: 0; transform: translateY(12px) scale(.975); } }
@keyframes lock-spin { to { transform: rotate(360deg); } }

@media (max-width: 620px) {
  .lock-backdrop { padding: 12px; }
  .lock-dialog { max-height: calc(100vh - 24px); padding: 25px 18px 20px; }
  .lock-steps { grid-template-columns: 1fr; }
  .dialog-actions { flex-direction: column-reverse; }
  .dialog-actions button { justify-content: center; }
}

@media (prefers-reduced-motion: reduce) {
  .lock-backdrop,
  .lock-dialog,
  .spinning { animation: none; }
  .dialog-actions button { transition: none; }
}
</style>
