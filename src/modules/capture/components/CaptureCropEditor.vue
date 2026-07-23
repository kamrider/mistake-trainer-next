<script setup lang="ts">
import { Crop, Plus, Redo2, RotateCw, Trash2, Undo2, X, ZoomIn, ZoomOut } from '@lucide/vue'
import { nextTick, onBeforeUnmount, onMounted, ref, watch } from 'vue'
import type { CaptureCropRecipe } from '../../../shared/api/bindings'

type Region = { id: string, x: number, y: number, width: number, height: number }
type Snapshot = { rotation: 0 | 90 | 180 | 270, regions: Region[] }

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
const renderedDataUrl = ref(props.dataUrl)
const rotation = ref<0 | 90 | 180 | 270>(0)
const zoom = ref(1)
const regions = ref<Region[]>([makeRegion(0)])
const activeId = ref(regions.value[0]!.id)
const undoStack = ref<Snapshot[]>([])
const redoStack = ref<Snapshot[]>([])
let drag: { id: string, mode: 'move' | 'resize', startX: number, startY: number, before: Region } | undefined
let previousBodyOverflow = ''

function makeRegion(index: number): Region {
  const inset = Math.min(0.06 + index * 0.018, 0.2)
  return { id: crypto.randomUUID(), x: inset, y: inset, width: 1 - inset * 2, height: 1 - inset * 2 }
}

function snapshot(): Snapshot {
  return { rotation: rotation.value, regions: regions.value.map(region => ({ ...region })) }
}

