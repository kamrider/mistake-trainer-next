import { describe, expect, it, vi } from 'vitest'
import type {
  CaptureLanAddress,
  CaptureLanPreflight,
  CaptureLanSession,
} from '../../../shared/api/bindings'
import {
  failure,
  success,
  type AppResult,
} from '../../../shared/api/app-result'
import {
  useCaptureLanSession,
  type CaptureLanOperations,
} from './useCaptureLanSession'

function deferred<T>() {
  let resolve!: (value: T) => void
  const promise = new Promise<T>((finish) => {
    resolve = finish
  })
  return { promise, resolve }
}

const addresses: CaptureLanAddress[] = [
  { label: 'Wi-Fi', address: '192.168.1.20' },
  { label: 'Ethernet', address: '10.0.0.8' },
]

const missingRule: CaptureLanPreflight = {
  supported: true,
  activeProfiles: ['public'],
  firewallRule: 'missing',
  canStart: false,
  needsNetworkChange: false,
  needsFirewallRepair: true,
}

const readyRule: CaptureLanPreflight = {
  ...missingRule,
  firewallRule: 'ready',
  canStart: true,
  needsFirewallRepair: false,
}

const session: CaptureLanSession = {
  sessionId: 'session-1',
  batchId: 'batch-1',
  qrSvgDataUrl: 'data:image/svg+xml,test',
  selectedAddress: '192.168.1.20',
  expiresAtUtcMs: 100_000,
  receivedItemCount: 0,
  receivedBytes: 0,
}

function createHarness(overrides: Partial<CaptureLanOperations> = {}) {
  let activeBatchId: string | undefined = 'batch-1'
  let blocked = false
  const operations: CaptureLanOperations = {
    addresses: vi.fn().mockResolvedValue(success(addresses)),
    preflight: vi.fn().mockResolvedValue(success(readyRule)),
    repair: vi.fn().mockResolvedValue(success(readyRule)),
    status: vi.fn().mockResolvedValue(success(null)),
    start: vi.fn().mockResolvedValue(success(session)),
    stop: vi.fn().mockResolvedValue(success(true)),
    ...overrides,
  }
  const onError = vi.fn()
  const onBusyChange = vi.fn((value: boolean) => {
    blocked = value
  })
  const controller = useCaptureLanSession({
    desktopAvailable: true,
    activeBatchId: () => activeBatchId,
    isBlocked: () => blocked,
    onBusyChange,
    onError,
    operations,
  })
  return {
    controller,
    operations,
    onError,
    onBusyChange,
    setActiveBatchId: (value: string | undefined) => { activeBatchId = value },
  }
}

