<script setup lang="ts">
import { Eye, SkipForward } from '@lucide/vue'
import { computed, nextTick, onBeforeUnmount, onMounted, ref, watch } from 'vue'
import type { ReviewFocusState } from '@/shared/api/bindings'

const props = defineProps<{
  focus: ReviewFocusState
  busy?: boolean
  completed?: boolean
  resumed?: boolean
}>()

const emit = defineEmits<{
  select: [number: number, elapsedMs: number]
  skip: []
  exit: []
}>()

const elapsedMs = ref(props.focus.elapsedMs)
const wrongNumber = ref<number>()
const wrongHint = ref('')
const board = ref<HTMLElement>()
const activeIndex = ref(Math.max(0, props.focus.numbers.indexOf(props.focus.nextNumber)))
let startedAt = performance.now() - props.focus.elapsedMs
let ticker: number | undefined
let wrongTimer: number | undefined

const elapsedText = computed(() => `${(elapsedMs.value / 1000).toFixed(1)} 秒`)
const completedCount = computed(() => Math.max(0, props.focus.nextNumber - 1))
const heading = computed(() => props.focus.kind === 'warmup' ? '先让视线醒过来' : '让眼睛换一条路')
const kicker = computed(() => props.focus.kind === 'warmup' ? '训练前热身' : '十题后的短休息')

function updateElapsed() {
  elapsedMs.value = Math.max(props.focus.elapsedMs, performance.now() - startedAt)
}

function selectNumber(number: number) {
  if (props.busy || number < props.focus.nextNumber) return
  updateElapsed()
  if (number !== props.focus.nextNumber) {
    wrongNumber.value = number
    wrongHint.value = `请先找到 ${props.focus.nextNumber}`
    if (wrongTimer !== undefined) window.clearTimeout(wrongTimer)
    wrongTimer = window.setTimeout(() => {
      wrongNumber.value = undefined
    }, 420)
    return
  }
  wrongHint.value = ''
  emit('select', number, Math.min(3_600_000, Math.round(elapsedMs.value)))
}

function selectable(index: number) {
  return props.focus.numbers[index]! >= props.focus.nextNumber
}

function focusIndex(index: number) {
  activeIndex.value = index
  nextTick(() => board.value?.querySelectorAll<HTMLButtonElement>('.number-tile')[index]?.focus())
}

function moveFocus(event: KeyboardEvent, index: number) {
  let delta = 0
  if (event.key === 'ArrowRight') delta = 1
  else if (event.key === 'ArrowLeft') delta = -1
  else if (event.key === 'ArrowDown') delta = 5
  else if (event.key === 'ArrowUp') delta = -5
  else if (event.key === 'Home') {
    event.preventDefault()
    const first = props.focus.numbers.findIndex(number => number >= props.focus.nextNumber)
    if (first >= 0) focusIndex(first)
    return
  }
  else if (event.key === 'End') {
    event.preventDefault()
    for (let candidate = props.focus.numbers.length - 1; candidate >= 0; candidate -= 1) {
      if (selectable(candidate)) {
        focusIndex(candidate)
        return
      }
    }
    return
  }
  else return

  event.preventDefault()
  let candidate = index
  for (let attempts = 0; attempts < props.focus.numbers.length; attempts += 1) {
    candidate = (candidate + delta + props.focus.numbers.length) % props.focus.numbers.length
    if (selectable(candidate)) {
      focusIndex(candidate)
      return
    }
  }
}

watch(
  () => props.focus.roundIndex,
  () => {
    elapsedMs.value = props.focus.elapsedMs
    startedAt = performance.now() - props.focus.elapsedMs
    wrongHint.value = ''
    wrongNumber.value = undefined
  },
)

watch(
  () => props.focus.nextNumber,
  (nextNumber) => {
    const nextIndex = props.focus.numbers.indexOf(nextNumber)
    if (nextIndex >= 0 && !selectable(activeIndex.value)) focusIndex(nextIndex)
  },
)

onMounted(() => {
  updateElapsed()
  ticker = window.setInterval(updateElapsed, 100)
})

