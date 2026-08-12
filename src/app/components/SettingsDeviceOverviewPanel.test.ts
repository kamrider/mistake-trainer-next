import { render, screen } from '@testing-library/vue'
import userEvent from '@testing-library/user-event'
import { describe, expect, it } from 'vitest'
import type { CloudAuthState, LibraryAccessStatus, SettingsOverview, WindowsCompatibilityStatus } from '../../shared/api/bindings'
import SettingsDeviceOverviewPanel from './SettingsDeviceOverviewPanel.vue'

const overview: SettingsOverview = {
  activeProblemCount: 8,
  archivedProblemCount: 2,
  trashedProblemCount: 1,
  pendingOperationCount: 11,
  failedOperationCount: 3,
  unresolvedConflictCount: 2,
  localEncryptionReady: true,
  cloudSyncConfigured: true,
}

const accessStatus: LibraryAccessStatus = {
  state: 'unlocked',
  trustedWindowsAccount: true,
  recoveryReason: null,
}

const cloudAuth: CloudAuthState = {
  configured: true,
  status: { kind: 'connected', emailHint: 'u***@example.com' },
}

const windowsCompatibility: WindowsCompatibilityStatus = {
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
}

const baseProps = {
  overview,
  accessStatus,
  accessError: '',
  cloudAuth,
  windowsCompatibility,
  loading: false,
}

describe('SettingsDeviceOverviewPanel', () => {
  it('renders truthful device, compatibility, and library status and emits a lock intention', async () => {
    const view = render(SettingsDeviceOverviewPanel, { props: baseProps })

    expect(screen.getByRole('heading', { name: '这台 Windows 电脑' })).toBeVisible()
    expect(screen.getByText('SQLCipher 与原图独立加密')).toBeVisible()
    expect(screen.getByText('当前 Windows 账户可解锁')).toBeVisible()
    expect(screen.getByText('已连接')).toBeVisible()
    expect(screen.getByText('退出云端只影响这台电脑，其他设备保持登录。')).toBeVisible()
    expect(screen.getByText('8 道活动题')).toBeVisible()
    expect(screen.getByText(/2 道归档 · 1 道回收站/)).toBeVisible()
    expect(screen.getByRole('article', { name: 'Windows 兼容性' })).toHaveTextContent('Build 26100.1000')
    expect(screen.getByText('5 项')).toBeVisible()

    await userEvent.click(screen.getByRole('button', { name: '立即锁定资料库' }))

    expect(view.emitted().requestLock).toHaveLength(1)
  })

  it('does not fabricate protection readiness while status checks are unavailable', () => {
    render(SettingsDeviceOverviewPanel, {
      props: {
        ...baseProps,
        overview: {
          ...overview,
          localEncryptionReady: false,
          cloudSyncConfigured: false,
        },
        accessStatus: undefined,
        accessError: '无法读取 Windows 资料库凭据，已保持锁定。',
        cloudAuth: undefined,
        windowsCompatibility: undefined,
      },
    })

    expect(screen.getByText('正在检查加密状态')).toBeVisible()
    expect(screen.getByText('状态暂不可用')).toBeVisible()
    expect(screen.getByText('正在检查')).toBeVisible()
    expect(screen.getByRole('status', { name: '当前设备保护状态' })).toHaveTextContent('已保持锁定')
    expect(screen.queryByText('当前 Windows 账户可解锁')).not.toBeInTheDocument()
    expect(screen.queryByRole('article', { name: 'Windows 兼容性' })).not.toBeInTheDocument()
  })
})
