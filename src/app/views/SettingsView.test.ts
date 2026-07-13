import { render, screen } from '@testing-library/vue'
import { describe, expect, it, vi } from 'vitest'
import SettingsView from './SettingsView.vue'

const settingsOverview = vi.hoisted(() => vi.fn())
vi.mock('@tauri-apps/api/core', () => ({ isTauri: () => true }))
vi.mock('../../shared/api/bindings', () => ({ commands: { settingsOverview } }))

describe('SettingsView', () => {
  it('shows encrypted local state and honest cloud readiness', async () => {
    settingsOverview.mockResolvedValue({ ok: true, data: {
      activeProblemCount: 8, archivedProblemCount: 2, trashedProblemCount: 1,
      pendingOperationCount: 11, failedOperationCount: 0, unresolvedConflictCount: 0,
      localEncryptionReady: true, cloudSyncConfigured: false,
    } })
    render(SettingsView)

    expect(await screen.findByText('SQLCipher 已启用')).toBeVisible()
    expect(screen.getByText('8 道活动题')).toBeVisible()
    expect(screen.getByText('尚未配置')).toBeVisible()
    expect(screen.getByText(/本地 outbox 已记录 11 项变更/)).toBeVisible()
  })
})
