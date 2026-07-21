import { computed, onScopeDispose, readonly, ref } from 'vue'

const MAX_REVIEW_DURATION_MS = 86_400_000
const REFRESH_INTERVAL_MS = 250

export interface ReviewClockOptions {
  limitSeconds?: number | null
}

function normalizeLimit(limitSeconds?: number | null) {
  if (!Number.isInteger(limitSeconds) || (limitSeconds ?? 0) <= 0)
    return null
  return Math.min(limitSeconds as number, MAX_REVIEW_DURATION_MS / 1_000)
}

function formatClock(totalSeconds: number) {
  const safeSeconds = Math.max(0, Math.trunc(totalSeconds))
  const minutes = Math.floor(safeSeconds / 60)
  const seconds = safeSeconds % 60
  return `${String(minutes).padStart(2, '0')}:${String(seconds).padStart(2, '0')}`
}

export function useReviewClock(options: ReviewClockOptions = {}) {
  const elapsedMs = ref(0)
  const running = ref(false)
  const limitSeconds = ref<number | null>(normalizeLimit(options.limitSeconds))
  let accumulatedMs = 0
  let startedAt = 0
  let timer: ReturnType<typeof setInterval> | undefined

  function sample() {
    if (!running.value)
      return
    elapsedMs.value = Math.min(
      MAX_REVIEW_DURATION_MS,
      Math.max(0, Math.round(accumulatedMs + performance.now() - startedAt)),
    )
  }

  function clearTimer() {
    if (timer !== undefined) {
      clearInterval(timer)
      timer = undefined
    }
  }

  function start() {
    if (running.value)
      return
    startedAt = performance.now()
    running.value = true
    timer = setInterval(sample, REFRESH_INTERVAL_MS)
  }

  function stop() {
    if (running.value) {
      sample()
      accumulatedMs = elapsedMs.value
      running.value = false
    }
    clearTimer()
  }

  function reset(nextLimitSeconds?: number | null) {
    stop()
    accumulatedMs = 0
    elapsedMs.value = 0
    limitSeconds.value = normalizeLimit(nextLimitSeconds)
  }

  const expired = computed(() => limitSeconds.value !== null
    && elapsedMs.value >= limitSeconds.value * 1_000)
  const displayText = computed(() => {
    if (limitSeconds.value === null)
      return formatClock(Math.floor(elapsedMs.value / 1_000))
    const remainingMs = Math.max(0, limitSeconds.value * 1_000 - elapsedMs.value)
    return formatClock(Math.ceil(remainingMs / 1_000))
  })

  onScopeDispose(stop)

  return {
    elapsedMs: readonly(elapsedMs),
    displayText,
    expired,
    running: readonly(running),
    start,
    stop,
    reset,
  }
}
