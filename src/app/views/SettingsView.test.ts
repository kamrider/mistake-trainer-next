import { fireEvent, render, screen, waitFor, within } from '@testing-library/vue'
import userEvent from '@testing-library/user-event'
import { createMemoryHistory, RouterView } from 'vue-router'
import { beforeEach, describe, expect, it, vi } from 'vitest'
import type { AppResult } from '../../shared/api/app-result'
import type { OcrComponentStatus, SyncNowReport } from '../../shared/api/bindings'
import { createAppRouter } from '../router'
import { libraryAccessControllerKey } from '../library-access-controller'
import { syncControllerKey } from '../sync-controller'
import {
  createWorkspaceTransitionGuard,
  workspaceTransitionGuardKey,
} from '../workspace-transition-guard'
import SettingsView from './SettingsView.vue'

const api = vi.hoisted(() => ({
  settingsOverview: vi.fn(),
  syncBackendStatus: vi.fn(),
  syncBackendSet: vi.fn(),
  authStatusCommand: vi.fn(),
  authSignIn: vi.fn(),
  authSignUp: vi.fn(),
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
  windowsUpdateStatus: vi.fn(),
  windowsUpdateCheck: vi.fn(),
  windowsUpdateInstall: vi.fn(),
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

const ocrNotInstalledComponent = {
  id: 'ppocrv6_small' as const,
  displayName: 'PP‑OCRv6 small',
  description: '约 31 MB，面向题号与文字框检测。',
  state: 'not_installed' as const,
  downloadBytes: 31_163_977,
  installedBytes: 0,
  recommended: true,
  installAllowed: true,
  statusDetail: '尚未下载；不会影响现有功能。',
  sourceLabel: 'ModelScope · RapidAI/RapidOCR 3.9.2',
  licenseLabel: 'PaddleOCR · Apache-2.0',
}
const ocrInstalledComponent = {
  ...ocrNotInstalledComponent,
  state: 'installed' as const,
  installedBytes: 31_163_977,
  statusDetail: '模型文件已经校验。',
}

function ocrCapabilityStatus(
  component: OcrComponentStatus = ocrNotInstalledComponent,
  automaticRecognitionEnabled = false,
) {
  return {
    assessment: {
      tier: 'balanced' as const,
      logicalProcessorCount: 4,
      totalMemoryMb: 8192,
      availableComponentStorageMb: 8192,
      avx2Supported: true,
      estimatedSuitable: true,
      recommendedComponentId: 'ppocrv6_small' as const,
      summary: '本机预检通过，推荐使用 PP‑OCRv6 small。',
    },
    components: [component, {
      id: 'opencv_preprocess' as const,
      displayName: 'OpenCV 图像预处理',
      description: '产品运行时尚未发布。',
      state: 'unavailable' as const,
      downloadBytes: 0,
      installedBytes: 0,
      recommended: false,
      installAllowed: false,
      statusDetail: '产品运行时尚未发布；当前不会下载或启用。',
      sourceLabel: 'OpenCV 官方项目',
      licenseLabel: 'Apache-2.0',
    }],
    recognitionFeature: {
      state: automaticRecognitionEnabled ? 'ready' as const : 'evidence_gate_pending' as const,
      requiredComponentId: 'ppocrv6_small' as const,
      detail: automaticRecognitionEnabled
        ? '题号定位增强可用。'
        : '智能分题仍在真实题图验证中；顺序模板和手工整理可继续使用。',
    },
    automaticRecognitionEnabled,
  }
}

function deferred<T = unknown>() {
  let resolve!: (value: T) => void
  let reject!: (reason?: unknown) => void
  const promise = new Promise<T>((finish, fail) => { resolve = finish; reject = fail })
  return { promise, resolve, reject }
}

async function renderRoutedSettings() {
  const router = createAppRouter(createMemoryHistory())
  await router.push('/settings')
  await router.isReady()
  render(RouterView, { global: { plugins: [router] } })
  return router
}

async function renderGuardedRoutedSettings() {
  const router = createAppRouter(createMemoryHistory())
  const workspaceTransitionGuard = createWorkspaceTransitionGuard()
  await router.push('/settings')
  await router.isReady()
  render(RouterView, {
    global: {
      plugins: [router],
      provide: {
        [workspaceTransitionGuardKey as symbol]: workspaceTransitionGuard,
      },
    },
  })
  return { router, workspaceTransitionGuard }
}

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
    api.windowsUpdateStatus.mockResolvedValue({ status: 'ok', data: { ok: true, data: {
      enabled: false,
      currentVersion: '0.1.0',
    } } })
    api.windowsUpdateCheck.mockResolvedValue({ status: 'ok', data: { ok: true, data: {
      available: false,
      currentVersion: '0.1.0',
      version: null,
      publishedAt: null,
    } } })
    api.windowsUpdateInstall.mockResolvedValue({ status: 'ok', data: { ok: true, data: {
      acceptedVersion: '0.2.0',
    } } })
    api.ocrCapabilityStatus.mockResolvedValue({
      status: 'ok',
      data: { ok: true, data: ocrCapabilityStatus() },
    })
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

    expect(await screen.findByRole('heading', { level: 1, name: '设置' })).toBeVisible()
    expect(screen.getByText(/数据安静地待在该在的地方/)).toBeVisible()
    const card = await screen.findByRole('article', { name: 'Windows 兼容性' })
    expect(card).toHaveTextContent('完整支持')
    expect(card).toHaveTextContent('Windows 11 Pro')
    expect(card).toHaveTextContent('Build 26100.1000')
    expect(card).toHaveTextContent('x86_64')
    expect(card).toHaveTextContent('WebView2 138.0.3351.83')
    expect(screen.getByRole('group', { name: '账户与同步' })).toHaveTextContent('同步账户')
    expect(screen.getByRole('group', { name: '学习体验' })).toHaveTextContent('科目配置')
    expect(screen.getByRole('group', { name: '数据与安全' })).toHaveTextContent('备份恢复')
    expect(screen.getByRole('group', { name: '应用维护' })).toHaveTextContent('安全诊断')
    expect(api.compatibilityStatus).toHaveBeenCalledOnce()
  })

  it('keeps healthy settings sections usable when the overview read rejects', async () => {
    api.settingsOverview.mockRejectedValueOnce(new Error('overview offline'))

    render(SettingsView)

    expect(await screen.findByRole('alert')).toHaveTextContent(
      '部分设置暂时无法读取：资料库概览。其他设置仍可使用',
    )
    expect(await screen.findByRole('region', { name: '常用科目' })).toBeVisible()
    expect(await screen.findByRole('region', { name: '训练间的专注插曲' })).toBeVisible()
    expect(await screen.findByRole('region', { name: '智能功能模式' })).toBeVisible()
    expect(await screen.findByRole('region', { name: '资料库存储位置' })).toBeVisible()
    expect(api.subjectPreferencesGet).toHaveBeenCalledOnce()
    expect(api.reviewPreferencesGet).toHaveBeenCalledOnce()
    expect(api.ocrCapabilityStatus).toHaveBeenCalledOnce()
    expect(api.storageStatus).toHaveBeenCalledOnce()
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

  it('keeps successful sync truthful when only the overview refresh fails', async () => {
    const initialOverview = {
      activeProblemCount: 1, archivedProblemCount: 0, trashedProblemCount: 0,
      pendingOperationCount: 1, failedOperationCount: 0, unresolvedConflictCount: 0,
      localEncryptionReady: true, cloudSyncConfigured: true,
    }
    api.settingsOverview
      .mockResolvedValueOnce({ ok: true, data: initialOverview })
      .mockRejectedValueOnce(new Error('overview offline'))
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
      global: { provide: { [syncControllerKey as symbol]: { run } } },
    })

    await userEvent.click(await screen.findByRole('button', { name: '立即同步' }))

    expect(await screen.findByText(
      '同步完成：上传 1 项，拉取 2 项；顶部资料库统计暂时没有刷新。',
    )).toBeVisible()
    expect(screen.queryByText(/同步请求没有完成/)).not.toBeInTheDocument()
  })

  it('disables settings refresh while manual sync is pending', async () => {
    const currentOverview = {
      activeProblemCount: 1, archivedProblemCount: 0, trashedProblemCount: 0,
      pendingOperationCount: 1, failedOperationCount: 0, unresolvedConflictCount: 0,
      localEncryptionReady: true, cloudSyncConfigured: true,
    }
    api.settingsOverview.mockResolvedValue({ ok: true, data: currentOverview })
    api.authStatusCommand.mockResolvedValue({ ok: true, data: {
      configured: true, status: { kind: 'connected', emailHint: 's***@example.test' },
    } })
    const syncGate = deferred<AppResult<SyncNowReport>>()
    const run = vi.fn().mockReturnValueOnce(syncGate.promise)
    render(SettingsView, {
      global: { provide: { [syncControllerKey as symbol]: { run } } },
    })

    await userEvent.click(await screen.findByRole('button', { name: '立即同步' }))
    await waitFor(() => expect(run).toHaveBeenCalledOnce())
    const refreshButton = screen.getByRole('button', { name: '刷新' })
    expect(refreshButton).toHaveClass('settings-refresh')
    expect(refreshButton).toBeDisabled()

    syncGate.resolve({
      ok: true,
      data: {
        pushedOperationCount: 0,
        uploadedAssetCount: 0,
        pulledChangeCount: 0,
        downloadedAssetCount: 0,
        finalCursor: 0,
      },
    })
    await waitFor(() => expect(screen.getByRole('button', { name: '刷新' })).toBeEnabled())
  })

  it('keeps the confirmed installed state when the full capability refresh fails', async () => {
    api.settingsOverview.mockResolvedValue({ ok: true, data: {
      activeProblemCount: 0, archivedProblemCount: 0, trashedProblemCount: 0,
      pendingOperationCount: 0, failedOperationCount: 0, unresolvedConflictCount: 0,
      localEncryptionReady: true, cloudSyncConfigured: false,
    } })
    api.ocrCapabilityStatus
      .mockResolvedValueOnce({ status: 'ok', data: { ok: true, data: ocrCapabilityStatus() } })
      .mockResolvedValueOnce({ status: 'ok', data: { ok: false, error: {
        code: 'ocr_status_failed',
        userMessage: '完整状态读取失败。',
        retryable: true,
        diagnosticId: 'diag-status',
      } } })
    api.ocrComponentInstall.mockResolvedValueOnce({
      status: 'ok',
      data: { ok: true, data: ocrInstalledComponent },
    })
    render(SettingsView)

    const panel = await screen.findByRole('region', { name: '智能功能模式' })
    await userEvent.click(await within(panel).findByRole('button', { name: '启用更准切题' }))

    expect(await within(panel).findByRole('button', { name: '移除模型' })).toBeEnabled()
    expect(within(panel).getByRole('status')).toHaveTextContent('本地模型已安装，但完整能力状态暂时未刷新')
    expect(panel).toHaveTextContent('基础版已开放')
    expect(panel).not.toHaveTextContent('题号增强已启用')
  })

  it('does not claim enhancement readiness when installation leaves the full capability disabled', async () => {
    api.settingsOverview.mockResolvedValue({ ok: true, data: {
      activeProblemCount: 0, archivedProblemCount: 0, trashedProblemCount: 0,
      pendingOperationCount: 0, failedOperationCount: 0, unresolvedConflictCount: 0,
      localEncryptionReady: true, cloudSyncConfigured: false,
    } })
    api.ocrCapabilityStatus
      .mockResolvedValueOnce({ status: 'ok', data: { ok: true, data: ocrCapabilityStatus() } })
      .mockResolvedValueOnce({
        status: 'ok',
        data: { ok: true, data: ocrCapabilityStatus(ocrInstalledComponent, false) },
      })
    api.ocrComponentInstall.mockResolvedValueOnce({
      status: 'ok',
      data: { ok: true, data: ocrInstalledComponent },
    })
    render(SettingsView)

    const panel = await screen.findByRole('region', { name: '智能功能模式' })
    await userEvent.click(await within(panel).findByRole('button', { name: '启用更准切题' }))

    expect(await within(panel).findByRole('status')).toHaveTextContent('本地模型已安装；当前仍使用基础预切。')
    expect(panel).toHaveTextContent('基础版已开放')
    expect(panel).not.toHaveTextContent('增强已就绪')
  })

  it('clears the password from view state after a successful authentication request', async () => {
    api.authStatusCommand.mockResolvedValue({ ok: true, data: {
      configured: true, status: { kind: 'signed_out', emailHint: null },
    } })
    api.authSignUp.mockResolvedValue({ status: 'ok', data: { ok: true, data: {
      configured: true, status: { kind: 'verification_required', emailHint: 'u***@example.com' },
    } } })
    const user = userEvent.setup()
    render(SettingsView)

    await user.click(await screen.findByRole('button', { name: '还没有账户？注册' }))
    await user.type(screen.getByRole('textbox', { name: '邮箱' }), 'user@example.com')
    const password = screen.getByLabelText('密码')
    await user.type(password, 'secret-123')
    await user.click(screen.getByRole('button', { name: '注册并连接' }))

    expect(api.authSignUp).toHaveBeenCalledWith({
      email: 'user@example.com',
      password: 'secret-123',
    })
    expect(await screen.findByText(/注册成功，请先完成邮箱验证/)).toBeVisible()
    expect(password).toHaveValue('')
    expect(document.body).not.toHaveTextContent('secret-123')
  })

  it('disables settings refresh while authentication is pending', async () => {
    api.authStatusCommand.mockResolvedValue({ ok: true, data: {
      configured: true, status: { kind: 'signed_out', emailHint: null },
    } })
    const signInGate = deferred<Awaited<ReturnType<typeof api.authSignIn>>>()
    api.authSignIn.mockReturnValueOnce(signInGate.promise)
    const user = userEvent.setup()
    render(SettingsView)

    await user.type(await screen.findByRole('textbox', { name: '邮箱' }), 'user@example.com')
    const password = screen.getByLabelText('密码')
    await user.type(password, 'secret-123')
    await user.click(screen.getByRole('button', { name: '登录并连接' }))

    await waitFor(() => expect(api.authSignIn).toHaveBeenCalledOnce())
    expect(screen.getByRole('button', { name: '刷新' })).toBeDisabled()

    signInGate.resolve({ status: 'ok', data: { ok: true, data: {
      configured: true, status: { kind: 'connected', emailHint: 'u***@example.com' },
    } } })
    await waitFor(() => expect(screen.getByRole('button', { name: '刷新' })).toBeEnabled())
    expect(await screen.findByRole('button', { name: '立即同步' })).toBeVisible()
    expect(document.body).not.toHaveTextContent('secret-123')
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
    expect(await screen.findByText('本机笔记')).toBeVisible()
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

  it('explains duplicate built-in subjects without dirtying the draft', async () => {
    render(SettingsView)

    await userEvent.type(
      await screen.findByRole('textbox', { name: '自定义科目名称' }),
      '数学',
    )
    await userEvent.click(screen.getByRole('button', { name: '添加自定义科目' }))

    expect(await screen.findByText('“数学”已在科目列表中。')).toBeVisible()
    expect(screen.queryByRole('button', { name: '删除自定义科目 数学' })).not.toBeInTheDocument()
    await userEvent.click(screen.getByRole('button', { name: '刷新' }))
    await waitFor(() => expect(api.subjectPreferencesGet).toHaveBeenCalledTimes(2))
    expect(api.subjectPreferencesSave).not.toHaveBeenCalled()
  })

  it('keeps the only enabled custom subject and gives actionable deletion guidance', async () => {
    api.subjectPreferencesGet.mockResolvedValueOnce({ ok: true, data: {
      enabledSubjects: ['编程'],
      customSubjects: ['编程'],
      captureSoundEnabled: true,
    } })
    render(SettingsView)

    const remove = await screen.findByRole('button', { name: '删除自定义科目 编程' })
    await userEvent.click(remove)

    expect(remove).toBeVisible()
    expect(await screen.findByText(
      '至少保留一个常用科目；请先启用其他科目，再删除“编程”。',
    )).toBeVisible()
    expect(api.subjectPreferencesSave).not.toHaveBeenCalled()
  })

  it('automatically persists the latest subject draft without applying an older response', async () => {
    api.settingsOverview.mockResolvedValue({ ok: true, data: {
      activeProblemCount: 0, archivedProblemCount: 0, trashedProblemCount: 0,
      pendingOperationCount: 0, failedOperationCount: 0, unresolvedConflictCount: 0,
      localEncryptionReady: true, cloudSyncConfigured: false,
    } })
    const firstSave = deferred()
    api.subjectPreferencesSave
      .mockReturnValueOnce(firstSave.promise)
      .mockResolvedValueOnce({ ok: true, data: {
        enabledSubjects: ['语文', '数学'],
        customSubjects: [],
        captureSoundEnabled: false,
      } })
    render(SettingsView)

    const subjectPanel = await screen.findByRole('region', { name: '常用科目' })
    await userEvent.click(screen.getByRole('checkbox', { name: '英语' }))
    await userEvent.click(screen.getByRole('button', { name: '保存科目配置' }))
    const sound = screen.getByRole('checkbox', { name: /拖放成功音效/ })
    await userEvent.click(sound)
    expect(sound).not.toBeChecked()
    expect(within(subjectPanel).getByRole('status')).toHaveTextContent('完成当前保存后会自动继续')

    firstSave.resolve({ ok: true, data: {
      enabledSubjects: ['语文', '数学'],
      customSubjects: [],
      captureSoundEnabled: true,
    } })

    await waitFor(() => expect(api.subjectPreferencesSave).toHaveBeenCalledTimes(2))
    expect(api.subjectPreferencesSave).toHaveBeenNthCalledWith(1, {
      enabledSubjects: ['语文', '数学'],
      customSubjects: [],
      captureSoundEnabled: true,
    })
    expect(api.subjectPreferencesSave).toHaveBeenNthCalledWith(2, {
      enabledSubjects: ['语文', '数学'],
      customSubjects: [],
      captureSoundEnabled: false,
    })
    expect(sound).not.toBeChecked()
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

  it('jumps directly to review rhythm settings from the sticky directory', async () => {
    api.settingsOverview.mockResolvedValue({ ok: true, data: {
      activeProblemCount: 0, archivedProblemCount: 0, trashedProblemCount: 0,
      pendingOperationCount: 0, failedOperationCount: 0, unresolvedConflictCount: 0,
      localEncryptionReady: true, cloudSyncConfigured: false,
    } })
    render(SettingsView)

    expect(await screen.findByRole('heading', { name: '训练间的专注插曲' })).toBeVisible()
    const reviewPanel = screen.getByRole('region', { name: '训练间的专注插曲' })
    const scrollIntoView = vi.fn()
    reviewPanel.scrollIntoView = scrollIntoView
    const directory = screen.getByRole('navigation', { name: '设置目录' })
    await userEvent.click(within(directory).getByRole('button', { name: /训练节奏/ }))

    expect(reviewPanel).toHaveAttribute('id', 'settings-review')
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

  it('automatically persists the latest review rhythm without reverting the selected option', async () => {
    api.settingsOverview.mockResolvedValue({ ok: true, data: {
      activeProblemCount: 0, archivedProblemCount: 0, trashedProblemCount: 0,
      pendingOperationCount: 0, failedOperationCount: 0, unresolvedConflictCount: 0,
      localEncryptionReady: true, cloudSyncConfigured: false,
    } })
    const firstSave = deferred()
    api.reviewPreferencesSave
      .mockReturnValueOnce(firstSave.promise)
      .mockResolvedValueOnce({ ok: true, data: { focusPolicy: 'session_start' } })
    render(SettingsView)

    const reviewPanel = await screen.findByRole('region', { name: '训练间的专注插曲' })
    const everyTen = screen.getByRole('radio', { name: /每完成 10 题/ })
    const sessionStart = screen.getByRole('radio', { name: /每轮开始前 · 推荐/ })
    await userEvent.click(everyTen)
    await userEvent.click(screen.getByRole('button', { name: '保存训练节奏' }))
    await userEvent.click(sessionStart)
    expect(sessionStart).toBeChecked()
    expect(within(reviewPanel).getByRole('status')).toHaveTextContent('完成当前保存后会自动继续')

    firstSave.resolve({ ok: true, data: { focusPolicy: 'every_10' } })

    await waitFor(() => expect(api.reviewPreferencesSave).toHaveBeenCalledTimes(2))
    expect(api.reviewPreferencesSave).toHaveBeenNthCalledWith(1, { focusPolicy: 'every_10' })
    expect(api.reviewPreferencesSave).toHaveBeenNthCalledWith(2, { focusPolicy: 'session_start' })
    expect(sessionStart).toBeChecked()
    expect(everyTen).not.toBeChecked()
    expect(await screen.findByText('训练节奏已保存，将从下一轮普通训练开始生效。')).toBeVisible()
  })

  it('protects an unsaved preference draft from the settings refresh', async () => {
    api.settingsOverview.mockResolvedValue({ ok: true, data: {
      activeProblemCount: 0, archivedProblemCount: 0, trashedProblemCount: 0,
      pendingOperationCount: 0, failedOperationCount: 0, unresolvedConflictCount: 0,
      localEncryptionReady: true, cloudSyncConfigured: false,
    } })
    api.subjectPreferencesSave.mockResolvedValue({ ok: true, data: {
      enabledSubjects: ['语文', '数学'],
      customSubjects: [],
      captureSoundEnabled: true,
    } })
    render(SettingsView)

    const english = await screen.findByRole('checkbox', { name: '英语' })
    await userEvent.click(english)
    expect(english).not.toBeChecked()
    await userEvent.click(screen.getByRole('button', { name: '刷新' }))

    expect(await screen.findByRole('alert')).toHaveTextContent('请先保存偏好')
    expect(api.subjectPreferencesGet).toHaveBeenCalledTimes(1)
    expect(english).not.toBeChecked()

    await userEvent.click(screen.getByRole('button', { name: '保存科目配置' }))
    expect(await screen.findByText('科目配置已保存')).toBeVisible()
    await userEvent.click(screen.getByRole('button', { name: '刷新' }))
    await waitFor(() => expect(api.subjectPreferencesGet).toHaveBeenCalledTimes(2))
  })

  it('does not let an in-flight refresh overwrite a preference saved after that refresh started', async () => {
    api.settingsOverview.mockResolvedValue({ ok: true, data: {
      activeProblemCount: 0, archivedProblemCount: 0, trashedProblemCount: 0,
      pendingOperationCount: 0, failedOperationCount: 0, unresolvedConflictCount: 0,
      localEncryptionReady: true, cloudSyncConfigured: false,
    } })
    const initialSubjects = {
      enabledSubjects: ['语文', '数学', '英语'],
      customSubjects: [],
      captureSoundEnabled: true,
    }
    const staleRefresh = deferred()
    api.subjectPreferencesGet
      .mockResolvedValueOnce({ ok: true, data: initialSubjects })
      .mockReturnValueOnce(staleRefresh.promise)
    api.subjectPreferencesSave.mockResolvedValue({ ok: true, data: {
      enabledSubjects: ['语文', '数学'],
      customSubjects: [],
      captureSoundEnabled: true,
    } })
    render(SettingsView)

    const english = await screen.findByRole('checkbox', { name: '英语' })
    await userEvent.click(screen.getByRole('button', { name: '刷新' }))
    await waitFor(() => expect(api.subjectPreferencesGet).toHaveBeenCalledTimes(2))
    await userEvent.click(english)
    await userEvent.click(screen.getByRole('button', { name: '保存科目配置' }))
    expect(await screen.findByText('科目配置已保存')).toBeVisible()
    expect(english).not.toBeChecked()

    staleRefresh.resolve({ ok: true, data: initialSubjects })
    await waitFor(() => expect(screen.getByRole('button', { name: '刷新' })).toBeEnabled())
    expect(english).not.toBeChecked()
  })

  it('confirms before leaving dirty settings and allows same-page query navigation', async () => {
    api.settingsOverview.mockResolvedValue({ ok: true, data: {
      activeProblemCount: 0, archivedProblemCount: 0, trashedProblemCount: 0,
      pendingOperationCount: 0, failedOperationCount: 0, unresolvedConflictCount: 0,
      localEncryptionReady: true, cloudSyncConfigured: false,
    } })
    const router = await renderRoutedSettings()
    const user = userEvent.setup()

    const english = await screen.findByRole('checkbox', { name: '英语' })
    await user.click(english)
    expect(english).not.toBeChecked()
    await router.push({ name: 'settings', query: { section: 'ocr' } })
    expect(router.currentRoute.value.name).toBe('settings')
    expect(screen.queryByRole('alertdialog')).not.toBeInTheDocument()
    expect(english).not.toBeChecked()

    const cancelledNavigation = router.push('/missing-page')
    expect(await screen.findByRole('alertdialog', { name: '放弃设置修改并离开？' })).toBeVisible()
    expect(router.currentRoute.value.name).toBe('settings')
    await waitFor(() => expect(screen.getByRole('button', { name: '继续编辑' })).toHaveFocus())
    await user.click(screen.getByRole('button', { name: '继续编辑' }))
    await cancelledNavigation
    expect(router.currentRoute.value.name).toBe('settings')
    expect(english).not.toBeChecked()
    expect(english).toHaveFocus()

    const confirmedNavigation = router.push('/missing-page')
    await user.click(await screen.findByRole('button', { name: '放弃修改并离开' }))
    await confirmedNavigation
    expect(router.currentRoute.value.name).toBe('not-found')
  })

  it('blocks navigation while preferences are saving and allows it after persistence finishes', async () => {
    api.settingsOverview.mockResolvedValue({ ok: true, data: {
      activeProblemCount: 0, archivedProblemCount: 0, trashedProblemCount: 0,
      pendingOperationCount: 0, failedOperationCount: 0, unresolvedConflictCount: 0,
      localEncryptionReady: true, cloudSyncConfigured: false,
    } })
    const saveGate = deferred()
    api.subjectPreferencesSave.mockReturnValueOnce(saveGate.promise)
    const router = await renderRoutedSettings()

    await userEvent.click(await screen.findByRole('checkbox', { name: '英语' }))
    await userEvent.click(screen.getByRole('button', { name: '保存科目配置' }))
    await router.push('/missing-page')

    expect(router.currentRoute.value.name).toBe('settings')
    expect(await screen.findByRole('alert')).toHaveTextContent('偏好正在保存')
    expect(screen.queryByRole('alertdialog')).not.toBeInTheDocument()

    saveGate.resolve({ ok: true, data: {
      enabledSubjects: ['语文', '数学'],
      customSubjects: [],
      captureSoundEnabled: true,
    } })
    expect(await screen.findByText('科目配置已保存')).toBeVisible()
    await router.push('/missing-page')
    expect(router.currentRoute.value.name).toBe('not-found')
  })

  it('uses one confirmation decision for repeated navigation attempts', async () => {
    api.settingsOverview.mockResolvedValue({ ok: true, data: {
      activeProblemCount: 0, archivedProblemCount: 0, trashedProblemCount: 0,
      pendingOperationCount: 0, failedOperationCount: 0, unresolvedConflictCount: 0,
      localEncryptionReady: true, cloudSyncConfigured: false,
    } })
    const router = await renderRoutedSettings()

    await userEvent.click(await screen.findByRole('checkbox', { name: '英语' }))
    const firstNavigation = router.push('/missing-one')
    await screen.findByRole('alertdialog', { name: '放弃设置修改并离开？' })
    const latestNavigation = router.push('/missing-two')
    await userEvent.click(screen.getByRole('button', { name: '放弃修改并离开' }))
    await Promise.all([firstNavigation, latestNavigation])

    expect(router.currentRoute.value.path).toBe('/missing-two')
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

  it('restores focus to the extracted sign-out action when its lock dialog is cancelled', async () => {
    api.authStatusCommand.mockResolvedValue({ ok: true, data: {
      configured: true, status: { kind: 'connected', emailHint: 'u***@example.com' },
    } })
    const user = userEvent.setup()
    render(SettingsView)

    const trigger = await screen.findByRole('button', { name: '退出云端并锁定' })
    await user.click(trigger)
    expect(screen.getByRole('heading', { name: '退出云端并锁定本机？' })).toBeVisible()

    await user.keyboard('{Escape}')

    await waitFor(() => expect(screen.queryByRole('dialog')).not.toBeInTheDocument())
    expect(trigger).toHaveFocus()
    expect(api.authDisconnect).not.toHaveBeenCalled()
    expect(api.libraryLock).not.toHaveBeenCalled()
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
    await waitFor(() => expect(screen.getAllByText('离线模式')).toHaveLength(2))
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

  it('disables refresh and backend choices while a backend selection is pending', async () => {
    const selection = deferred<Awaited<ReturnType<typeof api.syncBackendSet>>>()
    api.syncBackendSet.mockReturnValueOnce(selection.promise)
    render(SettingsView)

    const supabase = await screen.findByRole('button', { name: /^Supabase/ })
    await userEvent.click(supabase)

    await waitFor(() => expect(api.syncBackendSet).toHaveBeenCalledWith({ kind: 'supabase' }))
    expect(screen.getByRole('button', { name: '刷新' })).toBeDisabled()
    expect(supabase).toBeDisabled()
    expect(screen.getByRole('button', { name: /^仅本地/ })).toBeDisabled()

    selection.resolve({ ok: true, data: {
      kind: 'supabase', configured: true, ready: true, syncEnabled: true,
    } })
    expect(await screen.findByText('已选择 Supabase（海外/开发）')).toBeVisible()
    await waitFor(() => expect(screen.getByRole('button', { name: '刷新' })).toBeEnabled())
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

  it('admits only one native backup operation before disabled state renders', async () => {
    const creation = deferred()
    api.backupCreate.mockReturnValue(creation.promise)
    api.backupPrepareRestore.mockResolvedValue({ status: 'ok', data: { ok: true, data: null } })
    render(SettingsView)

    const createButton = await screen.findByRole('button', { name: /创建加密备份/ })
    const prepareButton = screen.getByRole('button', { name: /选择备份并准备恢复/ })
    createButton.click()
    createButton.click()
    prepareButton.click()
    await waitFor(() => expect(api.backupCreate).toHaveBeenCalled())

    expect(api.backupCreate).toHaveBeenCalledOnce()
    expect(api.backupPrepareRestore).not.toHaveBeenCalled()

    creation.resolve({ status: 'ok', data: { ok: true, data: null } })
    await waitFor(() => expect(createButton).toBeEnabled())
  })

  it('admits only one restore-package preparation before disabled state renders', async () => {
    const preparation = deferred()
    api.backupPrepareRestore.mockReturnValue(preparation.promise)
    api.backupCreate.mockResolvedValue({ status: 'ok', data: { ok: true, data: null } })
    render(SettingsView)

    const prepareButton = await screen.findByRole('button', { name: /选择备份并准备恢复/ })
    const createButton = screen.getByRole('button', { name: /创建加密备份/ })
    prepareButton.click()
    prepareButton.click()
    createButton.click()
    await waitFor(() => expect(api.backupPrepareRestore).toHaveBeenCalled())

    expect(api.backupPrepareRestore).toHaveBeenCalledOnce()
    expect(api.backupCreate).not.toHaveBeenCalled()

    preparation.resolve({ status: 'ok', data: { ok: true, data: null } })
    await waitFor(() => expect(prepareButton).toBeEnabled())
  })

  it('blocks route, workspace, and window transitions during a backup operation', async () => {
    const creation = deferred()
    api.backupCreate.mockReturnValue(creation.promise)
    const { router, workspaceTransitionGuard } = await renderGuardedRoutedSettings()

    await userEvent.click(await screen.findByRole('button', { name: /创建加密备份/ }))
    await waitFor(() => expect(api.backupCreate).toHaveBeenCalledOnce())
    await router.push({ name: 'dashboard' })
    const workspaceAllowed = await workspaceTransitionGuard.attempt()
    const busyUnload = new Event('beforeunload', { cancelable: true })
    window.dispatchEvent(busyUnload)
    const blockedMessage = screen.queryByText('备份操作正在完成，请等待完成后再离开设置。')

    creation.resolve({ status: 'ok', data: { ok: true, data: null } })

    expect(router.currentRoute.value.name).toBe('settings')
    expect(workspaceAllowed).toBe(false)
    expect(busyUnload.defaultPrevented).toBe(true)
    expect(blockedMessage).toBeVisible()
    await waitFor(() =>
      expect(screen.getByRole('button', { name: /创建加密备份/ })).toBeEnabled())
    await expect(workspaceTransitionGuard.attempt()).resolves.toBe(true)
    const idleUnload = new Event('beforeunload', { cancelable: true })
    window.dispatchEvent(idleUnload)
    expect(idleUnload.defaultPrevented).toBe(false)
    await router.push({ name: 'dashboard' })
    expect(router.currentRoute.value.name).toBe('dashboard')
  })

  it('keeps a validated candidate retryable and restores focus when restore startup fails', async () => {
    api.backupPrepareRestore.mockResolvedValue({ status: 'ok', data: { ok: true, data: {
      id: 'candidate-retry',
      expiresAtUtcMs: 1_725_086_400_000,
      summary: {
        formatVersion: 1,
        createdAtUtcMs: 1_725_000_000_000,
        assetCount: 4,
        encryptedBytes: 2_097_152,
        label: 'retryable-backup',
        readyForRestore: true,
      },
    } } })
    api.backupRestore.mockResolvedValue({ status: 'ok', data: {
      ok: false,
      error: {
        code: 'backup_restore_failed',
        userMessage: '恢复任务没有开始，候选仍可重试。',
        retryable: true,
        diagnosticId: 'restore-retry',
      },
    } })
    const user = userEvent.setup()
    render(SettingsView)

    await user.click(await screen.findByRole('button', { name: /选择备份并准备恢复/ }))
    const restoreTrigger = await screen.findByRole('button', { name: '查看风险并确认恢复' })
    await user.click(restoreTrigger)
    await user.click(screen.getByRole('checkbox', { name: /我明白：确认后当前题库/ }))
    await user.click(screen.getByRole('button', { name: /确认恢复并重启/ }))

    expect(await screen.findByRole('alert')).toHaveTextContent('候选仍可重试')
    expect(screen.queryByRole('dialog')).not.toBeInTheDocument()
    expect(screen.getByText('retryable-backup')).toBeVisible()
    expect(restoreTrigger).toHaveFocus()
    expect(api.backupRestore).toHaveBeenCalledWith('candidate-retry')
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

  it('shows ordinary builds as update-disabled without making a network check', async () => {
    render(SettingsView)

    expect(await screen.findByText('当前安装包未接入自动更新')).toBeVisible()
    expect(screen.getByText('当前版本 0.1.0')).toBeVisible()
    expect(screen.queryByRole('button', { name: '检查更新' })).not.toBeInTheDocument()
    expect(api.windowsUpdateStatus).toHaveBeenCalledOnce()
    expect(api.windowsUpdateCheck).not.toHaveBeenCalled()
  })

  it('checks once at a time and reports that the signed build is current', async () => {
    api.windowsUpdateStatus.mockResolvedValue({ status: 'ok', data: { ok: true, data: {
      enabled: true,
      currentVersion: '0.1.0',
    } } })
    let resolveCheck!: (value: unknown) => void
    api.windowsUpdateCheck.mockReturnValue(new Promise(resolve => {
      resolveCheck = resolve
    }))
    render(SettingsView)

    const trigger = await screen.findByRole('button', { name: '检查更新' })
    await fireEvent.click(trigger)
    await fireEvent.click(trigger)

    expect(api.windowsUpdateCheck).toHaveBeenCalledOnce()
    expect(screen.getByRole('button', { name: '正在检查…' })).toBeDisabled()
    resolveCheck({ status: 'ok', data: { ok: true, data: {
      available: false,
      currentVersion: '0.1.0',
      version: null,
      publishedAt: null,
    } } })
    expect(await screen.findByRole('status', { name: '应用更新状态' })).toHaveTextContent('当前已经是最新版本')
    await waitFor(() => expect(trigger).toHaveFocus())
  })

  it('installs only the exact version returned by the verified update check', async () => {
    api.windowsUpdateStatus.mockResolvedValue({ status: 'ok', data: { ok: true, data: {
      enabled: true,
      currentVersion: '0.1.0',
    } } })
    api.windowsUpdateCheck.mockResolvedValue({ status: 'ok', data: { ok: true, data: {
      available: true,
      currentVersion: '0.1.0',
      version: '0.2.0',
      publishedAt: '2026-07-28T00:00:00Z',
      endpoint: 'https://private.example/latest.json',
      signature: 'must-not-render',
    } } })
    render(SettingsView)

    await userEvent.click(await screen.findByRole('button', { name: '检查更新' }))
    const available = await screen.findByRole('status', { name: '应用更新状态' })
    expect(available).toHaveTextContent('发现已签名版本 0.2.0')
    expect(available).not.toHaveTextContent(/private\.example|must-not-render/)

    await userEvent.click(screen.getByRole('button', { name: '安装 0.2.0' }))
    expect(api.windowsUpdateInstall).toHaveBeenCalledWith('0.2.0')
    expect(await screen.findByRole('status', { name: '应用更新状态' })).toHaveTextContent('安装程序已启动')
  })

  it('keeps update failures retryable and never renders private updater details', async () => {
    api.windowsUpdateStatus.mockResolvedValue({ status: 'ok', data: { ok: true, data: {
      enabled: true,
      currentVersion: '0.1.0',
    } } })
    api.windowsUpdateCheck.mockResolvedValue({ status: 'ok', data: { ok: false, error: {
      code: 'update_check_failed',
      userMessage: '暂时无法检查更新，请稍后重试。',
      retryable: true,
      diagnosticId: 'private-diagnostic',
      rawError: 'https://private.example/latest.json?token=secret',
    } } })
    render(SettingsView)

    const trigger = await screen.findByRole('button', { name: '检查更新' })
    await userEvent.click(trigger)

    const status = await screen.findByRole('status', { name: '应用更新状态' })
    expect(status).toHaveTextContent('暂时无法检查更新，请稍后重试。')
    expect(status).not.toHaveTextContent(/private-diagnostic|private\.example|token=secret/)
    expect(trigger).toBeEnabled()
    expect(trigger).toHaveFocus()
  })

  it('discards a stale available version when installation reports a version change', async () => {
    api.windowsUpdateStatus.mockResolvedValue({ status: 'ok', data: { ok: true, data: {
      enabled: true,
      currentVersion: '0.1.0',
    } } })
    api.windowsUpdateCheck.mockResolvedValue({ status: 'ok', data: { ok: true, data: {
      available: true,
      currentVersion: '0.1.0',
      version: '0.2.0',
      publishedAt: null,
    } } })
    api.windowsUpdateInstall.mockResolvedValue({ status: 'ok', data: { ok: false, error: {
      code: 'update_version_changed',
      userMessage: '可用版本已经变化，请重新检查。',
      retryable: true,
      diagnosticId: 'update-version',
    } } })
    render(SettingsView)

    await userEvent.click(await screen.findByRole('button', { name: '检查更新' }))
    await userEvent.click(await screen.findByRole('button', { name: '安装 0.2.0' }))

    expect(await screen.findByRole('status', { name: '应用更新状态' })).toHaveTextContent('可用版本已经变化，请重新检查。')
    expect(screen.queryByRole('button', { name: '安装 0.2.0' })).not.toBeInTheDocument()
    expect(screen.getByRole('button', { name: '检查更新' })).toBeEnabled()
  })
})