onBeforeUnmount(() => {
  if (ticker !== undefined) window.clearInterval(ticker)
  if (wrongTimer !== undefined) window.clearTimeout(wrongTimer)
})
</script>

<template>
  <main
    class="focus-room"
    aria-labelledby="focus-heading"
  >
    <section class="focus-copy">
      <span class="focus-kicker"><Eye :size="16" />{{ kicker }}</span>
      <h1 id="focus-heading">
        {{ heading }}
      </h1>
      <p>{{ resumed ? '已恢复上次位置；继续从当前数字向后寻找。' : '从 1 到 25 依次点击。找错不会扣分，也不会改动训练进度。' }}</p>
      <div
        class="focus-status"
        aria-live="polite"
      >
        <strong>下一位 {{ focus.nextNumber }}</strong>
        <span>{{ completedCount }} / 25</span>
        <span>{{ elapsedText }}</span>
      </div>
      <p
        class="wrong-hint"
        :class="{ visible: wrongHint }"
        role="status"
      >
        {{ wrongHint || `正在寻找 ${focus.nextNumber}` }}
      </p>
    </section>

    <div
      ref="board"
      class="schulte-board"
      role="grid"
      :aria-busy="busy"
      aria-label="舒尔特数字方格"
    >
      <div
        v-for="(number, index) in focus.numbers"
        :key="number"
        class="grid-cell"
        role="gridcell"
      >
        <button
          type="button"
          class="number-tile"
          :class="{
            completed: number < focus.nextNumber,
            wrong: number === wrongNumber,
          }"
          :disabled="busy || number < focus.nextNumber"
          :tabindex="index === activeIndex && number >= focus.nextNumber ? 0 : -1"
          :aria-label="`数字 ${number}`"
          :aria-current="number === focus.nextNumber ? 'step' : undefined"
          @focus="activeIndex = index"
          @keydown="moveFocus($event, index)"
          @click="selectNumber(number)"
        >
          <span>{{ number }}</span>
        </button>
      </div>
      <div
        v-if="completed"
        class="completion-seal"
        role="status"
      >
        <strong>定神</strong>
        <span>这一轮已保存</span>
      </div>
    </div>

    <footer>
      <span>本轮可随时跳过；普通训练进度已经安全保存。</span>
      <div class="focus-actions">
        <button
          type="button"
          class="exit-focus"
          :disabled="busy"
          @click="emit('exit')"
        >
          退出训练台
        </button>
        <button
          type="button"
          class="skip-focus"
          :disabled="busy"
          @click="emit('skip')"
        >
          <SkipForward :size="17" />{{ busy ? '正在保存…' : '跳过，继续训练' }}
        </button>
      </div>
    </footer>
  </main>
</template>

