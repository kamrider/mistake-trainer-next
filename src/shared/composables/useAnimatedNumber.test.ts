import { effectScope, nextTick, ref } from 'vue'
import { afterEach, beforeEach, describe, expect, it, vi } from 'vitest'
import { useAnimatedNumber } from './useAnimatedNumber'

function installMotionPreference(matches: boolean) {
  vi.stubGlobal('matchMedia', vi.fn(() => ({
    matches,
    media: '(prefers-reduced-motion: reduce)',
    onchange: null,
    addEventListener: vi.fn(),
    removeEventListener: vi.fn(),
    addListener: vi.fn(),
    removeListener: vi.fn(),
    dispatchEvent: vi.fn(),
  })))
}

describe('useAnimatedNumber', () => {
  beforeEach(() => {
    vi.useFakeTimers()
    vi.stubGlobal('requestAnimationFrame', (callback: FrameRequestCallback) =>
      window.setTimeout(() => callback(performance.now()), 16))
    vi.stubGlobal('cancelAnimationFrame', (handle: number) => window.clearTimeout(handle))
  })

  afterEach(() => {
    vi.useRealTimers()
    vi.unstubAllGlobals()
  })

  it('animates later value changes and lands on the exact integer', async () => {
    installMotionPreference(false)
    const target = ref(0)
    const scope = effectScope()
    const shown = scope.run(() => useAnimatedNumber(target, 100))!

    target.value = 10
    await nextTick()
    await vi.advanceTimersByTimeAsync(48)
    expect(shown.value).toBeGreaterThan(0)
    expect(shown.value).toBeLessThan(10)

    await vi.advanceTimersByTimeAsync(100)
    expect(shown.value).toBe(10)
    scope.stop()
  })

  it('updates immediately when reduced motion is requested', async () => {
    installMotionPreference(true)
    const target = ref(3)
    const scope = effectScope()
    const shown = scope.run(() => useAnimatedNumber(target, 100))!

    target.value = 12
    await nextTick()
    expect(shown.value).toBe(12)
    scope.stop()
  })
})
