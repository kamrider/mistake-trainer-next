<script setup lang="ts">
import { ArrowDown, ArrowUp, Crop, Plus, Redo2, RotateCw, Trash2, Undo2, X, ZoomIn, ZoomOut } from '@lucide/vue'
import { computed, nextTick, onBeforeUnmount, onMounted, ref, watch } from 'vue'
import type { CaptureCropRecipe } from '../../../shared/api/bindings'
import {
  fitImageWithin,
  moveCropRegion,
  resizeCropRegion,
  type CropRegion,
  type CropResizeHandle,
} from '../domain/cropGeometry'

type Region = CropRegion & { id: string }
type Snapshot = { rotation: 0 | 90 | 180 | 270, regions: Region[], activeId: string }
type DragState = {
  id: string
  mode: 'move' | 'resize'
  handle: CropResizeHandle | undefined
  startX: number
  startY: number
  before: Region
}

const resizeHandles: { handle: CropResizeHandle, label: string }[] = [
  { handle: 'n', label: '调整上边界' },
  { handle: 'ne', label: '调整右上角' },
  { handle: 'e', label: '调整右边界' },
  { handle: 'se', label: '调整右下角' },
  { handle: 's', label: '调整下边界' },
  { handle: 'sw', label: '调整左下角' },
  { handle: 'w', label: '调整左边界' },
  { handle: 'nw', label: '调整左上角' },
]

const props = defineProps<{
  dataUrl: string
  itemName: string
  busy: boolean
}>()

const emit = defineEmits<{
  close: []
  apply: [recipes: CaptureCropRecipe[]]
}>()

const dialog = ref<HTMLElement>()
const closeButton = ref<HTMLButtonElement>()
const stage = ref<HTMLElement>()
const renderedDataUrl = ref(props.dataUrl)
const rotation = ref<0 | 90 | 180 | 270>(0)
const zoom = ref(1)
const naturalSize = ref({ width: 1200, height: 900 })
const viewportSize = ref({ width: 1000, height: 700 })
const isPanning = ref(false)
const regions = ref<Region[]>([makeRegion(0)])
const activeId = ref(regions.value[0]!.id)
const undoStack = ref<Snapshot[]>([])
const redoStack = ref<Snapshot[]>([])
let drag: DragState | undefined
let pan: { startX: number, startY: number, scrollLeft: number, scrollTop: number } | undefined
let spacePressed = false
let resizeObserver: ResizeObserver | undefined
let zoomScrollTimer: ReturnType<typeof setTimeout> | undefined
let rotationRenderVersion = 0
let previousBodyOverflow = ''

const baseImageSize = computed(() => fitImageWithin(
  naturalSize.value.width,
  naturalSize.value.height,
  Math.max(1, viewportSize.value.width - 56),
  Math.max(1, viewportSize.value.height - 56),
  1200,
))

const displayImageSize = computed(() => ({
  width: Math.max(1, Math.round(baseImageSize.value.width * zoom.value)),
  height: Math.max(1, Math.round(baseImageSize.value.height * zoom.value)),
}))

const imageShellStyle = computed(() => ({
  width: `${displayImageSize.value.width}px`,
  height: `${displayImageSize.value.height}px`,
}))

const stageSurfaceStyle = computed(() => ({
  width: `${Math.max(viewportSize.value.width, displayImageSize.value.width + 56)}px`,
  height: `${Math.max(viewportSize.value.height, displayImageSize.value.height + 56)}px`,
}))

function makeRegion(index: number): Region {
  const inset = Math.min(0.06 + index * 0.018, 0.2)
  return { id: crypto.randomUUID(), x: inset, y: inset, width: 1 - inset * 2, height: 1 - inset * 2 }
}

function snapshot(): Snapshot {
  return {
    rotation: rotation.value,
    regions: regions.value.map(region => ({ ...region })),
    activeId: activeId.value,
  }
}

