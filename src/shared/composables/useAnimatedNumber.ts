import {
  onScopeDispose,
  readonly,
  ref,
  toValue,
  watch,
  type MaybeRefOrGetter,
} from 'vue'

function safeInteger(value: number) {
  return Number.isFinite(value) ? Math.round(value) : 0
}

export function useAnimatedNumber(
  source: MaybeRefOrGetter<number>,
  durationMs = 360,
) {
  const value = ref(0)
  const reducedMotion = typeof window !== 'undefined' && typeof window.matchMedia === 'function'
    ? window.matchMedia('(prefers-reduced-motion: reduce)')
    : null
  let frame = 0
  let initialized = false
  let latestTarget = 0

  const stopAnimation = () => {
    if (frame !== 0) {
      cancelAnimationFrame(frame)
      frame = 0
    }
  }

  const animateTo = (nextValue: number) => {
    const target = safeInteger(nextValue)
    latestTarget = target
    stopAnimation()
    if (!initialized || reducedMotion?.matches || durationMs <= 0) {
      initialized = true
      value.value = target
      return
    }

    const from = value.value
    if (from === target) return
    const startedAt = performance.now()
    const tick = (now: number) => {
      const progress = Math.min(1, Math.max(0, (now - startedAt) / durationMs))
      const eased = 1 - (1 - progress) ** 3
      value.value = Math.round(from + (target - from) * eased)
      if (progress < 1) {
        frame = requestAnimationFrame(tick)
      } else {
        frame = 0
        value.value = target
      }
    }
    frame = requestAnimationFrame(tick)
  }

  const stopWatching = watch(() => toValue(source), animateTo, { immediate: true })
  const onMotionChange = (event: MediaQueryListEvent) => {
    if (event.matches) {
      stopAnimation()
      value.value = latestTarget
    }
  }
  reducedMotion?.addEventListener?.('change', onMotionChange)

  onScopeDispose(() => {
    stopAnimation()
    stopWatching()
    reducedMotion?.removeEventListener?.('change', onMotionChange)
  })

  return readonly(value)
}