describe('useCaptureLanSession', () => {
  it('coalesces concurrent preflight reads', async () => {
    const gate = deferred<AppResult<CaptureLanPreflight>>()
    const harness = createHarness({
      preflight: vi.fn().mockReturnValue(gate.promise),
    })

    const first = harness.controller.loadPreflight()
    const second = harness.controller.loadPreflight()
    expect(harness.operations.preflight).toHaveBeenCalledOnce()

    gate.resolve(success(readyRule))

    await expect(Promise.all([first, second])).resolves.toEqual([readyRule, readyRule])
    expect(harness.controller.preflight.value).toEqual(readyRule)
    expect(harness.controller.preflightBusy.value).toBe(false)
  })

  it('coalesces concurrent status reads', async () => {
    const gate = deferred<AppResult<CaptureLanSession | null>>()
    const harness = createHarness({
      status: vi.fn().mockReturnValue(gate.promise),
    })

    const first = harness.controller.loadStatus()
    const second = harness.controller.loadStatus()
    expect(harness.operations.status).toHaveBeenCalledOnce()

    gate.resolve(success(session))
    await Promise.all([first, second])

    expect(harness.controller.session.value).toEqual(session)
  })

  it('repairs once and starts with a validated address', async () => {
    const harness = createHarness({
      preflight: vi.fn().mockResolvedValue(success(missingRule)),
      repair: vi.fn().mockResolvedValue(success(readyRule)),
    })

    await harness.controller.start('192.168.1.20')

    expect(harness.operations.repair).toHaveBeenCalledOnce()
    expect(harness.operations.start).toHaveBeenCalledWith({
      batchId: 'batch-1',
      selectedAddress: '192.168.1.20',
    })
    expect(harness.controller.session.value).toEqual(session)
    expect(harness.onBusyChange.mock.calls).toEqual([[true], [false]])
  })

  it('does not treat permission cancellation as success and retries it later', async () => {
    const repair = vi.fn()
      .mockResolvedValueOnce(failure(
        'capture_lan_firewall_cancelled',
        '没有更改 Windows 权限；下次扫码会再次请求。',
        true,
        'diag-1',
      ))
      .mockResolvedValueOnce(success(readyRule))
    const harness = createHarness({
      preflight: vi.fn().mockResolvedValue(success(missingRule)),
      repair,
    })

    await harness.controller.start('192.168.1.20')
    expect(harness.operations.start).not.toHaveBeenCalled()
    expect(harness.onError).toHaveBeenCalledWith('没有更改 Windows 权限；下次扫码会再次请求。')

    await harness.controller.start('192.168.1.20')
    expect(repair).toHaveBeenCalledTimes(2)
    expect(harness.operations.start).toHaveBeenCalledOnce()
  })

  it('does not restore a stopped session from an older status response', async () => {
    const staleGate = deferred<AppResult<CaptureLanSession | null>>()
    const status = vi.fn()
      .mockResolvedValueOnce(success(session))
      .mockReturnValueOnce(staleGate.promise)
    const harness = createHarness({ status })

    await harness.controller.loadStatus()
    const stalePoll = harness.controller.loadStatus()
    expect(harness.controller.session.value).toEqual(session)

    await harness.controller.stop()
    expect(harness.controller.session.value).toBeUndefined()
    const pollAfterStop = harness.controller.loadStatus()
    expect(status).toHaveBeenCalledTimes(2)
    staleGate.resolve(success(session))
    await Promise.all([stalePoll, pollAfterStop])

    expect(harness.controller.session.value).toBeUndefined()
  })

  it('does not start for a batch that changed during preflight', async () => {
    const preflightGate = deferred<AppResult<CaptureLanPreflight>>()
    const harness = createHarness({
      preflight: vi.fn().mockReturnValue(preflightGate.promise),
    })

    const starting = harness.controller.start('192.168.1.20')
    harness.setActiveBatchId('batch-2')
    preflightGate.resolve(success(readyRule))
    await starting

    expect(harness.operations.start).not.toHaveBeenCalled()
    expect(harness.controller.session.value).toBeUndefined()
  })

  it('ignores late work after disposal', async () => {
    const preflightGate = deferred<AppResult<CaptureLanPreflight>>()
    const statusGate = deferred<AppResult<CaptureLanSession | null>>()
    const harness = createHarness({
      preflight: vi.fn().mockReturnValue(preflightGate.promise),
      status: vi.fn().mockReturnValue(statusGate.promise),
    })

    const loadingPreflight = harness.controller.loadPreflight()
    const loadingStatus = harness.controller.loadStatus()
    harness.controller.dispose()
    preflightGate.resolve(failure('offline', '不应显示', true, 'diag-2'))
    statusGate.resolve(success(session))
    await Promise.all([loadingPreflight, loadingStatus])

    expect(harness.controller.preflight.value).toBeUndefined()
    expect(harness.controller.preflightBusy.value).toBe(false)
    expect(harness.controller.session.value).toBeUndefined()
    expect(harness.onError).not.toHaveBeenCalled()
  })

  it('stops scheduled status polling after disposal', async () => {
    vi.useFakeTimers()
    try {
      const harness = createHarness()
      harness.controller.startPolling(1_000)

      await vi.advanceTimersByTimeAsync(1_000)
      expect(harness.operations.status).toHaveBeenCalledOnce()

      harness.controller.dispose()
      await vi.advanceTimersByTimeAsync(5_000)
      expect(harness.operations.status).toHaveBeenCalledOnce()
    }
    finally {
      vi.useRealTimers()
    }
  })
})