function restore(value: Snapshot) {
  rotation.value = value.rotation
  regions.value = value.regions.map(region => ({ ...region }))
  activeId.value = regions.value.some(region => region.id === value.activeId)
    ? value.activeId
    : regions.value[0]?.id ?? ''
}

function checkpoint() {
  undoStack.value.push(snapshot())
  if (undoStack.value.length > 40) undoStack.value.shift()
  redoStack.value = []
}

function undo() {
  const previous = undoStack.value.pop()
  if (!previous) return
  redoStack.value.push(snapshot())
  restore(previous)
}

function redo() {
  const next = redoStack.value.pop()
  if (!next) return
  undoStack.value.push(snapshot())
  restore(next)
}

function addRegion() {
  if (regions.value.length >= 10) return
  checkpoint()
  const region = makeRegion(regions.value.length)
  regions.value.push(region)
  activeId.value = region.id
}

function removeRegion(id: string) {
  if (regions.value.length <= 1) return
  checkpoint()
  const index = regions.value.findIndex(region => region.id === id)
  regions.value = regions.value.filter(region => region.id !== id)
  activeId.value = regions.value[Math.min(Math.max(index - 1, 0), regions.value.length - 1)]!.id
}

function reorderRegion(id: string, offset: -1 | 1) {
  const index = regions.value.findIndex(region => region.id === id)
  const target = index + offset
  if (index < 0 || target < 0 || target >= regions.value.length) return
  checkpoint()
  const next = [...regions.value]
  const [region] = next.splice(index, 1)
  next.splice(target, 0, region!)
  regions.value = next
  activeId.value = id
}

function rotate() {
  checkpoint()
  rotation.value = ((rotation.value + 90) % 360) as 0 | 90 | 180 | 270
}

function reset() {
  checkpoint()
  rotation.value = 0
  zoom.value = 1
  regions.value = [makeRegion(0)]
  activeId.value = regions.value[0]!.id
}

function beginPointer(
  region: Region,
  mode: 'move' | 'resize',
  event: PointerEvent,
  handle?: CropResizeHandle,
) {
  if (props.busy) return
  event.preventDefault()
  event.stopPropagation()
  activeId.value = region.id
  checkpoint()
  drag = { id: region.id, mode, handle, startX: event.clientX, startY: event.clientY, before: { ...region } }
  window.addEventListener('pointermove', movePointer)
  window.addEventListener('pointerup', endPointer, { once: true })
  window.addEventListener('pointercancel', endPointer, { once: true })
}

function movePointer(event: PointerEvent) {
  if (!drag) return
  const overlay = dialog.value?.querySelector<HTMLElement>('.crop-overlay')
  if (!overlay) return
  const width = overlay.clientWidth
  const height = overlay.clientHeight
  if (!width || !height) return
  const dx = (event.clientX - drag.startX) / width
  const dy = (event.clientY - drag.startY) / height
  const region = regions.value.find(value => value.id === drag!.id)
  if (!region) return
  const next = drag.mode === 'move'
    ? moveCropRegion(drag.before, dx, dy)
    : resizeCropRegion(drag.before, drag.handle ?? 'se', dx, dy)
  Object.assign(region, next)
}

function endPointer() {
  drag = undefined
  window.removeEventListener('pointermove', movePointer)
  window.removeEventListener('pointerup', endPointer)
  window.removeEventListener('pointercancel', endPointer)
}

function nudge(event: KeyboardEvent, region: Region) {
  if (!['ArrowLeft', 'ArrowRight', 'ArrowUp', 'ArrowDown'].includes(event.key)) return
  event.preventDefault()
  checkpoint()
  const delta = event.shiftKey ? 0.02 : 0.005
  const direction = event.key === 'ArrowLeft' || event.key === 'ArrowUp' ? -delta : delta
  let next: Region
  if (event.altKey) {
    if (event.key === 'ArrowLeft' || event.key === 'ArrowRight') {
      next = resizeCropRegion(region, 'e', direction, 0)
    }
    else next = resizeCropRegion(region, 's', 0, direction)
  }
  else if (event.key === 'ArrowLeft' || event.key === 'ArrowRight') next = moveCropRegion(region, direction, 0)
  else next = moveCropRegion(region, 0, direction)
  Object.assign(region, next)
}

