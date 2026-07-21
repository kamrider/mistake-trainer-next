import { describe, expect, it, vi } from 'vitest'
import { backendKindLabel, backendStatusLabel, loadSyncBackendStatus, setSyncBackend } from './sync-backend'

describe('sync backend API', () => {
  it('uses a deterministic local-only preview without invoking Tauri', async () => {
    const command = vi.fn()
    await expect(loadSyncBackendStatus(() => false, command)).resolves.toEqual({
      ok: true,
      data: { kind: 'local-only', configured: true, ready: true, syncEnabled: false },
    })
    expect(command).not.toHaveBeenCalled()
  })

  it('normalizes the typed status result in the desktop runtime', async () => {
    const command = vi.fn().mockResolvedValue({
      ok: true,
      data: { kind: 'supabase', configured: false, ready: false, syncEnabled: false },
    })
    await expect(loadSyncBackendStatus(() => true, command)).resolves.toEqual({
      ok: true,
      data: { kind: 'supabase', configured: false, ready: false, syncEnabled: false },
    })
    expect(command).toHaveBeenCalledOnce()
  })

  it('does not allow changing provider from browser preview', async () => {
    const command = vi.fn()
    const result = await setSyncBackend('tencent', () => false, command)
    expect(result.ok).toBe(false)
    if (!result.ok) expect(result.error.code).toBe('TAURI_REQUIRED')
    expect(command).not.toHaveBeenCalled()
  })

  it('maps provider labels and unavailable states for settings UI', () => {
    expect(backendKindLabel('local-only')).toContain('本地')
    expect(backendStatusLabel({ kind: 'supabase', configured: false, ready: false, syncEnabled: false })).toContain('尚未配置')
    expect(backendStatusLabel({ kind: 'tencent', configured: true, ready: false, syncEnabled: false })).toContain('等待适配器')
  })
})
