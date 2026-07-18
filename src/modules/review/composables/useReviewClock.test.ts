import { effectScope } from 'vue'
import { afterEach, describe, expect, it, vi } from 'vitest'
import { useReviewClock } from './useReviewClock'

afterEach(() => {
  vi.useRealTimers()
})

describe('useReviewClock', () => {
  it('tracks monotonic elapsed time and freezes when stopped', () => {
    vi.useFakeTimers()
    const scope = effectScope()
    const clock = scope.run(() => useReviewClock())!

    clock.start()
    vi.advanceTimersByTime(1_650)
    expect(clock.displayText.value).toBe('00:01')

    clock.stop()
    const frozen = clock.elapsedMs.value
    vi.advanceTimersByTime(2_000)
    expect(clock.elapsedMs.value).toBe(frozen)
    expect(clock.running.value).toBe(false)
    scope.stop()
  })

  it('shows a countdown and marks expiration without auto-stopping', () => {
    vi.useFakeTimers()
    const scope = effectScope()
    const clock = scope.run(() => useReviewClock({ limitSeconds: 3 }))!

    expect(clock.displayText.value).toBe('00:03')
    clock.start()
    vi.advanceTimersByTime(1_250)
    expect(clock.displayText.value).toBe('00:02')
    expect(clock.expired.value).toBe(false)

    vi.advanceTimersByTime(2_000)
    expect(clock.displayText.value).toBe('00:00')
    expect(clock.expired.value).toBe(true)
    expect(clock.running.value).toBe(true)
    scope.stop()
  })

  it('resets duration and accepts a new limit between cards', () => {
    vi.useFakeTimers()
    const scope = effectScope()
    const clock = scope.run(() => useReviewClock())!

    clock.start()
    vi.advanceTimersByTime(800)
    clock.reset(10)

    expect(clock.elapsedMs.value).toBe(0)
    expect(clock.displayText.value).toBe('00:10')
    expect(clock.running.value).toBe(false)
    clock.start()
    vi.advanceTimersByTime(500)
    expect(clock.elapsedMs.value).toBeGreaterThanOrEqual(500)
    scope.stop()
  })

  it('clamps a submitted duration to one day', () => {
    vi.useFakeTimers()
    const scope = effectScope()
    const clock = scope.run(() => useReviewClock())!

    clock.start()
    vi.advanceTimersByTime(86_400_500)
    clock.stop()
    expect(clock.elapsedMs.value).toBe(86_400_000)
    scope.stop()
  })
})
