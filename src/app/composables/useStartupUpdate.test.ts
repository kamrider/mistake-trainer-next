import { afterEach, beforeEach, describe, expect, it, vi } from 'vitest'
import { failure, success } from '../../shared/api/app-result'
import type { WindowsUpdateCheckReport } from '../../shared/api/bindings'
import {
  STARTUP_UPDATE_CHECK_INTERVAL_MS,
  STARTUP_UPDATE_DELAY_MS,
  STARTUP_UPDATE_STORAGE_KEY,
  useStartupUpdate,
} from './useStartupUpdate'

const currentVersion = '0.1.0'
const availableUpdate: WindowsUpdateCheckReport = {
  available: true,
  currentVersion,
  version: '0.2.0',
  publishedAt: '2026-08-10T00:00:00Z',
}

function createOperations() {
  return {
    status: vi.fn().mockResolvedValue(success({ enabled: true, currentVersion })),
    check: vi.fn().mockResolvedValue(success(availableUpdate)),
    install: vi.fn().mockResolvedValue(success({ acceptedVersion: '0.2.0' })),
  }
}

async function runScheduledCheck(controller: ReturnType<typeof useStartupUpdate>) {
  controller.start()
  await vi.advanceTimersByTimeAsync(STARTUP_UPDATE_DELAY_MS)
}