function clamp(value: number, min: number, max: number) {
  return Math.min(max, Math.max(min, value))
}

function readImageSize(event: Event) {
  const image = event.currentTarget as HTMLImageElement
  naturalSize.value = {
    width: image.naturalWidth || 1200,
    height: image.naturalHeight || 900,
  }
}

function updateViewport() {
  if (!stage.value) return
  viewportSize.value = {
    width: stage.value.clientWidth || 1000,
    height: stage.value.clientHeight || 700,
  }
}

async function setZoom(nextZoom: number, anchor?: { clientX: number, clientY: number }) {
  const viewport = stage.value
  const previous = displayImageSize.value
  const previousZoom = zoom.value
  const next = clamp(Math.round(nextZoom * 100) / 100, 0.75, 2.5)
  if (next === previousZoom) return

  const bounds = viewport?.getBoundingClientRect()
  const anchorX = viewport && bounds
    ? (anchor?.clientX ?? bounds.left + viewport.clientWidth / 2) - bounds.left + viewport.scrollLeft
    : 0
  const anchorY = viewport && bounds
    ? (anchor?.clientY ?? bounds.top + viewport.clientHeight / 2) - bounds.top + viewport.scrollTop
    : 0
  const ratioX = previous.width ? anchorX / Math.max(previous.width + 56, viewport?.scrollWidth ?? 1) : 0.5
  const ratioY = previous.height ? anchorY / Math.max(previous.height + 56, viewport?.scrollHeight ?? 1) : 0.5

  zoom.value = next
  await nextTick()
  const restoreAnchor = () => {
    if (!viewport || stage.value !== viewport) return
    const targetX = anchor && bounds ? anchor.clientX - bounds.left : viewport.clientWidth / 2
    const targetY = anchor && bounds ? anchor.clientY - bounds.top : viewport.clientHeight / 2
    viewport.scrollLeft = ratioX * viewport.scrollWidth - targetX
    viewport.scrollTop = ratioY * viewport.scrollHeight - targetY
  }
  restoreAnchor()
  clearTimeout(zoomScrollTimer)
  zoomScrollTimer = setTimeout(restoreAnchor, 220)
}

function handleWheel(event: WheelEvent) {
  if (!(event.ctrlKey || event.metaKey) || props.busy) return
  event.preventDefault()
  void setZoom(zoom.value + (event.deltaY < 0 ? 0.15 : -0.15), {
    clientX: event.clientX,
    clientY: event.clientY,
  })
}

function beginPan(event: PointerEvent) {
  if (props.busy || !stage.value) return
  if (event.button !== 1 && !(event.button === 0 && spacePressed)) return
  if ((event.target as HTMLElement).closest('.crop-region, button')) return
  event.preventDefault()
  pan = {
    startX: event.clientX,
    startY: event.clientY,
    scrollLeft: stage.value.scrollLeft,
    scrollTop: stage.value.scrollTop,
  }
  isPanning.value = true
  window.addEventListener('pointermove', movePan)
  window.addEventListener('pointerup', endPan, { once: true })
  window.addEventListener('pointercancel', endPan, { once: true })
}

function movePan(event: PointerEvent) {
  if (!pan || !stage.value) return
  stage.value.scrollLeft = pan.scrollLeft - (event.clientX - pan.startX)
  stage.value.scrollTop = pan.scrollTop - (event.clientY - pan.startY)
}

function endPan() {
  pan = undefined
  isPanning.value = false
  window.removeEventListener('pointermove', movePan)
  window.removeEventListener('pointerup', endPan)
  window.removeEventListener('pointercancel', endPan)
}

