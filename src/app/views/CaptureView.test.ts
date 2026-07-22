import { fireEvent, render, screen, waitFor } from '@testing-library/vue'
import { beforeEach, describe, expect, it, vi } from 'vitest'
import CaptureView from './CaptureView.vue'

const api = vi.hoisted(() => ({
  captureBatchList: vi.fn(),
  captureBatchDetail: vi.fn(),
  captureImportBytes: vi.fn(),
  captureLanAddresses: vi.fn(),
  captureLanPreflight: vi.fn(),
  captureLanFirewallRepair: vi.fn(),
  captureLanStart: vi.fn(),
  captureLanStatus: vi.fn(),
  captureCardMerge: vi.fn(),
  captureDraftDelete: vi.fn(),
  captureItemMove: vi.fn(),
  captureBatchAssignSubject: vi.fn(),
  subjectPreferencesGet: vi.fn(),
}))

vi.mock('@tauri-apps/api/core', () => ({ isTauri: () => true }))
vi.mock('@tauri-apps/api/event', () => ({ listen: vi.fn().mockResolvedValue(vi.fn()) }))
vi.mock('../../shared/api/bindings', () => ({ commands: api }))
vi.mock('../../modules/capture/components/CaptureWorkspace.vue', () => ({
  default: {
    props: ['batches', 'detail', 'busy', 'errorMessage', 'lanSession', 'subjectOptions'],
    emits: ['openBatch', 'mobileCapture', 'mergeCard', 'deleteDraft', 'moveItem', 'assignBatchSubject', 'importFiles'],
    setup(_props: Record<string, unknown>, { emit }: { emit: (event: string, ...args: unknown[]) => void }) {
      const emitFiles = () => emit('importFiles', [
        { name: 'bad.png', arrayBuffer: async () => new Uint8Array([1]).buffer },
        { name: 'good.png', arrayBuffer: async () => new Uint8Array([2]).buffer },
      ])
      const emitOverflow = () => emit('importFiles', Array.from({ length: 151 }, (_, index) => ({
        name: `image-${index + 1}.png`,
        arrayBuffer: async () => new Uint8Array([index & 255]).buffer,
      })))
      return { emitFiles, emitOverflow }
    },
    template: `
      <div>
        <button @click="$emit('openBatch', 'batch-1')">open batch</button>
        <button @click="emitFiles">import files</button>
        <button @click="emitOverflow">import overflow</button>
        <button :disabled="busy" @click="$emit('mobileCapture', '192.168.1.20')">scan</button>
        <button :disabled="busy" @click="$emit('mergeCard', ['item-1'], null, '数学')">merge card</button>
        <button :disabled="busy" @click="$emit('deleteDraft', 'draft-1')">delete card</button>
        <button :disabled="busy" @click="$emit('moveItem', { itemId: 'item-1', targetDraftId: 'draft-1', targetRole: 'answer', targetPosition: 0 })">change role</button>
        <button :disabled="busy" @click="$emit('assignBatchSubject', '化学')">assign subject</button>
        <span data-testid="subjects">{{ subjectOptions?.join('、') }}</span>
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
  api.captureImportBytes.mockResolvedValue(success({ ...detail, items: [] }))
  api.captureLanAddresses.mockResolvedValue(success([{ label: 'Wi-Fi', address: '192.168.1.20' }]))
  api.captureLanStatus.mockResolvedValue(success(null))
  api.captureLanStart.mockResolvedValue(success(session))
  api.captureCardMerge.mockResolvedValue(success(detail))
  api.captureDraftDelete.mockResolvedValue(success(detail))
  api.captureItemMove.mockResolvedValue(success(detail))
  api.captureBatchAssignSubject.mockResolvedValue(success({ ...detail, batch: { ...batch, subject: '化学', revision: 2 } }))
  api.subjectPreferencesGet.mockResolvedValue(success({
    enabledSubjects: ['数学', '化学'], customSubjects: [], captureSoundEnabled: true,
  }))
})

describe('CaptureView one-time Windows LAN permission', () => {
  it('persists card creation, in-card role changes, and reversible deletion', async () => {
    api.captureLanPreflight.mockResolvedValue(success(readyRule))
    render(CaptureView)
    await fireEvent.click(screen.getByRole('button', { name: 'open batch' }))
    await waitFor(() => expect(api.captureBatchDetail).toHaveBeenCalledWith('batch-1'))

    await fireEvent.click(screen.getByRole('button', { name: 'merge card' }))
    await waitFor(() => expect(api.captureCardMerge).toHaveBeenCalledWith({
      batchId: 'batch-1',
      expectedRevision: 1,
      targetDraftId: null,
      itemIds: ['item-1'],
      newDraftSubject: '数学',
    }))

    await waitFor(() => expect(screen.getByRole('button', { name: 'change role' })).toBeEnabled())
    await fireEvent.click(screen.getByRole('button', { name: 'change role' }))
    await waitFor(() => expect(api.captureItemMove).toHaveBeenCalledWith({
      batchId: 'batch-1',
      expectedRevision: 1,
      itemId: 'item-1',
      targetDraftId: 'draft-1',
      targetRole: 'answer',
      targetPosition: 0,
    }))

    await waitFor(() => expect(screen.getByRole('button', { name: 'delete card' })).toBeEnabled())
    await fireEvent.click(screen.getByRole('button', { name: 'delete card' }))
    await waitFor(() => expect(api.captureDraftDelete).toHaveBeenCalledWith('batch-1', 1, 'draft-1'))
  })

  it('loads configured subjects and persists whole-batch assignment', async () => {
    api.captureLanPreflight.mockResolvedValue(success(readyRule))
    render(CaptureView)
    await waitFor(() => expect(screen.getByTestId('subjects')).toHaveTextContent('数学、化学'))
    await fireEvent.click(screen.getByRole('button', { name: 'open batch' }))
    await waitFor(() => expect(api.captureBatchDetail).toHaveBeenCalledWith('batch-1'))
    await fireEvent.click(screen.getByRole('button', { name: 'assign subject' }))
    await waitFor(() => expect(api.captureBatchAssignSubject).toHaveBeenCalledWith({
      batchId: 'batch-1', expectedRevision: 1, subject: '化学',
    }))
  })

  it('makes custom subjects from settings available in the capture workspace', async () => {
    api.subjectPreferencesGet.mockResolvedValue(success({
      enabledSubjects: ['数学'], customSubjects: ['编程', '竞赛'], captureSoundEnabled: true,
    }))
    render(CaptureView)

    await waitFor(() => expect(screen.getByTestId('subjects')).toHaveTextContent('数学、编程、竞赛'))
  })

  it('continues a desktop batch import after one image is rejected', async () => {
    api.captureImportBytes
      .mockResolvedValueOnce(failure('图片损坏'))
      .mockResolvedValueOnce(success({ ...detail, items: [] }))
    render(CaptureView)

    await fireEvent.click(screen.getByRole('button', { name: 'open batch' }))
    await waitFor(() => expect(api.captureBatchDetail).toHaveBeenCalledWith('batch-1'))
    await fireEvent.click(screen.getByRole('button', { name: 'import files' }))

    await waitFor(() => expect(api.captureImportBytes).toHaveBeenCalledTimes(2))
    expect((api.captureImportBytes.mock.calls[0]![0] as { sourceName: string }).sourceName).toBe('bad.png')
    expect((api.captureImportBytes.mock.calls[1]![0] as { sourceName: string }).sourceName).toBe('good.png')
    expect((api.captureImportBytes.mock.calls[0]![0] as { sourceSequence: number }).sourceSequence).toBe(0)
    expect((api.captureImportBytes.mock.calls[1]![0] as { sourceSequence: number }).sourceSequence).toBe(1)
  })

  it('explains the 150-image batch limit instead of silently truncating', async () => {
    render(CaptureView)

    await fireEvent.click(screen.getByRole('button', { name: 'open batch' }))
    await waitFor(() => expect(api.captureBatchDetail).toHaveBeenCalledWith('batch-1'))
    await fireEvent.click(screen.getByRole('button', { name: 'import overflow' }))

    await waitFor(() => expect(api.captureImportBytes).toHaveBeenCalledTimes(150))
    expect(screen.getByTestId('error')).toHaveTextContent('本批最多保存 150 张')
  })

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
