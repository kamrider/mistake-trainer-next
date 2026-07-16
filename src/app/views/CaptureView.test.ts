import { fireEvent, render, screen, waitFor } from '@testing-library/vue'
import { afterEach, beforeEach, describe, expect, it, vi } from 'vitest'
import CaptureView from './CaptureView.vue'

const api = vi.hoisted(() => ({
  captureBatchList: vi.fn(),
  captureLanAddresses: vi.fn(),
  captureLanPreflight: vi.fn(),
  captureLanStatus: vi.fn(),
  captureLanOpenNetworkSettings: vi.fn(),
}))

vi.mock('@tauri-apps/api/core', () => ({ isTauri: () => true }))
vi.mock('@tauri-apps/api/event', () => ({ listen: vi.fn().mockResolvedValue(vi.fn()) }))
vi.mock('../../shared/api/bindings', () => ({ commands: api }))
vi.mock('../../modules/capture/components/CaptureWorkspace.vue', () => ({
  default: {
    props: ['lanAddresses', 'lanPreflight', 'lanPreflightBusy', 'lanGuidePolling'],
    emits: ['refreshLanPreflight', 'openLanNetworkSettings'],
    template: `
      <div>
        <span data-testid="addresses">{{ lanAddresses.map(address => address.address).join(',') }}</span>
        <span data-testid="preflight">{{ lanPreflight?.needsNetworkChange ? 'public' : 'private' }}</span>
        <span data-testid="busy">{{ lanPreflightBusy ? 'busy' : 'idle' }}</span>
        <span data-testid="polling">{{ lanGuidePolling ? 'polling' : 'stopped' }}</span>
        <button @click="$emit('refreshLanPreflight')">manual refresh</button>
        <button @click="$emit('openLanNetworkSettings', 'wifi')">open settings</button>
      </div>
    `,
  },
}))

const publicPreflight = {
  supported: true,
  activeProfiles: ['public'],
  firewallRule: 'ready',
  canStart: false,
  needsNetworkChange: true,
  needsFirewallRepair: false,
}

const privatePreflight = {
  ...publicPreflight,
  activeProfiles: ['private'],
  canStart: true,
  needsNetworkChange: false,
}

function resolved<T>(data: T) {
  return { ok: true, data }
}

beforeEach(() => {
  vi.clearAllMocks()
  api.captureBatchList.mockResolvedValue(resolved([]))
  api.captureLanStatus.mockResolvedValue(resolved(null))
  api.captureLanOpenNetworkSettings.mockResolvedValue(resolved(true))
})

afterEach(() => vi.useRealTimers())

