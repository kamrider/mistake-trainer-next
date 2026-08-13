<script setup lang="ts">
import { ChevronLeft, ChevronRight, X } from '@lucide/vue'
import { computed, onBeforeUnmount, onMounted, ref, watch } from 'vue'
import { acquireDialogDocumentBoundary } from '@/shared/ui/dialog-document-boundary'
import { trapDialogFocus } from '@/shared/ui/dialog-focus'

const props = defineProps<{
  images: string[]
  initialIndex: number
  label: string
}>()

const emit = defineEmits<{ close: [] }>()
const dialog = ref<HTMLElement>()
const closeButton = ref<HTMLButtonElement>()
const index = ref(clampIndex(props.initialIndex))
let returnFocus: HTMLElement | null = null
let releaseDialogBoundary: (() => void) | undefined

const currentImage = computed(() => props.images[index.value] ?? '')
const hasPrevious = computed(() => index.value > 0)
const hasNext = computed(() => index.value + 1 < props.images.length)

function clampIndex(value: number) {
  if (props.images.length === 0)
    return 0
  return Math.max(0, Math.min(Math.trunc(value), props.images.length - 1))
}

function previous() {
  if (hasPrevious.value)
    index.value -= 1
}

function next() {
  if (hasNext.value)
    index.value += 1
}

function handleKeydown(event: KeyboardEvent) {
  if (event.key === 'Escape') {
    event.preventDefault()
    emit('close')
    return
  }
  if (event.key === 'ArrowLeft') {
    event.preventDefault()
    previous()
    return
  }
  if (event.key === 'ArrowRight') {
    event.preventDefault()
    next()
    return
  }
  trapDialogFocus(event, dialog.value)
}

watch(() => props.initialIndex, value => {
  index.value = clampIndex(value)
})
watch(() => props.images.length, () => {
  index.value = clampIndex(index.value)
})

onMounted(() => {
  returnFocus = document.activeElement instanceof HTMLElement ? document.activeElement : null
  if (dialog.value) releaseDialogBoundary = acquireDialogDocumentBoundary(dialog.value)
  document.addEventListener('keydown', handleKeydown)
  closeButton.value?.focus()
})

onBeforeUnmount(() => {
  document.removeEventListener('keydown', handleKeydown)
  releaseDialogBoundary?.()
  returnFocus?.focus()
})
</script>

<template>
  <Teleport to="body">
    <div
      class="lightbox-backdrop"
      role="presentation"
      @mousedown.self="emit('close')"
    >
      <section
        ref="dialog"
        class="lightbox-dialog"
        role="dialog"
        aria-modal="true"
        :aria-label="`${label}大图`"
        tabindex="-1"
      >
        <header class="lightbox-header">
          <div>
            <p>{{ label }}</p>
            <span aria-live="polite">{{ index + 1 }} / {{ images.length }}</span>
          </div>
          <button
            ref="closeButton"
            class="lightbox-close"
            type="button"
            aria-label="关闭大图"
            @click="emit('close')"
          >
            <X :size="20" />
          </button>
        </header>

        <div class="lightbox-canvas">
          <img
            v-if="currentImage"
            :key="currentImage"
            :src="currentImage"
            :alt="`${label} ${index + 1}`"
          >
        </div>

        <footer class="lightbox-controls">
          <button
            type="button"
            :aria-label="`上一张${label}`"
            :disabled="!hasPrevious"
            @click="previous"
          >
            <ChevronLeft :size="19" />
            上一张
          </button>
          <small>方向键切换 · Esc 关闭</small>
          <button
            type="button"
            :aria-label="`下一张${label}`"
            :disabled="!hasNext"
            @click="next"
          >
            下一张
            <ChevronRight :size="19" />
          </button>
        </footer>
      </section>
    </div>
  </Teleport>
</template>

<style scoped>
.lightbox-backdrop {
  position: fixed;
  z-index: 1000;
  inset: 0;
  display: grid;
  padding: 24px;
  place-items: center;
  background: rgba(25, 34, 31, .82);
  backdrop-filter: blur(8px);
  animation: backdrop-in var(--motion-standard) var(--ease-standard) both;
}

.lightbox-dialog {
  display: grid;
  width: min(1120px, 100%);
  height: min(860px, calc(100vh - 48px));
  grid-template-rows: auto minmax(0, 1fr) auto;
  overflow: hidden;
  border: 1px solid rgba(246, 241, 231, .2);
  border-radius: 6px 24px 24px 24px;
  outline: none;
  background: var(--paper-raised);
  box-shadow: 0 28px 80px rgba(17, 25, 22, .42);
  animation: dialog-in var(--motion-page) var(--ease-standard) both;
}

.lightbox-header,
.lightbox-controls {
  display: flex;
  align-items: center;
  justify-content: space-between;
  padding: 14px 18px;
  border-bottom: 1px solid var(--line);
  background: rgba(246, 241, 231, .86);
}

.lightbox-header p,
.lightbox-header span { margin: 0; }
.lightbox-header p { color: var(--ink); font-weight: 760; }
.lightbox-header span { color: var(--ink-muted); font-family: var(--font-serif); font-size: 12px; }

.lightbox-close {
  display: grid;
  width: 44px;
  height: 44px;
  place-items: center;
  color: var(--ink);
  border: 1px solid var(--line);
  border-radius: 50%;
  background: var(--paper-raised);
  cursor: pointer;
}

.lightbox-canvas {
  display: grid;
  min-height: 0;
  padding: 18px;
  place-items: center;
  background: linear-gradient(135deg, rgba(232, 221, 199, .36), rgba(255, 253, 247, .8));
}

.lightbox-canvas img {
  display: block;
  width: 100%;
  height: 100%;
  object-fit: contain;
  animation: image-in var(--motion-standard) var(--ease-standard) both;
}

.lightbox-controls {
  border-top: 1px solid var(--line);
  border-bottom: 0;
}

.lightbox-controls button {
  display: inline-flex;
  gap: 7px;
  align-items: center;
  min-height: 44px;
  padding: 0 14px;
  color: var(--paper);
  border: 0;
  border-radius: 999px;
  background: var(--green-deep);
  cursor: pointer;
  font-weight: 720;
}

.lightbox-controls button:disabled { cursor: default; opacity: .32; }
.lightbox-controls small { color: var(--ink-muted); }

button:focus-visible { outline: 3px solid rgba(185, 88, 63, .32); outline-offset: 3px; }

@keyframes backdrop-in { from { opacity: 0; } }
@keyframes dialog-in { from { opacity: 0; transform: translateY(12px) scale(.985); } }
@keyframes image-in { from { opacity: 0; transform: scale(.99); } }

@media (max-width: 620px) {
  .lightbox-backdrop { padding: 8px; }
  .lightbox-dialog { height: calc(100vh - 16px); border-radius: 4px 16px 16px; }
  .lightbox-controls small { display: none; }
}

@media (prefers-reduced-motion: reduce) {
  .lightbox-backdrop,
  .lightbox-dialog,
  .lightbox-canvas img { animation: none; }
}
</style>
