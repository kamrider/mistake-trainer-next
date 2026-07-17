<script setup lang="ts">
import { GripVertical, Image as ImageIcon, Trash2 } from '@lucide/vue'
import { onBeforeUnmount, onMounted, ref } from 'vue'
import type { CaptureItemSummary } from '../../../shared/api/bindings'

const props = defineProps<{
  item: CaptureItemSummary
  dataUrl: string | undefined
  disabled?: boolean
  removable?: boolean
  variant?: 'compact' | 'gallery' | 'filmstrip'
  active?: boolean
}>()

const emit = defineEmits<{
  preview: [itemId: string]
  remove: [itemId: string]
  activate: [itemId: string]
  pointerStart: [itemId: string, event: PointerEvent]
}>()

const root = ref<HTMLElement>()
let observer: IntersectionObserver | undefined

onMounted(() => {
  if (!root.value) return
  observer = new IntersectionObserver((entries) => {
    if (!entries.some(entry => entry.isIntersecting)) return
    emit('preview', props.item.id)
    observer?.disconnect()
  }, { rootMargin: '180px' })
  observer.observe(root.value)
})

onBeforeUnmount(() => observer?.disconnect())
</script>

<template>
  <article
    ref="root"
    class="capture-thumbnail"
    :class="[`is-${variant ?? 'compact'}`, { 'is-disabled': disabled, 'is-active': active }]"
    :aria-label="item.sourceName"
    tabindex="0"
    @pointerdown="!disabled && emit('pointerStart', item.id, $event)"
    @click="emit('activate', item.id)"
    @keydown.enter.prevent="emit('activate', item.id)"
    @keydown.space.prevent="emit('activate', item.id)"
  >
    <div class="thumb-media">
      <img
        v-if="dataUrl"
        :src="dataUrl"
        :alt="variant === 'filmstrip' ? '' : item.sourceName"
        draggable="false"
      >
      <ImageIcon
        v-else
        :size="25"
        aria-hidden="true"
      />
      <span
        class="drag-mark"
        aria-hidden="true"
      >
        <GripVertical :size="15" />
      </span>
    </div>
    <div class="thumb-copy">
      <strong :title="item.sourceName">{{ item.sourceName }}</strong>
      <small>{{ item.width }} × {{ item.height }}</small>
    </div>
    <button
      v-if="removable"
      type="button"
      class="remove-button"
      :aria-label="`删除 ${item.sourceName}`"
      :disabled="disabled"
      @click.stop="emit('remove', item.id)"
    >
      <Trash2
        :size="14"
        aria-hidden="true"
      />
    </button>
  </article>
</template>

<style scoped>
.capture-thumbnail { position: relative; display: grid; grid-template-columns: 74px minmax(0,1fr) auto; gap: 10px; align-items: center; min-width: 0; padding: 7px; border: 1px solid rgba(33,51,45,.12); border-radius: 12px; background: rgba(255,253,247,.82); cursor: grab; user-select:none; -webkit-user-drag:none; transition: transform var(--motion-feedback) var(--ease-standard), border-color var(--motion-feedback), box-shadow var(--motion-feedback); }
.capture-thumbnail:hover, .capture-thumbnail:focus-visible { border-color: rgba(33,51,45,.3); box-shadow: 0 8px 22px rgba(34,48,43,.08); outline: none; transform: translateY(-1px); }
.capture-thumbnail:active { cursor: grabbing; }
.capture-thumbnail.is-disabled { cursor: default; opacity: .72; }
.thumb-media { position: relative; display: grid; height: 60px; overflow: hidden; place-items: center; color: #7a8b84; border-radius: 8px; background: var(--sand-paper); }
.thumb-media img { width: 100%; height: 100%; object-fit: cover; }
.drag-mark { position: absolute; right: 3px; bottom: 3px; display: grid; width: 22px; height: 22px; place-items: center; color: var(--paper); border-radius: 6px; background: rgba(33,51,45,.72); }
.thumb-copy { display: grid; min-width: 0; gap: 4px; }
.thumb-copy strong { overflow: hidden; font-size: 12px; text-overflow: ellipsis; white-space: nowrap; }
.thumb-copy small { color: var(--ink-muted); font-size: 10px; }
.remove-button { display: grid; width: 29px; height: 29px; place-items: center; color: var(--ink-muted); border: 0; border-radius: 50%; background: transparent; cursor: pointer; }
.remove-button:hover { color: var(--cinnabar); background: rgba(185,88,63,.1); }
.capture-thumbnail.is-gallery { display:block; padding:8px; }.is-gallery .thumb-media { height:180px; }.is-gallery .thumb-media img { object-fit:contain; }.is-gallery .thumb-copy { margin-top:8px; }.is-gallery .remove-button { position:absolute; top:12px; right:12px; color:var(--paper); background:rgba(33,51,45,.72); }
.capture-thumbnail.is-filmstrip { flex:0 0 88px; display:block; padding:4px; border-radius:10px; }.is-filmstrip .thumb-media { height:66px; }.is-filmstrip .thumb-media img { object-fit:cover; }.is-filmstrip .thumb-copy { display:none; }.is-filmstrip .drag-mark { display:none; }.capture-thumbnail.is-filmstrip.is-active { border-color:var(--cinnabar); box-shadow:0 0 0 2px rgba(185,88,63,.18); transform:translateY(-1px); }
@media (prefers-reduced-motion: reduce) { .capture-thumbnail { transition: none; } }
</style>
