import { render, screen } from '@testing-library/vue'
import userEvent from '@testing-library/user-event'
import { defineComponent, ref } from 'vue'
import { beforeEach, describe, expect, it, vi } from 'vitest'
import { useCaptureFeedback } from './useCaptureFeedback'

const oscillator = { frequency: { value: 0 }, connect: vi.fn(), start: vi.fn(), stop: vi.fn() }
const gain = { gain: { setValueAtTime: vi.fn(), exponentialRampToValueAtTime: vi.fn() }, connect: vi.fn() }
const audioContext = {
  currentTime: 1,
  destination: {},
  createOscillator: vi.fn(() => oscillator),
  createGain: vi.fn(() => gain),
  close: vi.fn(),
}

function renderFeedback(enabled = true) {
  const Host = defineComponent({
    setup() {
      const soundEnabled = ref(enabled)
      return { soundEnabled, feedback: useCaptureFeedback(soundEnabled) }
    },
    template: `
      <button @click="feedback.playDrop('question')">question</button>
      <button @click="feedback.playDrop('answer')">answer</button>
      <button @click="soundEnabled = false">disable</button>
    `,
  })
  return render(Host)
}

describe('useCaptureFeedback', () => {
  beforeEach(() => {
    vi.clearAllMocks()
    vi.stubGlobal('AudioContext', vi.fn(function () { return audioContext }))
    vi.stubGlobal('matchMedia', vi.fn(() => ({
      matches: false,
      addEventListener: vi.fn(),
      removeEventListener: vi.fn(),
    })))
  })

  it('plays distinct short local tones and respects the setting', async () => {
    const user = userEvent.setup()
    renderFeedback()
    await user.click(screen.getByRole('button', { name: 'question' }))
    expect(oscillator.frequency.value).toBe(440)
    expect(oscillator.stop).toHaveBeenLastCalledWith(1.055)
    await user.click(screen.getByRole('button', { name: 'answer' }))
    expect(oscillator.frequency.value).toBe(554)
    await user.click(screen.getByRole('button', { name: 'disable' }))
    await user.click(screen.getByRole('button', { name: 'question' }))
    expect(oscillator.start).toHaveBeenCalledTimes(2)
  })

  it('does not create audio when reduced motion is requested', async () => {
    vi.stubGlobal('matchMedia', vi.fn(() => ({
      matches: true,
      addEventListener: vi.fn(),
      removeEventListener: vi.fn(),
    })))
    const user = userEvent.setup()
    renderFeedback()
    await user.click(screen.getByRole('button', { name: 'question' }))
    expect(globalThis.AudioContext).not.toHaveBeenCalled()
  })
})
