import { onBeforeUnmount, ref, toValue, type MaybeRefOrGetter } from 'vue'

export type CaptureFeedbackRole = 'question' | 'answer'

export function useCaptureFeedback(soundEnabled: MaybeRefOrGetter<boolean>) {
  const media = typeof window !== 'undefined' && typeof window.matchMedia === 'function'
    ? window.matchMedia('(prefers-reduced-motion: reduce)')
    : undefined
  const reducedMotion = ref(media?.matches ?? false)
  let audioContext: AudioContext | undefined

  function updateMotion(event: MediaQueryListEvent) {
    reducedMotion.value = event.matches
  }

  media?.addEventListener?.('change', updateMotion)

  function playDrop(role: CaptureFeedbackRole) {
    if (!toValue(soundEnabled) || reducedMotion.value) return
    const AudioContextConstructor = globalThis.AudioContext
    if (!AudioContextConstructor) return
    try {
      audioContext ??= new AudioContextConstructor()
      const oscillator = audioContext.createOscillator()
      const gain = audioContext.createGain()
      const start = audioContext.currentTime
      const end = start + 0.055
      oscillator.type = 'sine'
      oscillator.frequency.value = role === 'question' ? 440 : 554
      gain.gain.setValueAtTime(0.025, start)
      gain.gain.exponentialRampToValueAtTime(0.0001, end)
      oscillator.connect(gain)
      gain.connect(audioContext.destination)
      oscillator.start(start)
      oscillator.stop(end)
    }
    catch {
      // Audio feedback is optional and must never interrupt a saved capture action.
    }
  }

  onBeforeUnmount(() => {
    media?.removeEventListener?.('change', updateMotion)
    try {
      const closing = audioContext?.close()
      if (closing) void closing.catch(() => undefined)
    }
    catch {
      // Optional audio cleanup cannot block view teardown.
    }
  })

  return { reducedMotion, playDrop }
}
