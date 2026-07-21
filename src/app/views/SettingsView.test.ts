import { render, screen, waitFor } from '@testing-library/vue'
import userEvent from '@testing-library/user-event'
import { beforeEach, describe, expect, it, vi } from 'vitest'
import SettingsView from './SettingsView.vue'

const api = vi.hoisted(() => ({
  settingsOverview: vi.fn(),
  syncBackendStatus: vi.fn(),
  syncBackendSet: vi.fn(),
  legacyScan: vi.fn(),
  legacyImport: vi.fn(),
  legacyImportList: vi.fn(),
  legacyRollback: vi.fn(),
  backupCreate: vi.fn(),
  backupPrepareRestore: vi.fn(),
  backupRestore: vi.fn(),
  subjectPreferencesGet: vi.fn(),
  subjectPreferencesSave: vi.fn(),
  reviewPreferencesGet: vi.fn(),
  reviewPreferencesSave: vi.fn(),
}))
vi.mock('@tauri-apps/api/core', () => ({ isTauri: () => true }))
vi.mock('../../shared/api/bindings', () => ({ commands: api }))

describe('SettingsView', () => {
  beforeEach(() => {
    vi.clearAllMocks()
    api.legacyImportList.mockResolvedValue({ ok: true, data: [] })
    api.syncBackendStatus.mockResolvedValue({ ok: true, data: {
      kind: 'local-only', configured: true, ready: true, syncEnabled: false,
    } })
    api.syncBackendSet.mockResolvedValue({ ok: true, data: {
      kind: 'local-only', configured: true, ready: true, syncEnabled: false,
    } })
    api.subjectPreferencesGet.mockResolvedValue({ ok: true, data: {
      enabledSubjects: ['语文', '数学', '英语'],
      customSubjects: [],
      captureSoundEnabled: true,
    } })
    api.reviewPreferencesGet.mockResolvedValue({ ok: true, data: { focusPolicy: 'off' } })
    api.reviewPreferencesSave.mockResolvedValue({ ok: true, data: { focusPolicy: 'every_10' } })
  })

  it('configures builtin and custom subjects plus capture sound', async () => {
    api.settingsOverview.mockResolvedValue({ ok: true, data: {
      activeProblemCount: 0, archivedProblemCount: 0, trashedProblemCount: 0,
      pendingOperationCount: 0, failedOperationCount: 0, unresolvedConflictCount: 0,
      localEncryptionReady: true, cloudSyncConfigured: false,
    } })
    api.subjectPreferencesSave.mockResolvedValue({ ok: true, data: {
      enabledSubjects: ['语文', '数学', '编程'],
      customSubjects: ['编程'],
      captureSoundEnabled: false,
    } })
    render(SettingsView)

    expect(await screen.findByRole('heading', { name: '常用科目' })).toBeVisible()
    await userEvent.click(screen.getByRole('checkbox', { name: '英语' }))
    await userEvent.type(screen.getByPlaceholderText('例如：编程、竞赛数学'), '编程')
    await userEvent.click(screen.getByRole('button', { name: '添加自定义科目' }))
    await userEvent.click(screen.getByRole('checkbox', { name: /拖放成功音效/ }))
    await userEvent.click(screen.getByRole('button', { name: '保存科目配置' }))

    expect(api.subjectPreferencesSave).toHaveBeenCalledWith({
      enabledSubjects: ['语文', '数学', '编程'],
      customSubjects: ['编程'],
      captureSoundEnabled: false,
    })
    expect(await screen.findByText('科目配置已保存')).toBeVisible()
  })

  it('configures a skippable focus rhythm for new ordinary sessions', async () => {
    api.settingsOverview.mockResolvedValue({ ok: true, data: {
      activeProblemCount: 0, archivedProblemCount: 0, trashedProblemCount: 0,
      pendingOperationCount: 0, failedOperationCount: 0, unresolvedConflictCount: 0,
      localEncryptionReady: true, cloudSyncConfigured: false,
    } })
    render(SettingsView)

    expect(await screen.findByRole('heading', { name: '训练间的专注插曲' })).toBeVisible()
    await userEvent.click(screen.getByRole('radio', { name: /每完成 10 题/ }))
    await userEvent.click(screen.getByRole('button', { name: '保存训练节奏' }))

    expect(api.reviewPreferencesSave).toHaveBeenCalledWith({ focusPolicy: 'every_10' })
    expect(await screen.findByText('训练节奏已保存，将从下一轮普通训练开始生效。')).toBeVisible()
    expect(screen.getByText(/模拟考试不会插入专注环节/)).toBeVisible()
    expect(screen.getByRole('radio', { name: /每轮开始前 · 推荐/ })).toBeInTheDocument()
  })

  it('keeps the selected focus rhythm visible when persistence fails', async () => {
    api.settingsOverview.mockResolvedValue({ ok: true, data: {
      activeProblemCount: 0, archivedProblemCount: 0, trashedProblemCount: 0,
      pendingOperationCount: 0, failedOperationCount: 0, unresolvedConflictCount: 0,
      localEncryptionReady: true, cloudSyncConfigured: false,
    } })
    api.reviewPreferencesSave.mockResolvedValue({ ok: false, error: {
      code: 'review_preferences_save_failed', userMessage: '训练节奏没有保存。',
      retryable: true, diagnosticId: 'diag-focus',
    } })
    render(SettingsView)

    const everyTen = await screen.findByRole('radio', { name: /每完成 10 题/ })
    await userEvent.click(everyTen)
    await userEvent.click(screen.getByRole('button', { name: '保存训练节奏' }))

    expect(await screen.findByText('训练节奏没有保存。')).toBeVisible()
    expect(everyTen).toBeChecked()
  })

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

  it('keeps local mode selected when a remote backend is not configured', async () => {
    api.syncBackendSet.mockResolvedValue({ ok: false, error: {
      code: 'SYNC_BACKEND_NOT_CONFIGURED',
      userMessage: '该同步服务尚未配置，已保持本地模式',
      retryable: false,
      diagnosticId: 'sync-backend-selection',
    } })
    render(SettingsView)

    expect(await screen.findByText('本地优先 · 不需要网络')).toBeVisible()
    await userEvent.click(screen.getByRole('button', { name: /^Supabase/ }))
    expect(api.syncBackendSet).toHaveBeenCalledWith({ kind: 'supabase' })
    expect(await screen.findByText('该同步服务尚未配置，已保持本地模式')).toBeVisible()
    expect(screen.getByRole('button', { name: /^仅本地/ })).toHaveClass('selected')
  })

  it('runs a read-only legacy preflight and renders actionable issues', async () => {
    api.settingsOverview.mockResolvedValue({ ok: true, data: {
      activeProblemCount: 0, archivedProblemCount: 0, trashedProblemCount: 0,
      pendingOperationCount: 0, failedOperationCount: 0, unresolvedConflictCount: 0,
      localEncryptionReady: true, cloudSyncConfigured: false,
    } })
    api.legacyScan.mockResolvedValue({ ok: true, data: {
      candidateId: 'candidate-one', problemCount: 3, expiresAtUtcMs: Date.now() + 30 * 60_000,
      report: {
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
      },
    } })
    render(SettingsView)

    await userEvent.click(await screen.findByRole('button', { name: '选择旧版目录并扫描' }))

    await waitFor(() => expect(api.legacyScan).toHaveBeenCalledOnce())
    expect(await screen.findByText('图片缺失')).toBeVisible()
    expect(screen.getByText('referenced image is missing')).toBeVisible()
    expect(screen.getByText(/尚未写入新题库/)).toBeVisible()
    expect(screen.getByRole('button', { name: '查看范围并确认导入' })).toBeEnabled()
  })

  it('clears a stale preflight report when a later scan fails', async () => {
    api.settingsOverview.mockResolvedValue({ ok: true, data: {
      activeProblemCount: 0, archivedProblemCount: 0, trashedProblemCount: 0,
      pendingOperationCount: 0, failedOperationCount: 0, unresolvedConflictCount: 0,
      localEncryptionReady: true, cloudSyncConfigured: false,
    } })
    api.legacyScan
      .mockResolvedValueOnce({ ok: true, data: {
        candidateId: 'candidate-stale', problemCount: 1, expiresAtUtcMs: Date.now() + 30 * 60_000,
        report: {
          members: 1, metadataRecords: 1, existingAssets: 0, trainingRecords: 0,
          frozenRecords: 0, duplicateAssets: 0, truncated: false,
          issues: [{ code: 'missing_asset', member: '小树', recordId: 'q-1', detail: 'referenced image is missing' }],
        },
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

  it('prepares an encrypted restore and requires explicit confirmation before restart', async () => {
    api.settingsOverview.mockResolvedValue({ ok: true, data: {
      activeProblemCount: 1, archivedProblemCount: 0, trashedProblemCount: 0,
      pendingOperationCount: 0, failedOperationCount: 0, unresolvedConflictCount: 0,
      localEncryptionReady: true, cloudSyncConfigured: false,
    } })
    api.backupCreate.mockResolvedValue({ status: 'ok', data: { ok: true, data: {
      formatVersion: 1, createdAtUtcMs: 1_725_000_000_000, assetCount: 4,
      encryptedBytes: 2_097_152, label: 'mistake-trainer-backup-safe', readyForRestore: false,
    } } })
    api.backupPrepareRestore.mockResolvedValue({ status: 'ok', data: { ok: true, data: {
      id: 'candidate-opaque-id', expiresAtUtcMs: 1_725_086_400_000,
      summary: { formatVersion: 1, createdAtUtcMs: 1_725_000_000_000, assetCount: 4,
        encryptedBytes: 2_097_152, label: 'mistake-trainer-backup-safe', readyForRestore: true },
    } } })
    api.backupRestore.mockResolvedValue({ status: 'ok', data: { ok: true, data: true } })
    render(SettingsView)

    await userEvent.click(await screen.findByRole('button', { name: /创建加密备份/ }))
    expect(await screen.findByText('加密备份已创建')).toBeVisible()
    expect(screen.getByText(/4 个资源 · 2.0 MB/)).toBeVisible()

    await userEvent.click(screen.getByRole('button', { name: /选择备份并准备恢复/ }))
    expect(await screen.findByText('安全恢复包已就绪')).toBeVisible()
    expect(screen.getByText(/已复制到隔离区并再次校验，当前资料库尚未改变/)).toBeVisible()
    expect(screen.queryByText('已恢复')).not.toBeInTheDocument()

    const restoreTrigger = screen.getByRole('button', { name: '查看风险并确认恢复' })
    await userEvent.click(restoreTrigger)
    expect(screen.getByRole('button', { name: '取消，保持现状' })).toHaveFocus()
    await userEvent.keyboard('{Escape}')
    await waitFor(() => expect(screen.queryByRole('dialog')).not.toBeInTheDocument())
    expect(restoreTrigger).toHaveFocus()

    await userEvent.click(restoreTrigger)
    const confirm = screen.getByRole('button', { name: /确认恢复并重启/ })
    expect(confirm).toBeDisabled()
    await userEvent.click(screen.getByRole('checkbox', { name: /我明白：确认后当前题库/ }))
    await userEvent.click(confirm)

    expect(api.backupRestore).toHaveBeenCalledWith('candidate-opaque-id')
    expect(await screen.findByText('正在准备重启…')).toBeVisible()
  })

  it('clears stale backup validation when a later package fails integrity checks', async () => {
    api.settingsOverview.mockResolvedValue({ ok: true, data: {
      activeProblemCount: 0, archivedProblemCount: 0, trashedProblemCount: 0,
      pendingOperationCount: 0, failedOperationCount: 0, unresolvedConflictCount: 0,
      localEncryptionReady: true, cloudSyncConfigured: false,
    } })
    api.backupPrepareRestore
      .mockResolvedValueOnce({ status: 'ok', data: { ok: true, data: {
        id: 'candidate-one', expiresAtUtcMs: 86_400_001,
        summary: { formatVersion: 1, createdAtUtcMs: 1, assetCount: 1, encryptedBytes: 1024,
          label: 'valid-package', readyForRestore: true },
      } } })
      .mockResolvedValueOnce({ status: 'ok', data: { ok: false, error: {
        code: 'backup_prepare_restore_failed', userMessage: '备份包不完整或校验失败，未对现有资料库做任何修改。',
        retryable: false, diagnosticId: 'diagnostic-1',
      } } })
    render(SettingsView)
    const validateButton = await screen.findByRole('button', { name: /选择备份并准备恢复/ })

    await userEvent.click(validateButton)
    expect(await screen.findByText('valid-package')).toBeVisible()
    await userEvent.click(validateButton)

    expect(await screen.findByRole('alert')).toHaveTextContent('未对现有资料库做任何修改')
    expect(screen.queryByText('valid-package')).not.toBeInTheDocument()
  })
})