function isTextControl(target: EventTarget | null) {
  return target instanceof HTMLElement && Boolean(target.closest('input, textarea, select, button, [contenteditable="true"]'))
}

function previewStyle(region: Region) {
  const horizontal = region.width >= 0.999 ? 0 : region.x / (1 - region.width) * 100
  const vertical = region.height >= 0.999 ? 0 : region.y / (1 - region.height) * 100
  return {
    backgroundImage: `url(${JSON.stringify(renderedDataUrl.value)})`,
    backgroundSize: `${100 / region.width}% ${100 / region.height}%`,
    backgroundPosition: `${horizontal}% ${vertical}%`,
  }
}

function apply() {
  const recipes = regions.value.map<CaptureCropRecipe>(region => ({
    rect: { x: region.x, y: region.y, width: region.width, height: region.height },
    rotationDegrees: rotation.value,
    outputMediaType: 'image/png',
    maxEdge: 4096,
    jpegQuality: 90,
  }))
  emit('apply', recipes)
}

async function renderRotation() {
  const renderVersion = ++rotationRenderVersion
  const sourceDataUrl = props.dataUrl
  if (!rotation.value) {
    renderedDataUrl.value = sourceDataUrl
    return
  }
  const image = new Image()
  image.src = sourceDataUrl
  await image.decode()
  if (renderVersion !== rotationRenderVersion) return
  const canvas = document.createElement('canvas')
  const swapped = rotation.value === 90 || rotation.value === 270
  canvas.width = swapped ? image.naturalHeight : image.naturalWidth
  canvas.height = swapped ? image.naturalWidth : image.naturalHeight
  const context = canvas.getContext('2d')
  if (!context) return
  context.translate(canvas.width / 2, canvas.height / 2)
  context.rotate(rotation.value * Math.PI / 180)
  context.drawImage(image, -image.naturalWidth / 2, -image.naturalHeight / 2)
  const dataUrl = canvas.toDataURL('image/png')
  if (renderVersion === rotationRenderVersion) renderedDataUrl.value = dataUrl
}

function handleDialogKeydown(event: KeyboardEvent) {
  if (event.code === 'Space' && !isTextControl(event.target)) {
    spacePressed = true
    event.preventDefault()
  }
  if (event.key === 'Escape' && !props.busy) {
    event.preventDefault()
    emit('close')
  }
  if ((event.ctrlKey || event.metaKey) && event.key.toLowerCase() === 'z') {
    event.preventDefault()
    if (event.shiftKey) redo()
    else undo()
  }
  if (event.key === 'Tab' && dialog.value) {
    const focusable = [...dialog.value.querySelectorAll<HTMLElement>(
      'button:not([disabled]), [role="button"][tabindex="0"], [href], input:not([disabled]), select:not([disabled])',
    )]
    if (!focusable.length) return
    const first = focusable[0]!
    const last = focusable[focusable.length - 1]!
    if (event.shiftKey && document.activeElement === first) {
      event.preventDefault()
      last.focus()
    }
    else if (!event.shiftKey && document.activeElement === last) {
      event.preventDefault()
      first.focus()
    }
  }
}

function handleDialogKeyup(event: KeyboardEvent) {
  if (event.code === 'Space') spacePressed = false
}

watch([() => props.dataUrl, rotation], () => void renderRotation(), { immediate: true })

onMounted(async () => {
  previousBodyOverflow = document.body.style.overflow
  document.body.style.overflow = 'hidden'
  window.addEventListener('keydown', handleDialogKeydown)
  window.addEventListener('keyup', handleDialogKeyup)
  updateViewport()
  if (typeof ResizeObserver !== 'undefined' && stage.value) {
    resizeObserver = new ResizeObserver(updateViewport)
    resizeObserver.observe(stage.value)
  }
  await nextTick()
  closeButton.value?.focus()
})