<style scoped>
.focus-room { display: grid; min-height: 100vh; padding: clamp(24px, 5vw, 64px); place-content: center; justify-items: center; background: radial-gradient(circle at 50% 30%, rgba(255,253,247,.96), transparent 36rem), linear-gradient(145deg, rgba(232,221,199,.58), rgba(246,241,231,.92)); }
.focus-copy { width: min(660px, 100%); text-align: center; }
.focus-kicker { display: inline-flex; gap: 7px; align-items: center; color: var(--cinnabar); font-size: 11px; font-weight: 820; letter-spacing: .13em; }
h1 { margin: 10px 0 8px; color: var(--green-deep); font-family: var(--font-serif); font-size: clamp(30px, 5vw, 48px); letter-spacing: -.04em; }
.focus-copy>p { margin: 0; color: var(--ink-muted); font-size: 13px; }
.focus-status { display: flex; gap: 12px; align-items: center; justify-content: center; margin-top: 18px; }
.focus-status strong, .focus-status span { min-width: 74px; padding: 7px 12px; border: 1px solid var(--line); border-radius: 999px; background: rgba(255,253,247,.76); font-size: 11px; }
.focus-status strong { color: var(--paper); border-color: var(--green-deep); background: var(--green-deep); }
.wrong-hint { min-height: 20px; margin-top: 10px !important; opacity: .55; transition: opacity var(--motion-feedback), transform var(--motion-feedback); }
.wrong-hint.visible { color: var(--cinnabar-dark); opacity: 1; transform: translateY(-1px); }
.schulte-board { position: relative; display: grid; width: min(590px, calc(100vw - 40px)); margin: clamp(20px, 4vh, 34px) 0; grid-template-columns: repeat(5, 1fr); gap: clamp(7px, 1.4vw, 13px); perspective: 900px; }
.grid-cell { display: grid; aspect-ratio: 1; }
.number-tile { position: relative; width: 100%; height: 100%; padding: 0; overflow: hidden; color: var(--ink); border: 1px solid rgba(34,48,43,.18); border-radius: clamp(10px, 2vw, 18px); background: rgba(255,253,247,.9); box-shadow: 0 10px 28px rgba(60,48,31,.07); cursor: pointer; font-family: var(--font-serif); font-size: clamp(19px, 4vw, 32px); font-variant-numeric: tabular-nums; transition: transform var(--motion-feedback) var(--ease-standard), opacity var(--motion-standard) var(--ease-standard); }
.number-tile:not(:disabled):hover { border-color: rgba(33,51,45,.42); box-shadow: 0 14px 32px rgba(60,48,31,.11); transform: translateY(-2px) scale(1.02); }
.number-tile.wrong { border-color: var(--cinnabar); background: #fbebe4; animation: tile-corrective var(--motion-standard) var(--ease-standard); }
.number-tile.completed { pointer-events: none; opacity: .16; transform: scale(.72) rotateX(12deg); }
.number-tile.completed span { opacity: 0; }
footer { display: flex; width: min(660px, 100%); gap: 18px; align-items: center; justify-content: space-between; color: var(--ink-muted); font-size: 11px; }
.focus-actions { display: flex; flex: 0 0 auto; gap: 8px; align-items: center; }
.exit-focus { min-height: 44px; padding: 0 13px; color: var(--ink-muted); border: 0; border-radius: 999px; background: transparent; cursor: pointer; }
.skip-focus { display: inline-flex; flex: 0 0 auto; min-height: 44px; gap: 7px; align-items: center; padding: 0 16px; color: var(--green-deep); border: 1px solid rgba(33,51,45,.25); border-radius: 999px; background: rgba(255,253,247,.78); cursor: pointer; font-weight: 720; transition: transform var(--motion-feedback); }
.skip-focus:hover:not(:disabled) { background: var(--paper-raised); transform: translateY(-1px); }
.skip-focus:disabled { cursor: wait; opacity: .55; }
.completion-seal { position: absolute; inset: 50% auto auto 50%; display: grid; width: 150px; height: 150px; place-content: center; color: var(--paper); border-radius: 50%; background: var(--cinnabar); box-shadow: 0 18px 50px rgba(185,88,63,.25); text-align: center; transform: translate(-50%,-50%) rotate(-4deg); animation: seal-in 420ms var(--ease-standard) both; }
.completion-seal strong { font-family: var(--font-serif); font-size: 34px; letter-spacing: .12em; }.completion-seal span { margin-top: 6px; font-size: 10px; letter-spacing: .08em; }
@keyframes tile-corrective { 35% { transform: translateX(-4px); } 70% { transform: translateX(4px); } }
@keyframes seal-in { from { opacity: 0; transform: translate(-50%,-50%) scale(.78) rotate(-9deg); } to { opacity: 1; transform: translate(-50%,-50%) scale(1) rotate(-4deg); } }
@media (max-width: 560px) { .focus-room { padding: 24px 16px; } .schulte-board { width: 100%; gap: 7px; } footer { align-items: stretch; flex-direction: column; text-align: center; } .focus-actions { justify-content: center; }.skip-focus { justify-content: center; } }
@media (prefers-reduced-motion: reduce) { .number-tile, .wrong-hint, .skip-focus, .completion-seal { animation: none; transition: none; } .number-tile.completed, .number-tile:not(:disabled):hover { transform: none; } }
@media (forced-colors: active) { .number-tile.completed { opacity: .45; } }
</style>