describe('useStartupUpdate', () => {
  beforeEach(() => {
    vi.useFakeTimers()
    window.localStorage.clear()
  })

  afterEach(() => {
    vi.useRealTimers()
  })

  it('does not schedule update work outside the desktop runtime', async () => {
    const operations = createOperations()
    const controller = useStartupUpdate({ desktopRuntime: false, operations })

    await runScheduledCheck(controller)

    expect(operations.status).not.toHaveBeenCalled()
    expect(operations.check).not.toHaveBeenCalled()
  })

  it('stays offline without recording an attempt or opening a dialog', async () => {
    const operations = createOperations()
    const controller = useStartupUpdate({
      desktopRuntime: true,
      operations,
      online: () => false,
    })

    await runScheduledCheck(controller)

    expect(operations.status).not.toHaveBeenCalled()
    expect(window.localStorage.getItem(STARTUP_UPDATE_STORAGE_KEY)).toBeNull()
    expect(controller.report.value).toBeUndefined()
  })

  it('does not contact the update endpoint when the installed build is disabled', async () => {
    const operations = createOperations()
    operations.status.mockResolvedValue(success({ enabled: false, currentVersion }))
    const controller = useStartupUpdate({ desktopRuntime: true, operations })

    await runScheduledCheck(controller)

    expect(operations.status).toHaveBeenCalledOnce()
    expect(operations.check).not.toHaveBeenCalled()
    expect(window.localStorage.getItem(STARTUP_UPDATE_STORAGE_KEY)).toBeNull()
  })

  it('fails closed when the daily attempt cannot be persisted', async () => {
    const operations = createOperations()
    const controller = useStartupUpdate({
      desktopRuntime: true,
      operations,
      storage: {
        getItem: () => null,
        setItem: () => { throw new Error('storage unavailable') },
      },
    })

    await runScheduledCheck(controller)

    expect(operations.status).toHaveBeenCalledOnce()
    expect(operations.check).not.toHaveBeenCalled()
    expect(controller.report.value).toBeUndefined()
  })

  it('records one daily attempt and exposes only Tauri-accepted available metadata', async () => {
    const now = 1_786_320_000_000
    const operations = createOperations()
    const controller = useStartupUpdate({ desktopRuntime: true, operations, now: () => now })

    await runScheduledCheck(controller)

    expect(operations.check).toHaveBeenCalledOnce()
    expect(window.localStorage.getItem(STARTUP_UPDATE_STORAGE_KEY)).toBe(String(now))
    expect(controller.report.value).toEqual(availableUpdate)
  })

  it('checks local status but skips the network endpoint inside the 24-hour window', async () => {
    const now = 1_786_320_000_000
    window.localStorage.setItem(
      STARTUP_UPDATE_STORAGE_KEY,
      String(now - STARTUP_UPDATE_CHECK_INTERVAL_MS + 1),
    )
    const operations = createOperations()
    const controller = useStartupUpdate({ desktopRuntime: true, operations, now: () => now })

    await runScheduledCheck(controller)

    expect(operations.status).toHaveBeenCalledOnce()
    expect(operations.check).not.toHaveBeenCalled()
    expect(controller.report.value).toBeUndefined()
  })

  it.each([
    ['expired', String(1_786_320_000_000 - STARTUP_UPDATE_CHECK_INTERVAL_MS)],
    ['malformed', 'not-a-time'],
    ['future', String(1_786_320_000_000 + 1)],
  ])('rechecks when the stored timestamp is %s', async (_case, storedValue) => {
    const now = 1_786_320_000_000
    window.localStorage.setItem(STARTUP_UPDATE_STORAGE_KEY, storedValue)
    const operations = createOperations()
    const controller = useStartupUpdate({ desktopRuntime: true, operations, now: () => now })

    await runScheduledCheck(controller)

    expect(operations.check).toHaveBeenCalledOnce()
    expect(window.localStorage.getItem(STARTUP_UPDATE_STORAGE_KEY)).toBe(String(now))
  })

  it('keeps latest-version and failed automatic checks silent', async () => {
    const latestOperations = createOperations()
    latestOperations.check.mockResolvedValue(success({
      available: false,
      currentVersion,
      version: null,
      publishedAt: null,
    }))
    const latest = useStartupUpdate({ desktopRuntime: true, operations: latestOperations })
    await runScheduledCheck(latest)
    expect(latest.report.value).toBeUndefined()
    expect(latest.message.value).toBe('')

    window.localStorage.clear()
    const failedOperations = createOperations()
    failedOperations.check.mockResolvedValue(failure(
      'update_check_failed',
      '暂时无法检查更新，请稍后重试。',
      true,
      'private-diagnostic',
    ))
    const failed = useStartupUpdate({ desktopRuntime: true, operations: failedOperations })
    await runScheduledCheck(failed)
    expect(failed.report.value).toBeUndefined()
    expect(failed.message.value).toBe('')
  })

  it('installs only the displayed version and coalesces duplicate clicks', async () => {
    let resolveInstall!: (result: ReturnType<typeof success<{ acceptedVersion: string }>>) => void
    const operations = createOperations()
    operations.install.mockReturnValue(new Promise(resolve => { resolveInstall = resolve }))
    const controller = useStartupUpdate({ desktopRuntime: true, operations })
    await runScheduledCheck(controller)

    const first = controller.install()
    const second = controller.install()

    expect(first).toBe(second)
    expect(operations.install).toHaveBeenCalledOnce()
    expect(operations.install).toHaveBeenCalledWith('0.2.0')
    expect(controller.installing.value).toBe(true)
    resolveInstall(success({ acceptedVersion: '0.2.0' }))
    await first
    expect(controller.message.value).toBe('安装程序已启动；请按系统提示完成更新。')
  })

  it('refreshes once when the available version changes before installation', async () => {
    const operations = createOperations()
    operations.check
      .mockResolvedValueOnce(success(availableUpdate))
      .mockResolvedValueOnce(success({ ...availableUpdate, version: '0.3.0' }))
    operations.install.mockResolvedValue(failure(
      'update_version_changed',
      '可用版本已经变化，请重新检查。',
      true,
      'version-changed',
    ))
    const controller = useStartupUpdate({ desktopRuntime: true, operations })
    await runScheduledCheck(controller)

    await controller.install()

    expect(operations.install).toHaveBeenCalledWith('0.2.0')
    expect(operations.check).toHaveBeenCalledTimes(2)
    expect(controller.report.value?.version).toBe('0.3.0')
    expect(controller.message.value).toBe('')
  })

  it('cancels the delayed check when the application unmounts', async () => {
    const operations = createOperations()
    const controller = useStartupUpdate({ desktopRuntime: true, operations })
    controller.start()
    controller.dispose()

    await vi.advanceTimersByTimeAsync(STARTUP_UPDATE_DELAY_MS)

    expect(operations.status).not.toHaveBeenCalled()
  })
})
