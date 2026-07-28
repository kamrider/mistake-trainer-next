import { fireEvent, render, screen, waitFor } from '@testing-library/vue'
import { beforeEach, describe, expect, it, vi } from 'vitest'
import { syncControllerKey } from '../sync-controller'
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
  captureDraftUpdate: vi.fn(),
  captureBatchAssignSubject: vi.fn(),
  captureItemPreview: vi.fn(),
  captureCommitReady: vi.fn(),
  captureRecognitionStatus: vi.fn(),
  captureRecognitionLastOperation: vi.fn(),
  captureRecognitionApply: vi.fn(),
  captureRecognitionRevert: vi.fn(),
  subjectPreferencesGet: vi.fn(),
}))

vi.mock('@tauri-apps/api/core', () => ({ isTauri: () => true }))
vi.mock('@tauri-apps/api/event', () => ({ listen: vi.fn().mockResolvedValue(vi.fn()) }))
vi.mock('../../shared/api/bindings', () => ({ commands: api }))
vi.mock('../../modules/capture/components/CaptureWorkspace.vue', () => ({
  default: {
    props: [
      'batches', 'detail', 'busy', 'errorMessage', 'lanSession', 'subjectOptions',
      'recognitionJob', 'recognitionOperation', 'recognitionNotice',
      'previews',
    ],
    emits: [
      'openBatch', 'mobileCapture', 'mergeCard', 'deleteDraft', 'moveItem',
      'assignBatchSubject', 'updateDraft', 'importFiles', 'commitReady',
      'recognitionApply', 'recognitionRevert', 'preview',
    ],
    setup(_props: Record<string, unknown>, { emit }: { emit: (event: string, ...args: unknown[]) => void }) {
      const emitFiles = () => emit('importFiles', [
        { name: 'bad.png', arrayBuffer: async () => new Uint8Array([1]).buffer },
        { name: 'good.png', arrayBuffer: async () => new Uint8Array([2]).buffer },
      ])
      const emitOverflow = () => emit('importFiles', Array.from({ length: 151 }, (_, index) => ({
        name: `image-${index + 1}.png`,
        arrayBuffer: async () => new Uint8Array([index & 255]).buffer,
      })))
      const emitPreviewTwice = () => {
        emit('preview', 'item-1')
        emit('preview', 'item-1')
      }
      const emitPreviewSeries = () => {
        for (let index = 1; index <= 41; index += 1) emit('preview', `item-${index}`)
      }
      return { emitFiles, emitOverflow, emitPreviewTwice, emitPreviewSeries }
    },
    template: `
      <div>
        <button @click="$emit('openBatch', 'batch-1')">open batch</button>
        <button :disabled="busy" @click="$emit('commitReady')">commit ready</button>
        <button @click="emitFiles">import files</button>
        <button @click="emitOverflow">import overflow</button>
        <button :disabled="busy" @click="$emit('mobileCapture', '192.168.1.20')">scan</button>
        <button :disabled="busy" @click="$emit('mergeCard', ['item-1'], null, '数学')">merge card</button>
        <button :disabled="busy" @click="$emit('deleteDraft', 'draft-1')">delete card</button>
        <button :disabled="busy" @click="$emit('moveItem', { itemId: 'item-1', targetDraftId: 'draft-1', targetRole: 'answer', targetPosition: 0 })">change role</button>
        <button :disabled="busy" @click="$emit('assignBatchSubject', '化学')">assign subject</button>
        <button @click="$emit('updateDraft', { id: 'draft-1' }, '数学', ['标签'], '最新备注')">update draft</button>
        <button @click="emitPreviewTwice">preview twice</button>
        <button @click="emitPreviewSeries">preview 41</button>
        <button @click="$emit('preview', 'item-1')">preview first</button>
        <button @click="$emit('recognitionApply', ['suggestion-1'])">apply recognition</button>
        <button
          v-if="recognitionOperation"
          @click="$emit('recognitionRevert', recognitionOperation.operationId)"
        >undo recognition</button>
        <span data-testid="subjects">{{ subjectOptions?.join('、') }}</span>
        <span data-testid="error">{{ errorMessage }}</span>
        <span data-testid="session">{{ lanSession?.sessionId ?? 'none' }}</span>
        <span data-testid="recognition-notice">{{ recognitionNotice }}</span>
        <span data-testid="preview-cache">{{ Object.keys(previews ?? {}).length }}</span>
        <span data-testid="preview-first">{{ previews?.['item-1'] ? 'loaded' : 'evicted' }}</span>
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
const syncController = {
  run: vi.fn(),
  scheduleMutation: vi.fn(),
  dispose: vi.fn(),
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
  api.captureDraftUpdate.mockResolvedValue(success(detail))
  api.captureBatchAssignSubject.mockResolvedValue(success({ ...detail, batch: { ...batch, subject: '化学', revision: 2 } }))
  api.captureItemPreview.mockImplementation((_batchId: string, itemId: string) =>
    Promise.resolve(success({ itemId, dataUrl: `data:image/png;base64,${itemId}` })))
  api.captureCommitReady.mockResolvedValue(success({
    committedProblemIds: ['problem-1'],
    committedCount: 1,
    remainingDraftCount: 0,
  }))
  api.captureRecognitionStatus.mockResolvedValue(success(null))
  api.captureRecognitionLastOperation.mockResolvedValue(success(null))
  api.subjectPreferencesGet.mockResolvedValue(success({
    enabledSubjects: ['数学', '化学'], customSubjects: [], captureSoundEnabled: true,
  }))
})

it('coalesces duplicate thumbnail requests while one preview is loading', async () => {
  let resolvePreview!: (value: unknown) => void
  api.captureItemPreview.mockReturnValue(new Promise(resolve => {
    resolvePreview = resolve
  }))
  render(CaptureView, {
    global: { provide: { [syncControllerKey as symbol]: syncController } },
  })
  await fireEvent.click(screen.getByRole('button', { name: 'open batch' }))
  await waitFor(() => expect(api.captureBatchDetail).toHaveBeenCalledWith('batch-1'))

  await fireEvent.click(screen.getByRole('button', { name: 'preview twice' }))

  expect(api.captureItemPreview).toHaveBeenCalledOnce()
  expect(api.captureItemPreview).toHaveBeenCalledWith('batch-1', 'item-1')
  resolvePreview(success({ itemId: 'item-1', dataUrl: 'data:image/png;base64,test' }))
  await waitFor(() => expect(screen.getByTestId('preview-cache')).toHaveTextContent('1'))
  await fireEvent.click(screen.getByRole('button', { name: 'preview twice' }))
  expect(api.captureItemPreview).toHaveBeenCalledOnce()
})

it('keeps only 40 previews and reloads an item after LRU eviction', async () => {
  render(CaptureView, {
    global: { provide: { [syncControllerKey as symbol]: syncController } },
  })
  await fireEvent.click(screen.getByRole('button', { name: 'open batch' }))
  await waitFor(() => expect(api.captureBatchDetail).toHaveBeenCalledWith('batch-1'))

  await fireEvent.click(screen.getByRole('button', { name: 'preview 41' }))

  await waitFor(() => expect(screen.getByTestId('preview-cache')).toHaveTextContent('40'))
  expect(screen.getByTestId('preview-first')).toHaveTextContent('evicted')
  expect(api.captureItemPreview).toHaveBeenCalledTimes(41)

  await fireEvent.click(screen.getByRole('button', { name: 'preview first' }))

  await waitFor(() => expect(screen.getByTestId('preview-first')).toHaveTextContent('loaded'))
  expect(screen.getByTestId('preview-cache')).toHaveTextContent('40')
  expect(api.captureItemPreview).toHaveBeenCalledTimes(42)
})

it('schedules cloud sync only after formal problems are committed', async () => {
  render(CaptureView, {
    global: { provide: { [syncControllerKey as symbol]: syncController } },
  })
  await fireEvent.click(screen.getByRole('button', { name: 'open batch' }))
  await waitFor(() => expect(api.captureBatchDetail).toHaveBeenCalledWith('batch-1'))
  await fireEvent.click(screen.getByRole('button', { name: 'commit ready' }))

  await waitFor(() => expect(syncController.scheduleMutation).toHaveBeenCalledOnce())
  expect(api.captureCommitReady).toHaveBeenCalledWith('batch-1', 1)
})

it('does not schedule sync when the batch contains no ready problem', async () => {
  api.captureCommitReady.mockResolvedValue(success({
    committedProblemIds: [],
    committedCount: 0,
    remainingDraftCount: 1,
  }))
  render(CaptureView, {
    global: { provide: { [syncControllerKey as symbol]: syncController } },
  })
  await fireEvent.click(screen.getByRole('button', { name: 'open batch' }))
  await waitFor(() => expect(api.captureBatchDetail).toHaveBeenCalledWith('batch-1'))
  await fireEvent.click(screen.getByRole('button', { name: 'commit ready' }))

  await waitFor(() => expect(api.captureCommitReady).toHaveBeenCalledOnce())
  expect(syncController.scheduleMutation).not.toHaveBeenCalled()
})

it('applies reviewed recognition atomically and exposes persistent safe undo', async () => {
  const organizingDetail = {
    ...detail,
    batch: { ...batch, state: 'organizing', revision: 2 },
    unassignedItemIds: ['item-1'],
  }
  const appliedDetail = {
    ...organizingDetail,
    batch: { ...organizingDetail.batch, revision: 3 },
    items: [{ id: 'derived-question' }, { id: 'derived-answer' }],
    drafts: [{ id: 'draft-smart' }],
    unassignedItemIds: [],
  }
  const revertedDetail = {
    ...organizingDetail,
    batch: { ...organizingDetail.batch, revision: 4 },
  }
  api.captureBatchDetail.mockResolvedValue(success(organizingDetail))
  api.captureRecognitionStatus.mockResolvedValue(success({
    id: 'job-1',
    batchId: 'batch-1',
    state: 'review',
    totalItems: 1,
    processedItems: 1,
    suggestions: [{
      id: 'suggestion-1',
      itemId: 'item-1',
      regions: [],
      confidenceBasisPoints: 9200,
      reviewBand: 'high',
      state: 'accepted',
      reasonCodes: [],
    }],
    createdAtUtcMs: 1,
    updatedAtUtcMs: 2,
  }))
  api.captureRecognitionApply.mockResolvedValue({
    status: 'ok',
    data: success({
      operationId: 'operation-1',
      appliedSuggestionCount: 1,
      createdDraftCount: 0,
      createdItemCount: 2,
      unmatchedAnswerCount: 0,
      staleSuggestionCount: 0,
      detail: appliedDetail,
    }),
  })
  api.captureRecognitionRevert.mockResolvedValue({
    status: 'ok',
    data: success({
      operationId: 'operation-1',
      revertedItemCount: 2,
      detail: revertedDetail,
    }),
  })
  render(CaptureView)

  await fireEvent.click(screen.getByRole('button', { name: 'open batch' }))
  await waitFor(() => expect(api.captureRecognitionStatus).toHaveBeenCalledWith('batch-1'))
  await fireEvent.click(screen.getByRole('button', { name: 'apply recognition' }))

  await waitFor(() => expect(api.captureRecognitionApply).toHaveBeenCalledWith({
    batchId: 'batch-1',
    jobId: 'job-1',
    expectedRevision: 2,
    acceptedSuggestionIds: ['suggestion-1'],
  }))
  expect(screen.getByTestId('recognition-notice')).toHaveTextContent('已切分 2 张题答图片，已放入素材牌库')

  await fireEvent.click(screen.getByRole('button', { name: 'undo recognition' }))
  await waitFor(() => expect(api.captureRecognitionRevert).toHaveBeenCalledWith({
    batchId: 'batch-1',
    operationId: 'operation-1',
    expectedRevision: 3,
  }))
  expect(screen.getByTestId('recognition-notice')).toHaveTextContent('已撤销智能整理')
})

describe('CaptureView one-time Windows LAN permission', () => {
  it('queues the latest draft text while another draft save is still running', async () => {
    let resolveFirst: (value: unknown) => void = () => undefined
    api.captureDraftUpdate
      .mockImplementationOnce(() => new Promise(resolve => { resolveFirst = resolve }))
      .mockResolvedValueOnce(success(detail))
    render(CaptureView)

    await fireEvent.click(screen.getByRole('button', { name: 'open batch' }))
    await waitFor(() => expect(api.captureBatchDetail).toHaveBeenCalledWith('batch-1'))
    await fireEvent.click(screen.getByRole('button', { name: 'update draft' }))
    await fireEvent.click(screen.getByRole('button', { name: 'update draft' }))

    expect(api.captureDraftUpdate).toHaveBeenCalledTimes(1)
    resolveFirst(success(detail))
    await waitFor(() => expect(api.captureDraftUpdate).toHaveBeenCalledTimes(2))
    expect(api.captureDraftUpdate.mock.calls[1]![0]).toMatchObject({ note: '最新备注' })
  })

  it('reloads the batch and retries a draft update after a revision conflict', async () => {
    const refreshedDetail = { ...detail, batch: { ...batch, revision: 2 } }
    api.captureBatchDetail
      .mockResolvedValueOnce(success(detail))
      .mockResolvedValueOnce(success(refreshedDetail))
    api.captureDraftUpdate
      .mockResolvedValueOnce({
        ok: false,
        error: {
          code: 'capture_revision_conflict',
          userMessage: '批次已更新',
          retryable: true,
          diagnosticId: 'diag-revision',
        },
      })
      .mockResolvedValueOnce(success({ ...refreshedDetail, batch: { ...refreshedDetail.batch, revision: 3 } }))
    render(CaptureView)

    await fireEvent.click(screen.getByRole('button', { name: 'open batch' }))
    await waitFor(() => expect(api.captureBatchDetail).toHaveBeenCalledWith('batch-1'))
    await fireEvent.click(screen.getByRole('button', { name: 'update draft' }))
    await waitFor(() => expect(api.captureDraftUpdate).toHaveBeenCalledTimes(2))

    expect(api.captureDraftUpdate.mock.calls[0]![0]).toMatchObject({ expectedRevision: 1 })
    expect(api.captureDraftUpdate.mock.calls[1]![0]).toMatchObject({ expectedRevision: 2 })
  })

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