onBeforeUnmount(() => {
  document.body.style.overflow = previousBodyOverflow
  window.removeEventListener('keydown', handleDialogKeydown)
  window.removeEventListener('keyup', handleDialogKeyup)
  window.removeEventListener('pointermove', movePointer)
  resizeObserver?.disconnect()
  clearTimeout(zoomScrollTimer)
  endPointer()
  endPan()
})
</script>

<template>
  <div
    class="crop-backdrop"
    role="presentation"
    @click.self="!busy && emit('close')"
  >
    <section
      ref="dialog"
      class="crop-dialog"
      role="dialog"
      aria-modal="true"
      aria-labelledby="crop-title"
    >
      <header>
        <div>
          <p>无损图片整理</p>
          <h2 id="crop-title">
            裁出真正需要的题目范围
          </h2>
          <span :title="itemName">{{ itemName }}</span>
        </div>
        <button
          ref="closeButton"
          type="button"
          class="icon-button"
          :disabled="busy"
          aria-label="关闭裁剪"
          @click="emit('close')"
        >
          <X :size="20" />
        </button>
      </header>

      <div
        class="crop-toolbar"
        aria-label="裁剪工具"
      >
        <button
          type="button"
          :disabled="busy || !undoStack.length"
          @click="undo"
        >
          <Undo2 :size="16" />撤销
        </button>
        <button
          type="button"
          :disabled="busy || !redoStack.length"
          @click="redo"
        >
          <Redo2 :size="16" />重做
        </button>
        <button
          type="button"
          :disabled="busy"
          @click="rotate"
        >
          <RotateCw :size="16" />旋转 90°
        </button>
        <button
          type="button"
          :disabled="busy || zoom <= 0.75"
          aria-label="缩小"
          @click="setZoom(zoom - .25)"
        >
          <ZoomOut :size="16" />
        </button>
        <strong>{{ Math.round(zoom * 100) }}%</strong>
        <button
          type="button"
          :disabled="busy || zoom >= 2.5"
          aria-label="放大"
          @click="setZoom(zoom + .25)"
        >
          <ZoomIn :size="16" />
        </button>
        <button
          type="button"
          :disabled="busy || regions.length >= 10"
          @click="addRegion"
        >
          <Plus :size="16" />再框一道题
        </button>
        <button
          type="button"
          :disabled="busy"
          @click="reset"
        >
          重置
        </button>
      </div>

      <div class="crop-body">
        <div
          ref="stage"
          class="crop-stage"
          :class="{ 'is-panning': isPanning }"
          role="region"
          aria-label="裁剪画布"
          tabindex="0"
          @wheel="handleWheel"
          @pointerdown="beginPan"
        >
          <div
            class="stage-surface"
            :style="stageSurfaceStyle"
          >
            <div
              class="image-shell"
              :style="imageShellStyle"
            >
              <img
                :src="renderedDataUrl"
                :alt="itemName"
                draggable="false"
                @load="readImageSize"
              >
              <div class="crop-overlay">
                <div
                  v-for="(region, index) in regions"
                  :key="region.id"
                  class="crop-region"
                  :class="{ 'is-active': region.id === activeId }"
                  :style="{ left: `${region.x * 100}%`, top: `${region.y * 100}%`, width: `${region.width * 100}%`, height: `${region.height * 100}%` }"
                  tabindex="0"
                  role="group"
                  :aria-label="`裁剪区域 ${index + 1}`"
                  @pointerdown="beginPointer(region, 'move', $event)"
                  @keydown="nudge($event, region)"
                >
                  <span>{{ index + 1 }}</span>
                  <button
                    v-for="handle in resizeHandles"
                    :key="handle.handle"
                    type="button"
                    class="resize-handle"
                    :class="`handle-${handle.handle}`"
                    :aria-label="`${handle.label}（区域 ${index + 1}）`"
                    @pointerdown="beginPointer(region, 'resize', $event, handle.handle)"
                  />
                </div>
              </div>
            </div>
          </div>
        </div>

        <aside>
          <div class="aside-heading">
            <span>输出顺序</span><strong>{{ regions.length }} 个区域</strong>
          </div>
          <TransitionGroup
            name="region-list"
            tag="div"
            class="region-list"
          >
            <div
              v-for="(region, index) in regions"
              :key="region.id"
              class="region-row"
              :class="{ 'is-active': region.id === activeId }"
            >
              <button
                type="button"
                class="region-select"
                :aria-label="`选择区域 ${index + 1}`"
                :aria-pressed="region.id === activeId"
                @click="activeId = region.id"
              >
                <span
                  class="region-preview"
                  :style="previewStyle(region)"
                ><b>{{ index + 1 }}</b></span>
                <span class="region-copy"><strong>区域 {{ index + 1 }}</strong><small>{{ Math.round(region.width * 100) }}% × {{ Math.round(region.height * 100) }}%</small></span>
              </button>
              <span class="region-actions">
                <button
                  type="button"
                  :disabled="index === 0"
                  :aria-label="`上移区域 ${index + 1}`"
                  @click="reorderRegion(region.id, -1)"
                ><ArrowUp :size="14" /></button>
                <button
                  type="button"
                  :disabled="index === regions.length - 1"
                  :aria-label="`下移区域 ${index + 1}`"
                  @click="reorderRegion(region.id, 1)"
                ><ArrowDown :size="14" /></button>
                <button
                  v-if="regions.length > 1"
                  type="button"
                  :aria-label="`删除区域 ${index + 1}`"
                  @click="removeRegion(region.id)"
                ><Trash2 :size="14" /></button>
              </span>
            </div>
          </TransitionGroup>
          <p class="keyboard-tip">
            方向键移动；Shift 加速；Alt + 方向键调整宽高。按住空格拖动画布，Ctrl + 滚轮缩放。所有区域会一次性保存，失败时原图不变。
          </p>
        </aside>
      </div>

      <footer>
        <p><Crop :size="17" /><span><strong>原图不会被覆盖</strong>，裁剪后仍可在入库前一键恢复。</span></p>
        <div>
          <button
            type="button"
            class="secondary"
            :disabled="busy"
            @click="emit('close')"
          >
            取消
          </button><button
            type="button"
            class="primary"
            :disabled="busy"
            @click="apply"
          >
            {{ busy ? '正在生成…' : `生成 ${regions.length} 张裁剪图` }}
          </button>
        </div>
      </footer>
    </section>
  </div>
