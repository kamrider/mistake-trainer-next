import { ref } from 'vue'
import { afterEach, describe, expect, it, vi } from 'vitest'
import { failure, success, type AppResult } from '../../shared/api/app-result'
import type { CloudAuthState, SyncNowReport } from '../../shared/api/bindings'
import type { AppPage } from '../AppShell.vue'
import type { LibraryAccessPhase } from './useLibraryAccessLifecycle'
import {
  useApplicationSyncLifecycle,
  type ApplicationSyncLifecycleOptions,
} from './useApplicationSyncLifecycle'

const report: SyncNowReport = {
  pushedOperationCount: 1,
  uploadedAssetCount: 0,
  pulledChangeCount: 2,
  downloadedAssetCount: 0,
  finalCursor: 3,
}

const connected: CloudAuthState = {
  configured: true,
  status: { kind: 'connected', emailHint: 's***@example.test' },
}

function deferred<T>() {
  let resolve!: (value: T) => void
  const promise = new Promise<T>((resolvePromise) => {
    resolve = resolvePromise
  })
  return { promise, resolve }
}

function createOptions(
  overrides: Partial<ApplicationSyncLifecycleOptions> = {},
): ApplicationSyncLifecycleOptions {
  return {
    desktopRuntime: true,
    libraryAccessPhase: ref<LibraryAccessPhase>('unlocked'),
    workspaceInitialized: ref(true),
    activePage: ref<AppPage>('dashboard'),
    restoreSession: vi.fn().mockResolvedValue(success(connected)),
    syncNow: vi.fn().mockResolvedValue(success(report)),
    onSyncSuccess: vi.fn().mockResolvedValue(undefined),
    ...overrides,
  }
}

