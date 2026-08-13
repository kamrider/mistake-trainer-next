import { describe, expect, it, vi } from 'vitest'
import { failure, success, type AppResult } from '../../shared/api/app-result'
import type {
  WindowsUpdateCheckReport,
  WindowsUpdateInstallReceipt,
} from '../../shared/api/bindings'
import { useSettingsWindowsUpdate } from './useSettingsWindowsUpdate'

const available: WindowsUpdateCheckReport = {
  available: true,
  currentVersion: '0.1.0',
  version: '0.2.0',
  publishedAt: '2026-08-10T00:00:00Z',
}

function deferred<T>() {
  let resolve!: (value: T) => void
  const promise = new Promise<T>((finish) => { resolve = finish })
  return { promise, resolve }
}

function harness() {
  const operations = {
    compatibility: vi.fn(async () => success({
      supportLevel: 'supported' as const,
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
    })),
    status: vi.fn(async () => success({ enabled: true, currentVersion: '0.1.0' })),
    check: vi.fn(async () => success(available)),
    install: vi.fn(async () => success({ acceptedVersion: '0.2.0' })),
  }
  const restoreFocus = vi.fn(async () => undefined)
  return {
    operations,
    restoreFocus,
    controller: useSettingsWindowsUpdate({ operations, restoreFocus }),
  }
}

describe('useSettingsWindowsUpdate', () => {
  it('loads supplementary compatibility and updater status silently', async () => {
    const current = harness()
    await expect(current.controller.loadCompatibility()).resolves.toBe(true)
    await expect(current.controller.loadStatus()).resolves.toBe(true)
    expect(current.controller.compatibility.value?.osName).toBe('Windows 11 Pro')
    expect(current.controller.status.value).toEqual({ enabled: true, currentVersion: '0.1.0' })

    current.operations.compatibility.mockRejectedValueOnce(new Error('probe failed'))
    current.operations.status.mockRejectedValueOnce(new Error('status failed'))
    await expect(current.controller.loadCompatibility()).resolves.toBe(false)
    await expect(current.controller.loadStatus()).resolves.toBe(false)
  })

  it('does not check for updates when the installed build is disabled', async () => {
    const current = harness()
    current.operations.status.mockResolvedValueOnce(success({ enabled: false, currentVersion: '0.1.0' }))
    await current.controller.loadStatus()

    await expect(current.controller.check()).resolves.toBe(false)
    expect(current.operations.check).not.toHaveBeenCalled()
  })

  it('reports available and latest-version checks with accepted metadata only', async () => {
    const current = harness()
    await current.controller.loadStatus()
    await expect(current.controller.check()).resolves.toBe(true)
    expect(current.controller.message.value).toBe(
      '发现已签名版本 0.2.0。下载后仍会再次核对版本和签名。',
    )
    expect(current.controller.publicationLabel.value).toContain('2026')

    current.operations.check.mockResolvedValueOnce(success({
      ...available,
      available: false,
      version: null,
      publishedAt: null,
    }))
    await expect(current.controller.check()).resolves.toBe(true)
    expect(current.controller.message.value).toBe('当前已经是最新版本。')
    expect(current.controller.report.value?.version).toBeNull()
  })

  it('coalesces check clicks and restores focus after completion', async () => {
    const pending = deferred<AppResult<WindowsUpdateCheckReport>>()
    const current = harness()
    await current.controller.loadStatus()
    current.operations.check.mockReturnValueOnce(pending.promise)

    const first = current.controller.check()
    const second = current.controller.check()
    expect(second).toBe(first)
    expect(current.controller.checking.value).toBe(true)
    expect(current.operations.check).toHaveBeenCalledOnce()

    pending.resolve(success(available))
    await first
    expect(current.restoreFocus).toHaveBeenCalledOnce()
  })

  it('keeps backend and transport check failures retryable', async () => {
    const current = harness()
    await current.controller.loadStatus()
    current.operations.check.mockResolvedValueOnce(failure(
      'update_check_failed', '暂时无法检查更新，请稍后重试。', true, 'private-detail',
    ))
    await expect(current.controller.check()).resolves.toBe(false)
    expect(current.controller.message.value).toBe('暂时无法检查更新，请稍后重试。')
    expect(current.controller.report.value).toBeUndefined()

    current.operations.check.mockRejectedValueOnce(new Error('network unavailable'))
    await expect(current.controller.check()).resolves.toBe(false)
    expect(current.controller.message.value).toBe(
      '暂时无法检查更新，请确认网络连接后重试；当前版本可继续离线使用。',
    )
  })

  it('installs only the exact displayed version and coalesces duplicate clicks', async () => {
    const pending = deferred<AppResult<WindowsUpdateInstallReceipt>>()
    const current = harness()
    await current.controller.loadStatus()
    await current.controller.check()
    current.restoreFocus.mockClear()
    current.operations.install.mockReturnValueOnce(pending.promise)

    const first = current.controller.install()
    const second = current.controller.install()
    expect(second).toBe(first)
    expect(current.operations.install).toHaveBeenCalledWith('0.2.0')
    expect(current.controller.installing.value).toBe(true)

    pending.resolve(success({ acceptedVersion: '0.2.0' }))
    await first
    expect(current.controller.message.value).toBe('安装程序已启动；请按系统提示完成更新。')
    expect(current.restoreFocus).toHaveBeenCalledOnce()
  })

  it('invalidates stale reports after install application and transport failures', async () => {
    const rejected = harness()
    await rejected.controller.loadStatus()
    await rejected.controller.check()
    rejected.operations.install.mockResolvedValueOnce(failure(
      'update_version_changed', '可用版本已经变化，请重新检查。', true, 'private-version',
    ))
    await expect(rejected.controller.install()).resolves.toBe(false)
    expect(rejected.controller.report.value).toBeUndefined()
    expect(rejected.controller.message.value).toBe('可用版本已经变化，请重新检查。')

    const thrown = harness()
    await thrown.controller.loadStatus()
    await thrown.controller.check()
    thrown.operations.install.mockRejectedValueOnce(new Error('installer failed'))
    await expect(thrown.controller.install()).resolves.toBe(false)
    expect(thrown.controller.report.value).toBeUndefined()
    expect(thrown.controller.message.value).toBe('更新没有安装，当前版本保持不变；请稍后重新检查。')
  })

  it('formats only valid publication timestamps', () => {
    const current = harness()
    expect(current.controller.formatPublication(null)).toBe('')
    expect(current.controller.formatPublication('not-a-date')).toBe('')
    expect(current.controller.formatPublication('2026-08-10T00:00:00Z')).toContain('2026')
  })
})
