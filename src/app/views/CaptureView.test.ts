import { fireEvent, render, screen, waitFor } from '@testing-library/vue'
import { beforeEach, describe, expect, it, vi } from 'vitest'
import CaptureView from './CaptureView.vue'

const api = vi.hoisted(() => ({
  captureBatchList: vi.fn(),
  captureBatchDetail: vi.fn(),
  captureLanAddresses: vi.fn(),
  captureLanPreflight: vi.fn(),
  captureLanFirewallRepair: vi.fn(),
  captureLanStart: vi.fn(),
  captureLanStatus: vi.fn(),
}))

vi.mock('@tauri-apps/api/core', () => ({ isTauri: () => true }))
vi.mock('@tauri-apps/api/event', () => ({ listen: vi.fn().mockResolvedValue(vi.fn()) }))
vi.mock('../../shared/api/bindings', () => ({ commands: api }))
vi.mock('../../modules/capture/components/CaptureWorkspace.vue', () => ({
  default: {
    props: ['batches', 'detail', 'busy', 'errorMessage', 'lanSession'],
    emits: ['openBatch', 'mobileCapture'],
    template: `
      <div>
        <button @click="$emit('openBatch', 'batch-1')">open batch</button>
        <button :disabled="busy" @click="$emit('mobileCapture', '192.168.1.20')">scan</button>
        <span data-testid="error">{{ errorMessage }}</span>
        <span data-testid="session">{{ lanSession?.sessionId ?? 'none' }}</span>
      </div>
    `,
  },
}))

const batch = {
  id: 'batch-1', subject: '数学', state: 'collecting', itemCount: 0,
  draftCount: 0, readyCount: 0, updatedAtUtcMs: 1, revision: 1,
}
const detail = { batch, items: [], drafts: [], unassignedItemIds: [] }
const missingRule = {
  supported: true,
  activeProfiles: ['public'],
  firewallRule: 'missing',
  canStart: false,
  needsNetworkChange: false,
  needsFirewallRepair: true,
}
const readyRule = {
  ...missingRule,
  firewallRule: 'ready',
  canStart: true,
  needsFirewallRepair: false,
}
const session = {
  sessionId: 'session-1', batchId: 'batch-1', qrSvgDataUrl: 'data:image/svg+xml,test',
  selectedAddress: '192.168.1.20', expiresAtUtcMs: 100_000,
  receivedItemCount: 0, receivedBytes: 0,
}

function success<T>(data: T) {
  return { ok: true, data }
}

function failure(userMessage: string) {
  return {
    ok: false,
    error: { code: 'capture_lan_firewall_cancelled', userMessage, retryable: true, diagnosticId: 'diag-1' },
  }
}

async function openBatchAndScan() {
  await fireEvent.click(screen.getByRole('button', { name: 'open batch' }))
  await waitFor(() => expect(api.captureBatchDetail).toHaveBeenCalledWith('batch-1'))
  await fireEvent.click(screen.getByRole('button', { name: 'scan' }))
}

beforeEach(() => {
  vi.clearAllMocks()
  api.captureBatchList.mockResolvedValue(success([batch]))
  api.captureBatchDetail.mockResolvedValue(success(detail))
  api.captureLanAddresses.mockResolvedValue(success([{ label: 'Wi-Fi', address: '192.168.1.20' }]))
  api.captureLanStatus.mockResolvedValue(success(null))
  api.captureLanStart.mockResolvedValue(success(session))
})

describe('CaptureView one-time Windows LAN permission', () => {
  it('repairs once on the first scan and immediately starts the QR session', async () => {
    api.captureLanPreflight.mockResolvedValue(success(missingRule))
    api.captureLanFirewallRepair.mockResolvedValue(success(readyRule))
    render(CaptureView)

    await openBatchAndScan()

    await waitFor(() => expect(api.captureLanFirewallRepair).toHaveBeenCalledOnce())
    await waitFor(() => expect(api.captureLanStart).toHaveBeenCalledWith({
        batchId: 'batch-1',
        selectedAddress: '192.168.1.20',
      }))
    expect(await screen.findByTestId('session')).toHaveTextContent('session-1')
  })

  it('starts directly without authorization when the persistent rule is ready', async () => {
    api.captureLanPreflight.mockResolvedValue(success(readyRule))
    render(CaptureView)

    await openBatchAndScan()

    await waitFor(() => expect(api.captureLanStart).toHaveBeenCalledOnce())
    expect(api.captureLanFirewallRepair).not.toHaveBeenCalled()
    expect(screen.getByTestId('session')).toHaveTextContent('session-1')
  })

  it('does not mark cancellation as success and retries authorization on the next scan', async () => {
    api.captureLanPreflight.mockResolvedValue(success(missingRule))
    api.captureLanFirewallRepair
      .mockResolvedValueOnce(failure('没有更改 Windows 权限；下次扫码会再次请求。'))
      .mockResolvedValueOnce(success(readyRule))
    render(CaptureView)

    await openBatchAndScan()
    await waitFor(() => expect(api.captureLanFirewallRepair).toHaveBeenCalledTimes(1))
    expect(api.captureLanStart).not.toHaveBeenCalled()
    await waitFor(() => expect(screen.getByTestId('error')).toHaveTextContent('下次扫码会再次请求'))

    await waitFor(() => expect(screen.getByRole('button', { name: 'scan' })).toBeEnabled())
    await fireEvent.click(screen.getByRole('button', { name: 'scan' }))

    await waitFor(() => expect(api.captureLanFirewallRepair).toHaveBeenCalledTimes(2))
    await waitFor(() => expect(api.captureLanStart).toHaveBeenCalledOnce())
    await waitFor(() => expect(screen.getByTestId('session')).toHaveTextContent('session-1'))
  })
})