describe('CaptureView Windows LAN guide', () => {
  it('refreshes network addresses after a manual public-to-hotspot transition', async () => {
    api.captureLanPreflight
      .mockResolvedValueOnce(resolved(publicPreflight))
      .mockResolvedValueOnce(resolved(privatePreflight))
    api.captureLanAddresses
      .mockResolvedValueOnce(resolved([{ label: 'old Wi-Fi', address: '192.168.1.20' }]))
      .mockResolvedValueOnce(resolved([{ label: 'phone hotspot', address: '172.20.10.2' }]))

    render(CaptureView)
    expect(await screen.findByText('public')).toBeVisible()
    expect(screen.getByTestId('addresses')).toHaveTextContent('192.168.1.20')

    await fireEvent.click(screen.getByRole('button', { name: 'manual refresh' }))

    await waitFor(() => expect(screen.getByTestId('preflight')).toHaveTextContent('private'))
    await waitFor(() => expect(screen.getByTestId('addresses')).toHaveTextContent('172.20.10.2'))
    expect(api.captureLanAddresses).toHaveBeenCalledTimes(2)
  })

  it('keeps polling silent, serial, and unable to apply after unmount', async () => {
    let resolvePoll!: (value: ReturnType<typeof resolved>) => void
    const pendingPoll = new Promise<ReturnType<typeof resolved>>(resolve => {
      resolvePoll = resolve
    })
    api.captureLanPreflight
      .mockResolvedValueOnce(resolved(publicPreflight))
      .mockReturnValueOnce(pendingPoll)
    api.captureLanAddresses.mockResolvedValue(resolved([{ label: 'Wi-Fi', address: '192.168.1.20' }]))

    const view = render(CaptureView)
    expect(await screen.findByText('public')).toBeVisible()
    vi.useFakeTimers()
    await fireEvent.click(screen.getByRole('button', { name: 'open settings' }))
    expect(screen.getByTestId('polling')).toHaveTextContent('polling')

    await vi.advanceTimersByTimeAsync(2_000)
    expect(api.captureLanPreflight).toHaveBeenCalledTimes(2)
    expect(screen.getByTestId('busy')).toHaveTextContent('idle')
    await vi.advanceTimersByTimeAsync(10_000)
    expect(api.captureLanPreflight).toHaveBeenCalledTimes(2)

    view.unmount()
    resolvePoll(resolved(privatePreflight))
    await Promise.resolve()
    await Promise.resolve()

    expect(api.captureLanAddresses).toHaveBeenCalledTimes(1)
  })

  it('expires a pending background check without applying its stale result', async () => {
    let resolvePoll!: (value: ReturnType<typeof resolved>) => void
    const pendingPoll = new Promise<ReturnType<typeof resolved>>(resolve => {
      resolvePoll = resolve
    })
    api.captureLanPreflight
      .mockResolvedValueOnce(resolved(publicPreflight))
      .mockReturnValueOnce(pendingPoll)
    api.captureLanAddresses.mockResolvedValue(resolved([{ label: 'Wi-Fi', address: '192.168.1.20' }]))

    const view = render(CaptureView)
    expect(await screen.findByText('public')).toBeVisible()
    vi.useFakeTimers()
    await fireEvent.click(screen.getByRole('button', { name: 'open settings' }))
    await vi.advanceTimersByTimeAsync(90_000)
    expect(api.captureLanPreflight).toHaveBeenCalledTimes(2)
    expect(screen.getByTestId('polling')).toHaveTextContent('stopped')

    resolvePoll(resolved(privatePreflight))
    await vi.advanceTimersByTimeAsync(0)

    expect(screen.getByTestId('preflight')).toHaveTextContent('public')
    expect(api.captureLanAddresses).toHaveBeenCalledTimes(1)
    view.unmount()
  })

  it('runs a fresh check when polling is restarted during an older request', async () => {
    let resolveOldPoll!: (value: ReturnType<typeof resolved>) => void
    const oldPoll = new Promise<ReturnType<typeof resolved>>(resolve => {
      resolveOldPoll = resolve
    })
    api.captureLanPreflight
      .mockResolvedValueOnce(resolved(publicPreflight))
      .mockReturnValueOnce(oldPoll)
      .mockResolvedValueOnce(resolved(privatePreflight))
    api.captureLanAddresses
      .mockResolvedValueOnce(resolved([{ label: 'old Wi-Fi', address: '192.168.1.20' }]))
      .mockResolvedValueOnce(resolved([{ label: 'new hotspot', address: '172.20.10.2' }]))

    const view = render(CaptureView)
    expect(await screen.findByText('public')).toBeVisible()
    vi.useFakeTimers()
    await fireEvent.click(screen.getByRole('button', { name: 'open settings' }))
    await vi.advanceTimersByTimeAsync(2_000)
    expect(api.captureLanPreflight).toHaveBeenCalledTimes(2)

    await fireEvent.click(screen.getByRole('button', { name: 'open settings' }))
    await vi.advanceTimersByTimeAsync(2_000)
    expect(api.captureLanPreflight).toHaveBeenCalledTimes(2)
    resolveOldPoll(resolved(publicPreflight))
    await vi.advanceTimersByTimeAsync(0)

    await vi.waitFor(() => expect(api.captureLanPreflight).toHaveBeenCalledTimes(3))
    await vi.waitFor(() => expect(screen.getByTestId('preflight')).toHaveTextContent('private'))
    expect(screen.getByTestId('addresses')).toHaveTextContent('172.20.10.2')
    expect(screen.getByTestId('polling')).toHaveTextContent('stopped')
    view.unmount()
  })
})