</template>

<style scoped>
.crop-backdrop{position:fixed;inset:0;z-index:120;display:grid;padding:20px;background:rgba(20,29,26,.72);backdrop-filter:blur(8px);animation:fade-in var(--motion-standard) var(--ease-standard)}
.crop-dialog{display:grid;grid-template-rows:auto auto minmax(0,1fr) auto;max-width:1480px;width:100%;height:calc(100vh - 40px);margin:auto;overflow:hidden;border:1px solid rgba(232,221,199,.5);border-radius:24px;background:var(--warm-paper);box-shadow:0 28px 90px rgba(13,23,19,.35);animation:dialog-in var(--motion-page) var(--ease-standard)}
header,footer{display:flex;align-items:center;justify-content:space-between;padding:18px 22px;border-color:rgba(33,51,45,.1)} header{border-bottom:1px solid rgba(33,51,45,.1)} header p{margin:0;color:var(--cinnabar);font-size:12px;font-weight:700;letter-spacing:.12em} header h2{margin:3px 0;font-family:serif;font-size:24px} header span{display:block;max-width:62vw;overflow:hidden;color:var(--ink-muted);font-size:12px;text-overflow:ellipsis;white-space:nowrap}.icon-button{display:grid;width:40px;height:40px;place-items:center;border:0;border-radius:50%;background:rgba(33,51,45,.08);cursor:pointer}
.crop-toolbar{display:flex;align-items:center;gap:8px;padding:10px 22px;overflow-x:auto;border-bottom:1px solid rgba(33,51,45,.1);background:rgba(232,221,199,.28);scrollbar-width:thin}.crop-toolbar button,.secondary,.primary{display:inline-flex;flex:0 0 auto;align-items:center;gap:6px;min-height:36px;padding:0 12px;border:1px solid rgba(33,51,45,.14);border-radius:10px;background:rgba(255,253,247,.9);color:var(--ink);cursor:pointer}.crop-toolbar strong{min-width:44px;text-align:center;font-size:12px}.crop-toolbar button:disabled,footer button:disabled,.region-actions button:disabled{opacity:.45;cursor:not-allowed}
.crop-body{display:grid;grid-template-columns:minmax(0,1fr) 286px;min-height:0}.crop-stage{position:relative;overflow:auto;overscroll-behavior:contain;background:#18221f;background-image:radial-gradient(rgba(246,241,231,.08) 1px,transparent 1px);background-size:18px 18px;outline:none;scrollbar-color:rgba(246,241,231,.45) rgba(10,17,15,.5);scrollbar-width:thin}.crop-stage:focus-visible{box-shadow:inset 0 0 0 3px var(--cinnabar)}.crop-stage.is-panning{cursor:grabbing;user-select:none}.stage-surface{display:grid;min-width:100%;min-height:100%;place-items:center}.image-shell{position:relative;display:inline-grid;flex:none;transition:width var(--motion-standard) var(--ease-standard),height var(--motion-standard) var(--ease-standard);will-change:width,height}.image-shell>img{display:block;width:100%;height:100%;object-fit:fill;pointer-events:none;user-select:none;box-shadow:0 10px 38px rgba(0,0,0,.32)}.crop-overlay{position:absolute;inset:0}.crop-region{position:absolute;box-sizing:border-box;border:2px solid #f1c86b;background:rgba(241,200,107,.08);cursor:move;touch-action:none;box-shadow:0 0 0 9999px rgba(5,12,10,.1);transition:border-color var(--motion-feedback),background var(--motion-feedback),box-shadow var(--motion-feedback)}.crop-region:focus-visible{outline:3px solid #fff;outline-offset:3px}.crop-region.is-active{z-index:2;border-color:#ffdf84;background:rgba(255,223,132,.12);box-shadow:0 0 0 2px rgba(33,51,45,.72),0 8px 22px rgba(0,0,0,.22)}.crop-region>span{position:absolute;top:5px;left:5px;display:grid;width:25px;height:25px;place-items:center;border-radius:50%;background:#f6d375;color:#24312c;font-size:12px;font-weight:800}.resize-handle{position:absolute;width:18px;height:18px;padding:0;border:2px solid #fff;border-radius:5px;background:var(--cinnabar);touch-action:none}.resize-handle:focus-visible{outline:3px solid #fff;outline-offset:2px}.handle-n{top:-9px;left:calc(50% - 9px);cursor:ns-resize}.handle-ne{top:-9px;right:-9px;cursor:nesw-resize}.handle-e{top:calc(50% - 9px);right:-9px;cursor:ew-resize}.handle-se{right:-9px;bottom:-9px;cursor:nwse-resize}.handle-s{bottom:-9px;left:calc(50% - 9px);cursor:ns-resize}.handle-sw{bottom:-9px;left:-9px;cursor:nesw-resize}.handle-w{top:calc(50% - 9px);left:-9px;cursor:ew-resize}.handle-nw{top:-9px;left:-9px;cursor:nwse-resize}
aside{overflow:auto;padding:18px;border-left:1px solid rgba(33,51,45,.1);background:rgba(255,253,247,.7)}.aside-heading{display:flex;justify-content:space-between;margin-bottom:12px;font-size:12px}.region-list{display:grid;gap:8px}.region-row{display:grid;grid-template-columns:minmax(0,1fr) auto;align-items:center;gap:6px;padding:6px;border:1px solid transparent;border-radius:14px;background:rgba(232,221,199,.34);transition:transform var(--motion-standard) var(--ease-standard),opacity var(--motion-standard),border-color var(--motion-feedback),background var(--motion-feedback)}.region-row.is-active{border-color:rgba(185,88,63,.55);background:rgba(185,88,63,.09)}.region-select{display:grid;grid-template-columns:52px minmax(0,1fr);align-items:center;gap:9px;min-width:0;padding:0;border:0;background:transparent;color:var(--ink);text-align:left;cursor:pointer}.region-select:focus-visible,.region-actions button:focus-visible{outline:2px solid var(--cinnabar);outline-offset:2px}.region-preview{position:relative;display:block;width:52px;height:44px;overflow:hidden;border:1px solid rgba(33,51,45,.16);border-radius:9px;background-color:#fff;background-repeat:no-repeat}.region-preview b{position:absolute;top:4px;left:4px;display:grid;width:19px;height:19px;place-items:center;border-radius:6px;background:var(--deep-green);color:var(--paper);font-size:10px}.region-copy{display:grid;min-width:0}.region-copy strong,.region-copy small{overflow:hidden;text-overflow:ellipsis;white-space:nowrap}.region-copy small{color:var(--ink-muted)}.region-actions{display:grid;grid-template-columns:repeat(2,26px);gap:3px}.region-actions button{display:grid;width:26px;height:26px;padding:0;place-items:center;border:0;border-radius:7px;background:rgba(33,51,45,.07);color:var(--ink);cursor:pointer}.region-actions button:last-child{color:var(--cinnabar)}.region-list-move,.region-list-enter-active,.region-list-leave-active{transition:transform var(--motion-standard) var(--ease-standard),opacity var(--motion-standard)}.region-list-enter-from,.region-list-leave-to{opacity:0;transform:translateY(-8px)}.keyboard-tip{margin-top:18px;color:var(--ink-muted);font-size:12px;line-height:1.65}
footer{border-top:1px solid rgba(33,51,45,.1)}footer>p{display:flex;align-items:center;gap:9px;margin:0;color:var(--ink-muted);font-size:13px}footer>div{display:flex;gap:10px}.primary{border-color:var(--deep-green);background:var(--deep-green);color:var(--paper);font-weight:700}.secondary{background:transparent}
@keyframes fade-in{from{opacity:0}}@keyframes dialog-in{from{opacity:0;transform:translateY(12px) scale(.985)}}
@media(max-width:900px){.crop-backdrop{padding:0}.crop-dialog{height:100vh;border-radius:0}.crop-body{grid-template-columns:1fr;grid-template-rows:minmax(0,1fr) auto}.crop-body aside{display:block;max-height:150px;padding:10px 12px;overflow:hidden;border-top:1px solid rgba(33,51,45,.1);border-left:0}.crop-body .aside-heading{margin-bottom:7px}.crop-body .region-list{display:flex;gap:7px;overflow-x:auto;padding-bottom:4px;scrollbar-width:thin}.crop-body .region-row{flex:0 0 238px}.crop-body .keyboard-tip{display:none}.crop-toolbar{padding-inline:12px}footer{gap:10px;padding:12px}footer>p{display:none}}
@media(max-width:520px){header h2{font-size:19px}header{padding:12px 14px}.crop-toolbar button{min-height:40px}.crop-body aside{max-height:126px}.crop-dialog footer>div{display:grid;width:100%;grid-template-columns:1fr 1fr}.crop-dialog footer button{justify-content:center;padding-inline:8px}}
@media(forced-colors:active){.crop-region,.crop-region.is-active,.resize-handle{border-color:CanvasText;forced-color-adjust:auto}.crop-region.is-active{outline:3px solid Highlight}.region-row.is-active{border-color:Highlight}}
@media(prefers-reduced-motion:reduce){.crop-backdrop,.crop-dialog{animation:none}.image-shell,.crop-region,.region-row,.region-list-move,.region-list-enter-active,.region-list-leave-active{transition:none}}
</style>
