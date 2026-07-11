import { describe, expect, it, vi } from 'vitest'
import { loadSystemStatus } from './system-status'

describe('loadSystemStatus', () => {
  it('does not invoke Tauri while rendering the browser design preview', async () => {
    const invoke = vi.fn()

    await expect(loadSystemStatus(() => false, invoke)).resolves.toEqual({
      ok: true,
      data: { appVersion: 'web-preview', storage: 'preview', sync: 'offline' },
    })
    expect(invoke).not.toHaveBeenCalled()
  })

  it('normalizes the generated command result inside Tauri', async () => {
    const invoke = vi.fn().mockResolvedValue({
      ok: true,
      data: { appVersion: '0.1.0', storage: 'locked', sync: 'offline' },
    })

    await expect(loadSystemStatus(() => true, invoke)).resolves.toEqual({
      ok: true,
      data: { appVersion: '0.1.0', storage: 'locked', sync: 'offline' },
    })
    expect(invoke).toHaveBeenCalledOnce()
  })
})