describe('useApplicationSyncLifecycle', () => {
  afterEach(() => {
    vi.restoreAllMocks()
  })

  it('restores a connected session, synchronizes, and reports successful work', async () => {
    const options = createOptions()
    const lifecycle = useApplicationSyncLifecycle(options)

    await lifecycle.restoreCloudAndSync('startup')

    expect(lifecycle.phase.value).toBe('synced')
    expect(options.syncNow).toHaveBeenCalledOnce()
    expect(options.onSyncSuccess).toHaveBeenCalledWith(report, 'startup')
  })

  it.each([
    [{ configured: true, status: { kind: 'offline', emailHint: 's***@test' } }, 'offline'],
    [{ configured: false, status: { kind: 'unconfigured', emailHint: null } }, 'local_only'],
    [{ configured: true, status: { kind: 'signed_out', emailHint: null } }, 'signed_out'],
    [{ configured: true, status: { kind: 'verification_required', emailHint: 's***@test' } }, 'signed_out'],
  ] as const)('maps auth state %# without starting sync', async (authState, phase) => {
    const options = createOptions({
      restoreSession: vi.fn().mockResolvedValue(success(authState as CloudAuthState)),
    })
    const lifecycle = useApplicationSyncLifecycle(options)

    await lifecycle.restoreCloudAndSync('startup')

    expect(lifecycle.phase.value).toBe(phase)
    expect(options.syncNow).not.toHaveBeenCalled()
  })

  it.each([
    ['AUTH_NETWORK', 'offline'],
    ['AUTH_TIMEOUT', 'offline'],
    ['cloud_network', 'offline'],
    ['cloud_timeout', 'offline'],
    ['cloud_unavailable', 'offline'],
    ['AUTH_COMMAND_UNAVAILABLE', 'retry_waiting'],
  ] as const)('classifies restore failure %s as %s', async (code, phase) => {
    const options = createOptions({
      restoreSession: vi.fn().mockResolvedValue(failure(code, 'restore failed', true, code)),
    })
    const lifecycle = useApplicationSyncLifecycle(options)

    await lifecycle.restoreCloudAndSync('startup')

    expect(lifecycle.phase.value).toBe(phase)
  })

  it.each([
    ['SYNC_CAPTURE_ACTIVE', 'deferred_capture'],
    ['SYNC_ALREADY_RUNNING', 'syncing'],
    ['cloud_network', 'offline'],
    ['SYNC_COMMAND_UNAVAILABLE', 'retry_waiting'],
  ] as const)('maps sync failure %s to %s and preserves the result', async (code, phase) => {
    const syncFailure = failure(code, 'sync failed', true, code)
    const options = createOptions({ syncNow: vi.fn().mockResolvedValue(syncFailure) })
    const lifecycle = useApplicationSyncLifecycle(options)

    const result = await lifecycle.controller.run('manual')

    expect(result).toEqual(syncFailure)
    expect(lifecycle.phase.value).toBe(phase)
  })

  it('coalesces concurrent restore triggers and accepts a later trigger', async () => {
    const pending = deferred<AppResult<CloudAuthState>>()
    const restoreSession = vi.fn().mockReturnValueOnce(pending.promise).mockResolvedValue(success(connected))
    const options = createOptions({ restoreSession })
    const lifecycle = useApplicationSyncLifecycle(options)

    const first = lifecycle.restoreCloudAndSync('startup')
    const second = lifecycle.restoreCloudAndSync('online')
    expect(restoreSession).toHaveBeenCalledOnce()
    pending.resolve(success(connected))
    await Promise.all([first, second])

    await lifecycle.restoreCloudAndSync('online')
    expect(restoreSession).toHaveBeenCalledTimes(2)
  })

  it('keeps browser preview local-only without invoking desktop operations', async () => {
    const options = createOptions({ desktopRuntime: false })
    const lifecycle = useApplicationSyncLifecycle(options)

    await lifecycle.restoreCloudAndSync('startup')

    expect(lifecycle.phase.value).toBe('local_only')
    expect(options.restoreSession).not.toHaveBeenCalled()
    expect(options.syncNow).not.toHaveBeenCalled()
  })

  it('returns a stable offline failure when the sync operation throws', async () => {
    const options = createOptions({
      syncNow: vi.fn().mockRejectedValue(new Error('command unavailable')),
    })
    const lifecycle = useApplicationSyncLifecycle(options)

    const result = await lifecycle.controller.run('manual')

    expect(result).toEqual(failure(
      'SYNC_REQUEST_FAILED',
      '暂时无法连接云端，本地内容已经保存并会等待重试。',
      true,
      'sync-request-failed',
    ))
    expect(lifecycle.phase.value).toBe('offline')
  })

  it('ignores browser triggers until the local workspace is unlocked and initialized', async () => {
    const libraryAccessPhase = ref<LibraryAccessPhase>('locked')
    const workspaceInitialized = ref(false)
    const options = createOptions({ libraryAccessPhase, workspaceInitialized })
    const lifecycle = useApplicationSyncLifecycle(options)
    lifecycle.start()

    window.dispatchEvent(new Event('online'))
    await Promise.resolve()
    expect(options.restoreSession).not.toHaveBeenCalled()

    libraryAccessPhase.value = 'unlocked'
    window.dispatchEvent(new Event('online'))
    await Promise.resolve()
    expect(options.restoreSession).not.toHaveBeenCalled()

    workspaceInitialized.value = true
    window.dispatchEvent(new Event('online'))
    await vi.waitFor(() => expect(options.restoreSession).toHaveBeenCalledOnce())
    lifecycle.dispose()
  })

  it('owns guarded browser triggers, visibility cooldown, and disposal', async () => {
    let now = 1_000
    const options = createOptions({ now: () => now })
    const visibility = vi.spyOn(document, 'visibilityState', 'get').mockReturnValue('visible')
    const lifecycle = useApplicationSyncLifecycle(options)
    lifecycle.start()
    await lifecycle.restoreCloudAndSync('startup')
    expect(options.restoreSession).toHaveBeenCalledOnce()

    document.dispatchEvent(new Event('visibilitychange'))
    await Promise.resolve()
    expect(options.restoreSession).toHaveBeenCalledOnce()

    now += 15_001
    document.dispatchEvent(new Event('visibilitychange'))
    await vi.waitFor(() => expect(options.restoreSession).toHaveBeenCalledTimes(2))

    lifecycle.dispose()
    window.dispatchEvent(new Event('online'))
    await Promise.resolve()
    expect(options.restoreSession).toHaveBeenCalledTimes(2)
    visibility.mockRestore()
  })
})
