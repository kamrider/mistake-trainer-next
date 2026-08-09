import { describe, expect, it, vi } from 'vitest'
import { createWorkspaceTransitionGuard } from './workspace-transition-guard'

function deferred<T>() {
  let resolve!: (value: T) => void
  const promise = new Promise<T>((finish) => {
    resolve = finish
  })
  return { promise, resolve }
}

describe('createWorkspaceTransitionGuard', () => {
  it('allows an empty registry and evaluates a stable registration snapshot in order', async () => {
    const guard = createWorkspaceTransitionGuard()
    await expect(guard.attempt()).resolves.toBe(true)

    const order: string[] = []
    const unregisterFirst = guard.register(() => {
      order.push('first')
      return false
    })
    const second = vi.fn(() => {
      order.push('second')
      return true
    })
    const unregisterSecond = guard.register(second)

    await expect(guard.attempt()).resolves.toBe(false)
    expect(order).toEqual(['first'])
    expect(second).not.toHaveBeenCalled()

    unregisterFirst()
    await expect(guard.attempt()).resolves.toBe(true)
    expect(order).toEqual(['first', 'second'])
    unregisterSecond()
    await expect(guard.attempt()).resolves.toBe(true)
  })

  it('shares one aggregate decision across concurrent attempts and resets after settlement', async () => {
    const gate = deferred<boolean>()
    const decision = vi.fn(() => gate.promise)
    const guard = createWorkspaceTransitionGuard()
    guard.register(decision)

    const first = guard.attempt()
    const duplicate = guard.attempt()
    expect(first).toBe(duplicate)
    expect(decision).toHaveBeenCalledOnce()

    gate.resolve(true)
    await expect(Promise.all([first, duplicate])).resolves.toEqual([true, true])
    await expect(guard.attempt()).resolves.toBe(true)
    expect(decision).toHaveBeenCalledTimes(2)
  })

  it('uses the attempt snapshot even when a registration is removed while pending', async () => {
    const gate = deferred<boolean>()
    const guard = createWorkspaceTransitionGuard()
    const unregister = guard.register(() => gate.promise)
    const after = vi.fn(() => true)
    guard.register(after)

    const pending = guard.attempt()
    unregister()
    gate.resolve(true)

    await expect(pending).resolves.toBe(true)
    expect(after).toHaveBeenCalledOnce()
    await expect(guard.attempt()).resolves.toBe(true)
    expect(after).toHaveBeenCalledTimes(2)
  })
})