function restore(value: Snapshot) {
  rotation.value = value.rotation
  regions.value = value.regions.map(region => ({ ...region }))
  activeId.value = regions.value[0]?.id ?? ''
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

function beginPointer(region: Region, mode: 'move' | 'resize', event: PointerEvent) {
  if (props.busy) return
  event.preventDefault()
  event.stopPropagation()
  activeId.value = region.id
  checkpoint()
  drag = { id: region.id, mode, startX: event.clientX, startY: event.clientY, before: { ...region } }
  window.addEventListener('pointermove', movePointer)
  window.addEventListener('pointerup', endPointer, { once: true })
}

function movePointer(event: PointerEvent) {
  if (!drag) return
  const overlay = dialog.value?.querySelector<HTMLElement>('.crop-overlay')
  if (!overlay) return
  const width = overlay.clientWidth * zoom.value
  const height = overlay.clientHeight * zoom.value
  if (!width || !height) return
  const dx = (event.clientX - drag.startX) / width
  const dy = (event.clientY - drag.startY) / height
  const region = regions.value.find(value => value.id === drag!.id)
  if (!region) return
  if (drag.mode === 'move') {
    region.x = clamp(drag.before.x + dx, 0, 1 - region.width)
    region.y = clamp(drag.before.y + dy, 0, 1 - region.height)
  }
  else {
    region.width = clamp(drag.before.width + dx, 0.015, 1 - region.x)
    region.height = clamp(drag.before.height + dy, 0.015, 1 - region.y)
  }
}

function endPointer() {
  drag = undefined
  window.removeEventListener('pointermove', movePointer)
}

function nudge(event: KeyboardEvent, region: Region) {
  if (!['ArrowLeft', 'ArrowRight', 'ArrowUp', 'ArrowDown'].includes(event.key)) return
  event.preventDefault()
  checkpoint()
  const delta = event.shiftKey ? 0.02 : 0.005
  const direction = event.key === 'ArrowLeft' || event.key === 'ArrowUp' ? -delta : delta
  if (event.altKey) {
    if (event.key === 'ArrowLeft' || event.key === 'ArrowRight') {
      region.width = clamp(region.width + direction, 0.015, 1 - region.x)
    }
    else region.height = clamp(region.height + direction, 0.015, 1 - region.y)
  }
  else if (event.key === 'ArrowLeft' || event.key === 'ArrowRight') {
    region.x = clamp(region.x + direction, 0, 1 - region.width)
  }
  else region.y = clamp(region.y + direction, 0, 1 - region.height)
}

function clamp(value: number, min: number, max: number) {
  return Math.min(max, Math.max(min, value))
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
  if (!rotation.value) {
    renderedDataUrl.value = props.dataUrl
    return
  }
  const image = new Image()
  image.src = props.dataUrl
  await image.decode()
  const canvas = document.createElement('canvas')
  const swapped = rotation.value === 90 || rotation.value === 270
  canvas.width = swapped ? image.naturalHeight : image.naturalWidth
  canvas.height = swapped ? image.naturalWidth : image.naturalHeight
  const context = canvas.getContext('2d')
  if (!context) return
  context.translate(canvas.width / 2, canvas.height / 2)
  context.rotate(rotation.value * Math.PI / 180)
  context.drawImage(image, -image.naturalWidth / 2, -image.naturalHeight / 2)
  renderedDataUrl.value = canvas.toDataURL('image/png')
}

function handleDialogKeydown(event: KeyboardEvent) {
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

watch([() => props.dataUrl, rotation], () => void renderRotation(), { immediate: true })

onMounted(async () => {
  previousBodyOverflow = document.body.style.overflow
  document.body.style.overflow = 'hidden'
  window.addEventListener('keydown', handleDialogKeydown)
  await nextTick()
  closeButton.value?.focus()
})

onBeforeUnmount(() => {
  document.body.style.overflow = previousBodyOverflow
  window.removeEventListener('keydown', handleDialogKeydown)
  window.removeEventListener('pointermove', movePointer)
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
          @click="zoom = Math.max(.75, zoom - .25)"
        >
          <ZoomOut :size="16" />
        </button>
        <strong>{{ Math.round(zoom * 100) }}%</strong>
        <button
          type="button"
          :disabled="busy || zoom >= 2.5"
          aria-label="放大"
          @click="zoom = Math.min(2.5, zoom + .25)"
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
        <div class="crop-stage">
          <div
            class="image-shell"
            :style="{ transform: `scale(${zoom})` }"
          >
            <img
              :src="renderedDataUrl"
              :alt="itemName"
            >
            <div class="crop-overlay">
              <div
                v-for="(region, index) in regions"
                :key="region.id"
                class="crop-region"
                :class="{ 'is-active': region.id === activeId }"
                :style="{ left: `${region.x * 100}%`, top: `${region.y * 100}%`, width: `${region.width * 100}%`, height: `${region.height * 100}%` }"
                tabindex="0"
                role="button"
                :aria-label="`裁剪区域 ${index + 1}`"
                @pointerdown="beginPointer(region, 'move', $event)"
                @keydown="nudge($event, region)"
              >
                <span>{{ index + 1 }}</span>
                <button
                  type="button"
                  class="resize-handle"
                  aria-label="调整右下角"
                  @pointerdown="beginPointer(region, 'resize', $event)"
                />
              </div>
            </div>
          </div>
        </div>

        <aside>
          <div class="aside-heading">
            <span>输出顺序</span><strong>{{ regions.length }} 个区域</strong>
          </div>
          <div
            v-for="(region, index) in regions"
            :key="region.id"
            class="region-row"
            :class="{ 'is-active': region.id === activeId }"
            role="button"
            tabindex="0"
            @click="activeId = region.id"
            @keydown.enter.prevent="activeId = region.id"
          >
            <span>{{ index + 1 }}</span><p><strong>区域 {{ index + 1 }}</strong><small>{{ Math.round(region.width * 100) }}% × {{ Math.round(region.height * 100) }}%</small></p>
            <button
              v-if="regions.length > 1"
              type="button"
              aria-label="删除这个区域"
              @click.stop="removeRegion(region.id)"
            >
              <Trash2 :size="15" />
            </button>
          </div>
          <p class="keyboard-tip">
            方向键移动；Shift 加速；Alt + 方向键调整宽高。所有区域会一次性保存，失败时原图不变。
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
.crop-toolbar{display:flex;align-items:center;gap:8px;padding:10px 22px;border-bottom:1px solid rgba(33,51,45,.1);background:rgba(232,221,199,.28)}.crop-toolbar button,.secondary,.primary{display:inline-flex;align-items:center;gap:6px;min-height:36px;padding:0 12px;border:1px solid rgba(33,51,45,.14);border-radius:10px;background:rgba(255,253,247,.9);color:var(--ink);cursor:pointer}.crop-toolbar strong{min-width:44px;text-align:center;font-size:12px}.crop-toolbar button:disabled,footer button:disabled{opacity:.45;cursor:not-allowed}
.crop-body{display:grid;grid-template-columns:minmax(0,1fr) 250px;min-height:0}.crop-stage{display:grid;overflow:auto;place-items:center;padding:28px;background:#18221f;background-image:radial-gradient(rgba(246,241,231,.08) 1px,transparent 1px);background-size:18px 18px}.image-shell{position:relative;display:inline-grid;transform-origin:center;transition:transform var(--motion-standard) var(--ease-standard)}.image-shell>img{display:block;max-width:min(1000px,calc(100vw - 370px));max-height:calc(100vh - 245px);object-fit:contain;box-shadow:0 10px 38px rgba(0,0,0,.32)}.crop-overlay{position:absolute;inset:0}.crop-region{position:absolute;box-sizing:border-box;border:2px solid #f1c86b;background:rgba(241,200,107,.08);cursor:move;box-shadow:0 0 0 9999px rgba(5,12,10,.1);transition:border-color var(--motion-feedback),background var(--motion-feedback)}.crop-region.is-active{z-index:2;border-color:#ffdf84;background:rgba(255,223,132,.12);box-shadow:0 0 0 2px rgba(33,51,45,.72),0 8px 22px rgba(0,0,0,.22)}.crop-region>span{position:absolute;top:5px;left:5px;display:grid;width:25px;height:25px;place-items:center;border-radius:50%;background:#f6d375;color:#24312c;font-size:12px;font-weight:800}.resize-handle{position:absolute;right:-8px;bottom:-8px;width:18px;height:18px;padding:0;border:2px solid #fff;border-radius:5px;background:var(--cinnabar);cursor:nwse-resize}
aside{overflow:auto;padding:18px;border-left:1px solid rgba(33,51,45,.1);background:rgba(255,253,247,.7)}.aside-heading{display:flex;justify-content:space-between;margin-bottom:12px;font-size:12px}.region-row{display:grid;grid-template-columns:30px minmax(0,1fr) auto;width:100%;align-items:center;gap:8px;margin-bottom:8px;padding:9px;border:1px solid transparent;border-radius:12px;background:rgba(232,221,199,.34);text-align:left;cursor:pointer}.region-row.is-active{border-color:rgba(185,88,63,.55);background:rgba(185,88,63,.09)}.region-row>span{display:grid;width:28px;height:28px;place-items:center;border-radius:8px;background:var(--deep-green);color:var(--paper);font-weight:700}.region-row p{display:grid;margin:0}.region-row small{color:var(--ink-muted)}.region-row>button{display:grid;width:30px;height:30px;place-items:center;border:0;border-radius:50%;background:transparent;color:var(--cinnabar)}.keyboard-tip{margin-top:18px;color:var(--ink-muted);font-size:12px;line-height:1.65}
footer{border-top:1px solid rgba(33,51,45,.1)}footer>p{display:flex;align-items:center;gap:9px;margin:0;color:var(--ink-muted);font-size:13px}footer>div{display:flex;gap:10px}.primary{border-color:var(--deep-green);background:var(--deep-green);color:var(--paper);font-weight:700}.secondary{background:transparent}
@keyframes fade-in{from{opacity:0}}@keyframes dialog-in{from{opacity:0;transform:translateY(12px) scale(.985)}}
@media(max-width:900px){.crop-backdrop{padding:0}.crop-dialog{height:100vh;border-radius:0}.crop-body{grid-template-columns:1fr}.crop-body aside{display:none}.image-shell>img{max-width:calc(100vw - 30px)}footer>p{display:none}}
@media(prefers-reduced-motion:reduce){.crop-backdrop,.crop-dialog{animation:none}.image-shell{transition:none}}
</style>
