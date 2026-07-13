import { render, screen, waitFor } from '@testing-library/vue'
import userEvent from '@testing-library/user-event'
import { beforeEach, describe, expect, it, vi } from 'vitest'
import SettingsView from './SettingsView.vue'

const api = vi.hoisted(() => ({
  settingsOverview: vi.fn(),
  legacyScan: vi.fn(),
}))
vi.mock('@tauri-apps/api/core', () => ({ isTauri: () => true }))
vi.mock('../../shared/api/bindings', () => ({ commands: api }))

describe('SettingsView', () => {
  beforeEach(() => vi.clearAllMocks())

  it('shows encrypted local state and honest cloud readiness', async () => {
    api.settingsOverview.mockResolvedValue({ ok: true, data: {
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

  it('runs a read-only legacy preflight and renders actionable issues', async () => {
    api.settingsOverview.mockResolvedValue({ ok: true, data: {
      activeProblemCount: 0, archivedProblemCount: 0, trashedProblemCount: 0,
      pendingOperationCount: 0, failedOperationCount: 0, unresolvedConflictCount: 0,
      localEncryptionReady: true, cloudSyncConfigured: false,
    } })
    api.legacyScan.mockResolvedValue({ ok: true, data: {
      members: 2,
      metadataRecords: 8,
      existingAssets: 7,
      trainingRecords: 3,
      frozenRecords: 1,
      duplicateAssets: 1,
      truncated: false,
      issues: [{
        code: 'missing_asset',
        member: '小树',
        recordId: 'answer-1',
        detail: 'referenced image is missing',
      }],
    } })
    render(SettingsView)

    await userEvent.click(await screen.findByRole('button', { name: '选择旧版目录并扫描' }))

    await waitFor(() => expect(api.legacyScan).toHaveBeenCalledOnce())
    expect(await screen.findByText('图片缺失')).toBeVisible()
    expect(screen.getByText('referenced image is missing')).toBeVisible()
    expect(screen.getByText(/尚未写入新题库/)).toBeVisible()
    expect(screen.queryByText(/可以进入复制与校验阶段/)).not.toBeInTheDocument()
  })

  it('clears a stale preflight report when a later scan fails', async () => {
    api.settingsOverview.mockResolvedValue({ ok: true, data: {
      activeProblemCount: 0, archivedProblemCount: 0, trashedProblemCount: 0,
      pendingOperationCount: 0, failedOperationCount: 0, unresolvedConflictCount: 0,
      localEncryptionReady: true, cloudSyncConfigured: false,
    } })
    api.legacyScan
      .mockResolvedValueOnce({ ok: true, data: {
        members: 1, metadataRecords: 1, existingAssets: 0, trainingRecords: 0,
        frozenRecords: 0, duplicateAssets: 0, truncated: false,
        issues: [{ code: 'missing_asset', member: '小树', recordId: 'q-1', detail: 'referenced image is missing' }],
      } })
      .mockRejectedValueOnce(new Error('worker failed'))
    render(SettingsView)
    const scanButton = await screen.findByRole('button', { name: '选择旧版目录并扫描' })

    await userEvent.click(scanButton)
    expect(await screen.findByText('referenced image is missing')).toBeVisible()
    await userEvent.click(scanButton)

    expect(await screen.findByRole('alert')).toHaveTextContent('原目录未被修改')
    expect(screen.queryByText('referenced image is missing')).not.toBeInTheDocument()
  })
})
