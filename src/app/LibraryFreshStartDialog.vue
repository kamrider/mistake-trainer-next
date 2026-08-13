<script setup lang="ts">
import { TriangleAlert, X } from '@lucide/vue'
import { computed, nextTick, onBeforeUnmount, onMounted, ref } from 'vue'
import { acquireDialogDocumentBoundary } from '@/shared/ui/dialog-document-boundary'
import { trapDialogFocus } from '@/shared/ui/dialog-focus'

defineProps<{ busy: boolean; message?: string }>()
const emit = defineEmits<{ cancel: [], confirm: [confirmation: string] }>()
const requiredText = '永久放弃原资料库'
const confirmation = ref('')
const panel = ref<HTMLElement>()
const input = ref<HTMLInputElement>()
const confirmed = computed(() => confirmation.value === requiredText)
let previouslyFocused: HTMLElement | null = null
let releaseBoundary: (() => void) | undefined

onMounted(async () => {
  previouslyFocused = document.activeElement instanceof HTMLElement ? document.activeElement : null
  if (panel.value) releaseBoundary = acquireDialogDocumentBoundary(panel.value)
  await nextTick()
  input.value?.focus()
})

onBeforeUnmount(() => {
  releaseBoundary?.()
  previouslyFocused?.focus()
})

function close() {
  emit('cancel')
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
    class="fresh-backdrop"
    @mousedown.self="!busy && close()"
  >
    <section
      ref="panel"
      class="fresh-dialog"
      role="dialog"
      aria-modal="true"
      aria-labelledby="fresh-title"
      aria-describedby="fresh-description"
      tabindex="-1"
      @keydown="handleKeydown"
    >
      <button
        class="close-button"
        type="button"
        aria-label="关闭重新开始确认"
        :disabled="busy"
        @click="close"
      >
        <X :size="18" />
      </button>
      <TriangleAlert
        :size="34"
        class="warning-mark"
      />
      <h2 id="fresh-title">
        放弃原资料并重新开始？
      </h2>
      <p id="fresh-description">
        这只会清除本机保留的加密身份与恢复控制信息，不会删除任何外置目录。之后应用会建立一个全新的空资料库；原资料若无备份将无法找回。
      </p>
      <p
        v-if="message"
        class="dialog-error"
        role="alert"
      >
        {{ message }}
      </p>
      <label>
        <span>请输入“{{ requiredText }}”以确认</span>
        <input
          ref="input"
          v-model="confirmation"
          :disabled="busy"
          autocomplete="off"
        >
      </label>
      <div class="dialog-actions">
        <button
          type="button"
          :disabled="busy"
          @click="close"
        >
          取消
        </button>
        <button
          type="button"
          class="danger"
          :disabled="busy || !confirmed"
          @click="emit('confirm', confirmation)"
        >
          {{ busy ? '正在安全处理…' : '确认放弃并重新开始' }}
        </button>
      </div>
    </section>
  </div>
</template>

<style scoped>
.fresh-backdrop { position: fixed; inset: 0; z-index: 90; display: grid; padding: 24px; place-items: center; background: rgba(24,34,30,.58); backdrop-filter: blur(7px); }
.fresh-dialog { position: relative; width: min(520px,100%); padding: 30px; border: 1px solid rgba(185,88,63,.35); border-radius: 22px; background: #fffdf7; box-shadow: 0 28px 80px rgba(22,32,28,.3); }
.close-button { position: absolute; top: 16px; right: 16px; display: grid; width: 42px; height: 42px; place-items: center; border: 0; border-radius: 50%; background: rgba(33,51,45,.06); }
.warning-mark { color: var(--cinnabar); }
h2 { margin: 12px 0 0; color: var(--green-deep); font-family: var(--font-serif); }
p { color: var(--ink-muted); line-height: 1.75; }
.dialog-error { padding: 11px 12px; border: 1px solid rgba(185,88,63,.35); border-radius: 10px; color: var(--cinnabar); background: rgba(185,88,63,.08); line-height: 1.55; }
label { display: grid; gap: 8px; margin-top: 18px; color: var(--ink); font-size: 13px; font-weight: 700; }
input { min-height: 44px; padding: 0 12px; border: 1px solid var(--line-strong); border-radius: 10px; background: var(--paper-raised); }
.dialog-actions { display: flex; justify-content: flex-end; gap: 10px; margin-top: 22px; }
.dialog-actions button { min-height: 44px; padding: 0 16px; border: 1px solid var(--line); border-radius: 10px; background: var(--paper-raised); }
.dialog-actions .danger { color: #fff; border-color: var(--cinnabar); background: var(--cinnabar); }
button:disabled { opacity: .45; }
</style>
