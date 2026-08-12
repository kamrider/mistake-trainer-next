import { describe, expect, it, vi } from 'vitest'
import { createRecoverySingleFlight } from './recovery-single-flight'

describe('createRecoverySingleFlight', () => {
  it('coalesces different recovery actions until the active action settles', async () => {
    let finish: ((value: boolean) => void) | undefined
    const firstOperation = vi.fn(() => new Promise<boolean>((resolve) => { finish = resolve }))
    const competingOperation = vi.fn(async () => true)
    const run = createRecoverySingleFlight()

    const first = run(firstOperation)
    const competing = run(competingOperation)

    expect(competing).toBe(first)
    expect(firstOperation).toHaveBeenCalledTimes(1)
    expect(competingOperation).not.toHaveBeenCalled()
    finish?.(true)
    await expect(first).resolves.toBe(true)

    await expect(run(competingOperation)).resolves.toBe(true)
    expect(competingOperation).toHaveBeenCalledTimes(1)
  })
})
