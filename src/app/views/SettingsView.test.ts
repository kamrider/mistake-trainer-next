import { fireEvent, render, screen, waitFor, within } from '@testing-library/vue'
import userEvent from '@testing-library/user-event'
import { beforeEach, describe, expect, it, vi } from 'vitest'
import { libraryAccessControllerKey } from '../library-access-controller'
import { syncControllerKey } from '../sync-controller'
import SettingsView from './SettingsView.vue'

const api = vi.hoisted(() => ({
  settingsOverview: vi.fn(),
  syncBackendStatus: vi.fn(),
  syncBackendSet: vi.fn(),
  authStatusCommand: vi.fn(),
  authDisconnect: vi.fn(),
  libraryAccessStatus: vi.fn(),
  libraryLock: vi.fn(),
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
  storageStatus: vi.fn(),
  storageMigrateSelect: vi.fn(),
  storageMigrationReceipt: vi.fn(),
  diagnosticsExport: vi.fn(),
  compatibilityStatus: vi.fn(),
  ocrCapabilityStatus: vi.fn(),
  ocrComponentInstall: vi.fn(),
  ocrComponentRemove: vi.fn(),
  syncConflictList: vi.fn(),
  syncConflictResolve: vi.fn(),
  syncConflictResolveEntity: vi.fn(),
  syncNow: vi.fn(),
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
    api.authStatusCommand.mockResolvedValue({ ok: true, data: {
      configured: false, status: { kind: 'unconfigured', emailHint: null },
    } })
    api.authDisconnect.mockResolvedValue({ status: 'ok', data: { ok: true, data: {
      configured: true, status: { kind: 'signed_out', emailHint: null },
    } } })
    api.libraryAccessStatus.mockResolvedValue({
      ok: true,
      data: { locked: false, trustedWindowsAccount: true },
    })
    api.libraryLock.mockResolvedValue({
      ok: true,
      data: { locked: true, trustedWindowsAccount: true },
    })
    api.subjectPreferencesGet.mockResolvedValue({ ok: true, data: {
      enabledSubjects: ['语文', '数学', '英语'],
      customSubjects: [],
      captureSoundEnabled: true,
    } })
    api.reviewPreferencesGet.mockResolvedValue({ ok: true, data: { focusPolicy: 'off' } })
    api.reviewPreferencesSave.mockResolvedValue({ ok: true, data: { focusPolicy: 'every_10' } })
    api.storageStatus.mockResolvedValue({ status: 'ok', data: { ok: true, data: {
      kind: 'default',
      locationLabel: '默认位置 · Windows 应用数据',
      databaseBytes: 4096,
      assetBytes: 8192,
      migrationPending: false,
    } } })
    api.storageMigrateSelect.mockResolvedValue({ status: 'ok', data: { ok: true, data: null } })
    api.storageMigrationReceipt.mockResolvedValue({ ok: true, data: null })
    api.diagnosticsExport.mockResolvedValue({ status: 'ok', data: { ok: true, data: null } })
    api.compatibilityStatus.mockResolvedValue({ status: 'ok', data: { ok: true, data: {
      supportLevel: 'supported',
      supported: true,
      osName: 'Windows 11 Pro',
      displayVersion: '24H2',
      buildNumber: 26100,
      updateBuildRevision: 1000,
      processArchitecture: 'x86_64',
      nativeArchitecture: 'x86_64',
      webview2Version: '138.0.3351.83',
      minimumWindowsBuild: 17763,
      summary: '当前设备处于完整支持范围。',
    } } })
    api.ocrCapabilityStatus.mockResolvedValue({ status: 'ok', data: { ok: true, data: {
      assessment: {
        tier: 'balanced',
        logicalProcessorCount: 4,
        totalMemoryMb: 8192,
        availableComponentStorageMb: 8192,
        avx2Supported: true,
        estimatedSuitable: true,
        recommendedComponentId: 'ppocrv6_small',
        summary: '本机预检通过，推荐使用 PP‑OCRv6 small。',
      },
      components: [{
        id: 'ppocrv6_small',
        displayName: 'PP‑OCRv6 small',
        description: '约 31 MB，面向题号与文字框检测。',
        state: 'not_installed',
        downloadBytes: 31_163_977,
        installedBytes: 0,
        recommended: true,
        installAllowed: true,
        statusDetail: '尚未下载；不会影响现有功能。',
        sourceLabel: 'ModelScope · RapidAI/RapidOCR 3.9.2',
        licenseLabel: 'PaddleOCR · Apache-2.0',
      }, {
        id: 'opencv_preprocess',
        displayName: 'OpenCV 图像预处理',
        description: '产品运行时尚未发布。',
        state: 'unavailable',
        downloadBytes: 0,
        installedBytes: 0,
        recommended: false,
        installAllowed: false,
        statusDetail: '产品运行时尚未发布；当前不会下载或启用。',
        sourceLabel: 'OpenCV 官方项目',
        licenseLabel: 'Apache-2.0',
      }],
      recognitionFeature: {
        state: 'evidence_gate_pending',
        requiredComponentId: 'ppocrv6_small',
        detail: '智能分题仍在真实题图验证中；顺序模板和手工整理可继续使用。',
      },
      automaticRecognitionEnabled: false,
    } } })
    api.ocrComponentInstall.mockResolvedValue({ status: 'ok', data: { ok: false, error: {
      code: 'not_expected',
      userMessage: 'not expected',
      retryable: false,
      diagnosticId: 'test',
    } } })
    api.ocrComponentRemove.mockResolvedValue({ status: 'ok', data: { ok: false, error: {
      code: 'not_expected',
      userMessage: 'not expected',
      retryable: false,
      diagnosticId: 'test',
    } } })
    api.syncConflictList.mockResolvedValue({ ok: true, data: [] })
    api.syncConflictResolve.mockResolvedValue({ ok: true, data: [] })
    api.syncConflictResolveEntity.mockResolvedValue({ ok: true, data: [] })
    api.syncNow.mockResolvedValue({
      status: 'ok',
      data: {
        ok: true,
        data: {
          pushedOperationCount: 0,
          uploadedAssetCount: 0,
          pulledChangeCount: 0,
          downloadedAssetCount: 0,
          finalCursor: 0,
        },
      },
    })
  })

  it('shows the detected Windows build, architecture, and WebView2 runtime', async () => {
    api.settingsOverview.mockResolvedValue({ ok: true, data: {
      activeProblemCount: 0, archivedProblemCount: 0, trashedProblemCount: 0,
      pendingOperationCount: 0, failedOperationCount: 0, unresolvedConflictCount: 0,
      localEncryptionReady: true, cloudSyncConfigured: false,
    } })

    render(SettingsView)

    const card = await screen.findByRole('article', { name: 'Windows 兼容性' })
    expect(card).toHaveTextContent('完整支持')
    expect(card).toHaveTextContent('Windows 11 Pro')
    expect(card).toHaveTextContent('Build 26100.1000')
    expect(card).toHaveTextContent('x86_64')
    expect(card).toHaveTextContent('WebView2 138.0.3351.83')
    expect(api.compatibilityStatus).toHaveBeenCalledOnce()
  })

  it('keeps the two smart modes clear and reachable from the settings directory', async () => {
    api.settingsOverview.mockResolvedValue({ ok: true, data: {
      activeProblemCount: 0, archivedProblemCount: 0, trashedProblemCount: 0,
      pendingOperationCount: 0, failedOperationCount: 0, unresolvedConflictCount: 0,
      localEncryptionReady: true, cloudSyncConfigured: false,
    } })
    render(SettingsView)

    const panel = await screen.findByRole('region', { name: '智能功能模式' })
    expect(panel).toHaveAttribute('id', 'settings-ocr')
    expect(within(panel).getByRole('article', { name: '智能切图（已开放）' })).toBeVisible()
    expect(within(panel).getByRole('article', { name: '全自动识题（未开放）' })).toBeVisible()
    expect(within(panel).queryByRole('button')).not.toBeInTheDocument()
    expect(api.ocrComponentInstall).not.toHaveBeenCalled()

    const scrollIntoView = vi.fn()
    panel.scrollIntoView = scrollIntoView
    const directory = screen.getByRole('navigation', { name: '设置目录' })
    await userEvent.click(within(directory).getByRole('button', { name: /智能功能/ }))
    expect(scrollIntoView).toHaveBeenCalledWith({
      block: 'start',
      behavior: 'smooth',
    })
  })

  it('delegates manual sync to the global controller without a duplicate native call', async () => {
    api.settingsOverview.mockResolvedValue({ ok: true, data: {
      activeProblemCount: 1, archivedProblemCount: 0, trashedProblemCount: 0,
      pendingOperationCount: 1, failedOperationCount: 0, unresolvedConflictCount: 0,
      localEncryptionReady: true, cloudSyncConfigured: true,
    } })
    api.authStatusCommand.mockResolvedValue({ ok: true, data: {
      configured: true, status: { kind: 'connected', emailHint: 's***@example.test' },
    } })
    const run = vi.fn().mockResolvedValue({
      ok: true,
      data: {
        pushedOperationCount: 1,
        uploadedAssetCount: 0,
        pulledChangeCount: 2,
        downloadedAssetCount: 0,
        finalCursor: 3,
      },
    })
    render(SettingsView, {
      global: {
        provide: {
          [syncControllerKey as symbol]: { run },
        },
      },
    })

    await userEvent.click(await screen.findByRole('button', { name: '立即同步' }))

    expect(run).toHaveBeenCalledWith('manual')
    expect(api.syncNow).not.toHaveBeenCalled()
    expect(await screen.findByText('同步完成：上传 1 项，拉取 2 项。')).toBeVisible()
  })

  it('places real unresolved sync choices below the library overview', async () => {
    api.settingsOverview.mockResolvedValue({ ok: true, data: {
      activeProblemCount: 1, archivedProblemCount: 0, trashedProblemCount: 0,
      pendingOperationCount: 0, failedOperationCount: 0, unresolvedConflictCount: 1,
      localEncryptionReady: true, cloudSyncConfigured: true,
    } })
    api.syncConflictList.mockResolvedValue({ ok: true, data: [{
      id: 'conflict-note',
      entityType: 'problem',
      entityId: 'opaque-problem',
      entityLabel: '数学',
      fieldName: 'note',
      localValue: { kind: 'string', value: '本机笔记' },
      remoteValue: { kind: 'string', value: '云端笔记' },
      createdAtUtcMs: 1_725_000_000_000,
    }] })
    render(SettingsView)

    expect(await screen.findByRole('heading', { name: '本机和云端改了同一处内容' })).toBeVisible()
    expect(screen.getByText('本机笔记')).toBeVisible()
    expect(screen.getByText('云端笔记')).toBeVisible()
    expect(screen.queryByText('opaque-problem')).not.toBeInTheDocument()
    expect(screen.queryByText('只呈现同字段真冲突；不同字段自动合并。')).not.toBeInTheDocument()
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

  it('jumps directly to frequently used subject settings from the sticky directory', async () => {
    api.settingsOverview.mockResolvedValue({ ok: true, data: {
      activeProblemCount: 0, archivedProblemCount: 0, trashedProblemCount: 0,
      pendingOperationCount: 0, failedOperationCount: 0, unresolvedConflictCount: 0,
      localEncryptionReady: true, cloudSyncConfigured: false,
    } })
    render(SettingsView)

    expect(await screen.findByRole('heading', { name: '常用科目' })).toBeVisible()
    const subjectPanel = screen.getByRole('region', { name: '常用科目' })
    const scrollIntoView = vi.fn()
    subjectPanel.scrollIntoView = scrollIntoView
    const directory = screen.getByRole('navigation', { name: '设置目录' })
    await userEvent.click(within(directory).getByRole('button', { name: /科目配置/ }))

    expect(subjectPanel).toHaveAttribute('id', 'settings-subjects')
    expect(scrollIntoView).toHaveBeenCalledWith({
      block: 'start',
      behavior: 'smooth',
    })
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

    expect(await screen.findByText('SQLCipher 与原图独立加密')).toBeVisible()
    expect(screen.getByText('8 道活动题')).toBeVisible()
    expect(screen.getByText('尚未配置')).toBeVisible()
    expect(screen.getByText(/待同步变更 11 项/)).toBeVisible()
    expect(screen.getByRole('heading', { name: '这台 Windows 电脑' })).toBeVisible()
    expect(await screen.findByText('当前 Windows 账户可解锁')).toBeVisible()
    expect(screen.getByText('已接通 · 当前设备保护')).toBeVisible()
  })

  it('confirms an immediate local lock and restores focus when cancelled', async () => {
    api.settingsOverview.mockResolvedValue({ ok: true, data: {
      activeProblemCount: 8, archivedProblemCount: 2, trashedProblemCount: 1,
      pendingOperationCount: 0, failedOperationCount: 0, unresolvedConflictCount: 0,
      localEncryptionReady: true, cloudSyncConfigured: false,
    } })
    const user = userEvent.setup()
    const enterRestarting = vi.fn()
    render(SettingsView, {
      global: {
        provide: {
          [libraryAccessControllerKey as symbol]: { enterRestarting },
        },
      },
    })

    const trigger = await screen.findByRole('button', { name: '立即锁定资料库' })
    await user.click(trigger)
    expect(screen.getByRole('heading', { name: '现在锁定本地资料库？' })).toBeVisible()
    expect(screen.getByRole('button', { name: '取消，继续使用' })).toHaveFocus()

    await user.keyboard('{Escape}')
    await waitFor(() => expect(screen.queryByRole('dialog')).not.toBeInTheDocument())
    expect(trigger).toHaveFocus()

    await user.click(trigger)
    await user.click(screen.getByRole('button', { name: '立即锁定' }))

    expect(api.libraryLock).toHaveBeenCalledOnce()
    expect(enterRestarting).toHaveBeenCalledOnce()
    expect(await screen.findByRole('button', { name: '正在锁定…' })).toBeDisabled()
  })

  it('clears cloud credentials before locking a connected library', async () => {
    api.settingsOverview.mockResolvedValue({ ok: true, data: {
      activeProblemCount: 1, archivedProblemCount: 0, trashedProblemCount: 0,
      pendingOperationCount: 0, failedOperationCount: 0, unresolvedConflictCount: 0,
      localEncryptionReady: true, cloudSyncConfigured: true,
    } })
    api.authStatusCommand.mockResolvedValue({ ok: true, data: {
      configured: true, status: { kind: 'connected', emailHint: 'u***@example.com' },
    } })
    const user = userEvent.setup()
    render(SettingsView)

    await user.click(await screen.findByRole('button', { name: '退出云端并锁定' }))
    const dialog = screen.getByRole('dialog')
    expect(within(dialog).getByText(/只会退出这台电脑的云端会话/)).toBeVisible()
    expect(within(dialog).getByText(/其他设备保持登录/)).toBeVisible()
    await user.click(screen.getByRole('button', { name: '退出并锁定' }))

    await waitFor(() => {
      expect(api.authDisconnect).toHaveBeenCalledOnce()
      expect(api.libraryLock).toHaveBeenCalledOnce()
    })
    expect(api.authDisconnect.mock.invocationCallOrder[0]!).toBeLessThan(api.libraryLock.mock.invocationCallOrder[0]!)
  })

  it('explains regional cloud limits while keeping local-first recovery visible', async () => {
    api.settingsOverview.mockResolvedValue({ ok: true, data: {
      activeProblemCount: 2, archivedProblemCount: 0, trashedProblemCount: 0,
      pendingOperationCount: 3, failedOperationCount: 0, unresolvedConflictCount: 0,
      localEncryptionReady: true, cloudSyncConfigured: true,
    } })
    api.syncBackendStatus.mockResolvedValue({ ok: true, data: {
      kind: 'supabase', configured: true, ready: true, syncEnabled: true,
    } })
    api.authStatusCommand.mockResolvedValue({ ok: true, data: {
      configured: true, status: { kind: 'offline', emailHint: 'u***@example.com' },
    } })
    render(SettingsView)

    expect(await screen.findByText('国内网络提示')).toBeVisible()
    expect(screen.getByText(/Supabase 在中国大陆可能出现连接超时/)).toBeVisible()
    expect(screen.getAllByText('离线模式')).toHaveLength(2)
    expect(await screen.findByText('退出云端只影响这台电脑，其他设备保持登录。')).toBeVisible()
  })

  it('does not invent offline unlock readiness when the device status cannot be read', async () => {
    api.settingsOverview.mockResolvedValue({ ok: true, data: {
      activeProblemCount: 2, archivedProblemCount: 0, trashedProblemCount: 0,
      pendingOperationCount: 0, failedOperationCount: 0, unresolvedConflictCount: 0,
      localEncryptionReady: true, cloudSyncConfigured: false,
    } })
    api.libraryAccessStatus.mockResolvedValue({
      ok: false,
      error: {
        code: 'LIBRARY_ACCESS_UNAVAILABLE',
        userMessage: '无法读取 Windows 资料库凭据，已保持锁定；请检查系统凭据服务后重试。',
        retryable: true,
        diagnosticId: 'device-status',
      },
    })
    render(SettingsView)

    expect(await screen.findByText('状态暂不可用')).toBeVisible()
    expect(await screen.findByRole('status', { name: '当前设备保护状态' })).toHaveTextContent('无法读取 Windows 资料库凭据')
    expect(screen.queryByText('当前 Windows 账户可解锁')).not.toBeInTheDocument()
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

  it('marks the unavailable Tencent backend as planned and never selects it', async () => {
    render(SettingsView)

    const tencent = await screen.findByRole('button', { name: /腾讯云.*规划中/ })
    expect(tencent).toBeDisabled()
    expect(screen.getByText('规划中')).toBeVisible()
    await fireEvent.click(tencent)
    expect(api.syncBackendSet).not.toHaveBeenCalledWith({ kind: 'tencent' })
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

  it('shows bounded storage capacity without exposing an absolute path', async () => {
    api.settingsOverview.mockResolvedValue({ ok: true, data: {
      activeProblemCount: 0, archivedProblemCount: 0, trashedProblemCount: 0,
      pendingOperationCount: 0, failedOperationCount: 0, unresolvedConflictCount: 0,
      localEncryptionReady: true, cloudSyncConfigured: false,
    } })
    api.storageStatus.mockResolvedValue({ status: 'ok', data: { ok: true, data: {
      kind: 'custom',
      locationLabel: '自定义位置 · StudyDisk',
      databaseBytes: 4096,
      assetBytes: 8192,
      migrationPending: false,
    } } })
    render(SettingsView)

    expect(await screen.findByRole('heading', { name: '资料库存储位置' })).toBeVisible()
    expect(await screen.findByText('自定义位置 · StudyDisk')).toBeVisible()
    const storage = screen.getByRole('region', { name: '资料库存储位置' })
    expect(within(storage).getByText('4.0 KB')).toBeVisible()
    expect(within(storage).getByText('8.0 KB')).toBeVisible()
    expect(within(storage).getByText('12.0 KB')).toBeVisible()
    expect(storage).not.toHaveTextContent(/C:\\|Users\\|Lytree/)
  })

  it('cancels native folder selection without reporting a failure and restores focus', async () => {
    api.settingsOverview.mockResolvedValue({ ok: true, data: {
      activeProblemCount: 0, archivedProblemCount: 0, trashedProblemCount: 0,
      pendingOperationCount: 0, failedOperationCount: 0, unresolvedConflictCount: 0,
      localEncryptionReady: true, cloudSyncConfigured: false,
    } })
    const user = userEvent.setup()
    render(SettingsView)

    const trigger = await screen.findByRole('button', { name: '迁移资料库' })
    await user.click(trigger)
    await user.click(screen.getByRole('button', { name: '选择文件夹并开始迁移' }))

    await waitFor(() => expect(screen.queryByRole('dialog')).not.toBeInTheDocument())
    expect(trigger).toHaveFocus()
    expect(api.storageMigrateSelect).toHaveBeenCalledOnce()
    expect(screen.queryByText(/没有完成迁移/)).not.toBeInTheDocument()
  })

  it('enters the global restart boundary after a migration is safely staged', async () => {
    api.settingsOverview.mockResolvedValue({ ok: true, data: {
      activeProblemCount: 0, archivedProblemCount: 0, trashedProblemCount: 0,
      pendingOperationCount: 0, failedOperationCount: 0, unresolvedConflictCount: 0,
      localEncryptionReady: true, cloudSyncConfigured: false,
    } })
    api.storageMigrateSelect.mockResolvedValue({ status: 'ok', data: { ok: true, data: {
      outcome: 'scheduled',
      destinationLabel: '自定义位置 · StudyDisk',
      copiedAssetCount: 3,
      copiedBytes: 12_288,
    } } })
    const enterRestarting = vi.fn()
    const user = userEvent.setup()
    render(SettingsView, {
      global: {
        provide: {
          [libraryAccessControllerKey as symbol]: { enterRestarting },
        },
      },
    })

    await user.click(await screen.findByRole('button', { name: '迁移资料库' }))
    await user.click(screen.getByRole('button', { name: '选择文件夹并开始迁移' }))

    await waitFor(() => expect(enterRestarting).toHaveBeenCalledOnce())
    expect(screen.getByRole('button', { name: '正在复制并校验…' })).toBeDisabled()
  })

  it('keeps the migration decision open and retryable after a safe failure', async () => {
    api.settingsOverview.mockResolvedValue({ ok: true, data: {
      activeProblemCount: 0, archivedProblemCount: 0, trashedProblemCount: 0,
      pendingOperationCount: 0, failedOperationCount: 0, unresolvedConflictCount: 0,
      localEncryptionReady: true, cloudSyncConfigured: false,
    } })
    api.storageMigrateSelect.mockResolvedValue({ status: 'ok', data: { ok: false, error: {
      code: 'storage_migration_failed',
      userMessage: '目标磁盘空间不足，原资料库保持不变。',
      retryable: true,
      diagnosticId: 'storage-test',
    } } })
    const user = userEvent.setup()
    render(SettingsView)

    await user.click(await screen.findByRole('button', { name: '迁移资料库' }))
    await user.click(screen.getByRole('button', { name: '选择文件夹并开始迁移' }))

    expect(await screen.findByRole('alert')).toHaveTextContent('目标磁盘空间不足，原资料库保持不变。')
    expect(screen.getByRole('dialog')).toBeVisible()
    expect(screen.getByRole('button', { name: '选择文件夹并开始迁移' })).toBeEnabled()
  })

  it('announces and consumes the previous migration outcome without exposing paths', async () => {
    api.settingsOverview.mockResolvedValue({ ok: true, data: {
      activeProblemCount: 0, archivedProblemCount: 0, trashedProblemCount: 0,
      pendingOperationCount: 0, failedOperationCount: 0, unresolvedConflictCount: 0,
      localEncryptionReady: true, cloudSyncConfigured: false,
    } })
    api.storageMigrationReceipt.mockResolvedValue({ ok: true, data: {
      outcome: 'moved',
      destinationLabel: '自定义位置 · StudyDisk',
      copiedAssetCount: 4,
      copiedBytes: 16_384,
    } })
    render(SettingsView)

    expect(await screen.findByRole('status', { name: '存储迁移结果' })).toHaveTextContent('资料库已安全迁移')
    expect(screen.getByRole('status', { name: '存储迁移结果' })).toHaveTextContent('4 个加密资源')
    expect(api.storageMigrationReceipt).toHaveBeenCalledOnce()
  })

  it('exports a privacy-safe diagnostic receipt without rendering a local path', async () => {
    api.settingsOverview.mockResolvedValue({ ok: true, data: {
      activeProblemCount: 0, archivedProblemCount: 0, trashedProblemCount: 0,
      pendingOperationCount: 0, failedOperationCount: 0, unresolvedConflictCount: 0,
      localEncryptionReady: true, cloudSyncConfigured: false,
    } })
    api.diagnosticsExport.mockResolvedValue({ status: 'ok', data: { ok: true, data: {
      reportId: '019f4b87-4cab-7b83-a4a0-46acac7d1362',
      fileLabel: 'Mistake-Trainer-Diagnostics-1700000000000-019f4b87.json',
      generatedAtUtcMs: 1_700_000_000_000,
      warningCount: 0,
      path: String.raw`C:\Users\Private\Diagnostics`,
    } } })
    render(SettingsView)

    expect(await screen.findByText('不会包含题图、答案、笔记、账户信息或本机路径')).toBeVisible()
    await userEvent.click(screen.getByRole('button', { name: '生成安全诊断报告' }))

    const receipt = await screen.findByRole('status', { name: '诊断报告已生成' })
    expect(receipt).toHaveTextContent('Mistake-Trainer-Diagnostics-1700000000000-019f4b87.json')
    expect(receipt).toHaveTextContent('019f4b87-4cab-7b83-a4a0-46acac7d1362')
    expect(receipt).toHaveTextContent('所有检查通过')
    expect(receipt).not.toHaveTextContent(/C:\\Users|Private\\Diagnostics/)
    expect(api.diagnosticsExport).toHaveBeenCalledOnce()
  })

  it('treats diagnostic folder cancellation as neutral and restores trigger focus', async () => {
    api.settingsOverview.mockResolvedValue({ ok: true, data: {
      activeProblemCount: 0, archivedProblemCount: 0, trashedProblemCount: 0,
      pendingOperationCount: 0, failedOperationCount: 0, unresolvedConflictCount: 0,
      localEncryptionReady: true, cloudSyncConfigured: false,
    } })
    render(SettingsView)

    const trigger = await screen.findByRole('button', { name: '生成安全诊断报告' })
    await userEvent.click(trigger)

    await waitFor(() => expect(api.diagnosticsExport).toHaveBeenCalledOnce())
    expect(trigger).toHaveFocus()
    expect(screen.queryByRole('alert', { name: '诊断报告未生成' })).not.toBeInTheDocument()
  })

  it('prevents duplicate diagnostic exports while a native selection is pending', async () => {
    api.settingsOverview.mockResolvedValue({ ok: true, data: {
      activeProblemCount: 0, archivedProblemCount: 0, trashedProblemCount: 0,
      pendingOperationCount: 0, failedOperationCount: 0, unresolvedConflictCount: 0,
      localEncryptionReady: true, cloudSyncConfigured: false,
    } })
    let resolveExport!: (value: unknown) => void
    api.diagnosticsExport.mockReturnValue(new Promise(resolve => {
      resolveExport = resolve
    }))
    render(SettingsView)

    const trigger = await screen.findByRole('button', { name: '生成安全诊断报告' })
    await fireEvent.click(trigger)
    await fireEvent.click(trigger)

    expect(api.diagnosticsExport).toHaveBeenCalledOnce()
    expect(screen.getByRole('button', { name: '正在检查并生成…' })).toBeDisabled()
    resolveExport({ status: 'ok', data: { ok: true, data: null } })
    await waitFor(() => expect(screen.getByRole('button', { name: '生成安全诊断报告' })).toBeEnabled())
  })

  it('keeps a failed diagnostic export local and retryable', async () => {
    api.settingsOverview.mockResolvedValue({ ok: true, data: {
      activeProblemCount: 0, archivedProblemCount: 0, trashedProblemCount: 0,
      pendingOperationCount: 0, failedOperationCount: 0, unresolvedConflictCount: 0,
      localEncryptionReady: true, cloudSyncConfigured: false,
    } })
    api.diagnosticsExport.mockResolvedValue({ status: 'ok', data: { ok: false, error: {
      code: 'diagnostics_export_failed',
      userMessage: '目标文件夹不可写，请换一个位置。',
      retryable: true,
      diagnosticId: 'diagnostic-private',
    } } })
    render(SettingsView)

    const trigger = await screen.findByRole('button', { name: '生成安全诊断报告' })
    await userEvent.click(trigger)

    expect(await screen.findByRole('alert', { name: '诊断报告未生成' })).toHaveTextContent('目标文件夹不可写，请换一个位置。')
    expect(trigger).toBeEnabled()
    await userEvent.click(trigger)
    expect(api.diagnosticsExport).toHaveBeenCalledTimes(2)
  })

  it('clears an older diagnostic receipt before a retry starts', async () => {
    api.settingsOverview.mockResolvedValue({ ok: true, data: {
      activeProblemCount: 0, archivedProblemCount: 0, trashedProblemCount: 0,
      pendingOperationCount: 0, failedOperationCount: 0, unresolvedConflictCount: 0,
      localEncryptionReady: true, cloudSyncConfigured: false,
    } })
    api.diagnosticsExport
      .mockResolvedValueOnce({ status: 'ok', data: { ok: true, data: {
        reportId: '019f4b87-4cab-7b83-a4a0-46acac7d1362',
        fileLabel: 'Mistake-Trainer-Diagnostics-old.json',
        generatedAtUtcMs: 1_700_000_000_000,
        warningCount: 0,
      } } })
      .mockResolvedValueOnce({ status: 'ok', data: { ok: false, error: {
        code: 'diagnostics_export_failed',
        userMessage: '目标文件夹不可写，请换一个位置。',
        retryable: true,
        diagnosticId: 'diagnostic-private',
      } } })
    render(SettingsView)

    const trigger = await screen.findByRole('button', { name: '生成安全诊断报告' })
    await userEvent.click(trigger)
    expect(await screen.findByRole('status', { name: '诊断报告已生成' })).toHaveTextContent('Mistake-Trainer-Diagnostics-old.json')

    await userEvent.click(trigger)
    expect(await screen.findByRole('alert', { name: '诊断报告未生成' })).toBeVisible()
    expect(screen.queryByRole('status', { name: '诊断报告已生成' })).not.toBeInTheDocument()
  })
})
