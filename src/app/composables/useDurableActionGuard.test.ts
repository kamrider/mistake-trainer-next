import { render } from '@testing-library/vue'
import { defineComponent } from 'vue'
import { describe, expect, it, vi } from 'vitest'
import {
  type DurableActionAttempt,
  useDurableActionGuard,
} from './useDurableActionGuard'

function harness(register = true) {
  let busy = false
  let attempt: DurableActionAttempt | undefined
  let controller: ReturnType<typeof useDurableActionGuard> | undefined
  const onBlocked = vi.fn()
  const unregister = vi.fn()
  const registerContextTransition = vi.fn((candidate: DurableActionAttempt) => {
    attempt = candidate
    return unregister
  })
  const Host = defineComponent({
    setup() {
      controller = useDurableActionGuard({
        busy: () => busy,
        onBlocked,
        ...(register ? { registerContextTransition } : {}),
      })
      return () => null
    },
  })
  const view = render(Host)
  return {
    view,
    onBlocked,
    unregister,
    registerContextTransition,
    controller: () => controller!,
    registeredAttempt: () => attempt!,
    setBusy(value: boolean) { busy = value },
  }
}

describe('useDurableActionGuard', () => {
  it('allows idle transitions and blocks busy transitions through one registered attempt', () => {
    const current = harness()

    expect(current.registerContextTransition).toHaveBeenCalledOnce()
    expect(current.registeredAttempt()).toBe(current.controller().attemptLeave)
    expect(current.registeredAttempt()()).toBe(true)

    current.setBusy(true)
    expect(current.controller().attemptLeave()).toBe(false)
    expect(current.onBlocked).toHaveBeenCalledOnce()
  })

  it('prevents window unload only while busy', () => {
    const current = harness(false)
    expect(current.registerContextTransition).not.toHaveBeenCalled()

    const idleEvent = new Event('beforeunload', { cancelable: true })
    window.dispatchEvent(idleEvent)
    expect(idleEvent.defaultPrevented).toBe(false)

    current.setBusy(true)
    const busyEvent = new Event('beforeunload', { cancelable: true })
    window.dispatchEvent(busyEvent)
    expect(busyEvent.defaultPrevented).toBe(true)
  })

  it('unregisters the context attempt and removes the unload listener', () => {
    const current = harness()
    current.setBusy(true)
    current.view.unmount()

    expect(current.unregister).toHaveBeenCalledOnce()
    const afterUnmount = new Event('beforeunload', { cancelable: true })
    window.dispatchEvent(afterUnmount)
    expect(afterUnmount.defaultPrevented).toBe(false)
  })
})
