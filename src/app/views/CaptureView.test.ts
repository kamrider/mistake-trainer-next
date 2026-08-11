import { fireEvent, render, screen, waitFor } from '@testing-library/vue'
import { nextTick } from 'vue'
import { createMemoryHistory, RouterView } from 'vue-router'
import { beforeEach, describe, expect, it, vi } from 'vitest'
import { createAppRouter } from '../router'
import { syncControllerKey } from '../sync-controller'
import {
  createWorkspaceTransitionGuard,
  workspaceTransitionGuardKey,
} from '../workspace-transition-guard'
import CaptureView from './CaptureView.vue'

const api = vi.hoisted(() => ({
  captureBatchList: vi.fn(),
  captureBatchDetail: vi.fn(),
  captureImportSelect: vi.fn(),
  captureImportBytes: vi.fn(),
  captureLanAddresses: vi.fn(),
  captureLanPreflight: vi.fn(),
  captureLanFirewallRepair: vi.fn(),
  captureLanStart: vi.fn(),
  captureLanStatus: vi.fn(),
  captureLanStop: vi.fn(),
  captureCardMerge: vi.fn(),
  capturePairSuggestionsApply: vi.fn(),
  captureDraftDelete: vi.fn(),
  captureItemMove: vi.fn(),
  captureDraftUpdate: vi.fn(),
  captureBatchAssignSubject: vi.fn(),
  captureItemRemove: vi.fn(),
  captureCropSourcePreview: vi.fn(),
  captureCropApply: vi.fn(),
  captureCropRevert: vi.fn(),
  captureItemPreview: vi.fn(),
  captureQualityCheck: vi.fn(),
  captureCommitReady: vi.fn(),
  captureRecognitionStatus: vi.fn(),
  captureRecognitionLastOperation: vi.fn(),
  captureRecognitionReview: vi.fn(),
  captureRecognitionApply: vi.fn(),
  captureRecognitionRevert: vi.fn(),
  ocrCapabilityStatus: vi.fn(),
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
      'recognitionOperationBusy',
      'previews', 'importProgress', 'commitMessage', 'saveState', 'draftSaveRetryAvailable',
      'qualityReports', 'qualityCheckingItemId', 'qualityErrors', 'qualityDismissedItemIds',
    ],
    emits: [
      'openBatch', 'back', 'mobileCapture', 'stopMobileCapture', 'mergeCard', 'deleteDraft', 'moveItem',
      'assignBatchSubject', 'updateDraft', 'importSelect', 'importFiles', 'removeItem', 'crop', 'revertCrop', 'commitReady',
      'recognitionEdit', 'recognitionApply', 'recognitionRevert', 'applyPairSuggestions', 'preview', 'retryDraftSave',
      'qualityCheck', 'qualityDismiss',
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
        <button @click="$emit('openBatch', 'batch-2')">open batch two</button>
        <button @click="$emit('back')">close batch</button>
        <button :disabled="busy" @click="$emit('commitReady')">commit ready</button>
        <button @click="$emit('importSelect')">import select</button>
        <button @click="emitFiles">import files</button>
        <button @click="emitOverflow">import overflow</button>
        <button :disabled="busy" @click="$emit('mobileCapture', '192.168.1.20')">scan</button>
        <button @click="$emit('stopMobileCapture')">stop mobile</button>
        <button :disabled="busy" @click="$emit('mergeCard', ['item-1'], null, '数学')">merge card</button>
        <button :disabled="busy" @click="$emit('applyPairSuggestions', ['pair-1'])">apply pair</button>
        <button :disabled="busy" @click="$emit('deleteDraft', 'draft-1')">delete card</button>
        <button :disabled="busy" @click="$emit('removeItem', 'item-1')">remove item</button>
        <button
          v-if="detail?.items.some(item => item.id === 'item-1' && !item.cropDerivationId)"
          :key="[detail?.batch.revision, errorMessage ? 'error' : 'ready'].join('-')"
          data-crop-item-id="item-1"
          :disabled="busy"
          @click="$emit('crop', 'item-1')"
        >crop item</button>
        <button
          v-if="detail?.items.some(item => item.id === 'derived-1' && item.cropDerivationId)"
          data-crop-result-item-id="derived-1"
        >revert cropped item</button>
        <button
          v-if="!detail"
          data-crop-item-id="item-1"
        >unrelated inbox action</button>
        <button :disabled="busy" @click="$emit('revertCrop', 'crop-1')">revert crop</button>
        <button :disabled="busy" @click="$emit('moveItem', { itemId: 'item-1', targetDraftId: 'draft-1', targetRole: 'answer', targetPosition: 0 })">change role</button>
        <button :disabled="busy" @click="$emit('assignBatchSubject', '化学')">assign subject</button>
        <button @click="$emit('updateDraft', { id: 'draft-1' }, '数学', ['标签'], '最新备注')">update draft</button>
        <button @click="$emit('updateDraft', { id: 'draft-1' }, '数学', ['标签'], '冲突后的更新备注')">update draft newest</button>
        <button @click="$emit('updateDraft', { id: 'draft-2' }, '物理', [], '另一张草稿')">update second draft</button>
        <button
          v-if="draftSaveRetryAvailable"
          :disabled="busy"
          @click="$emit('retryDraftSave')"
        >retry draft save</button>
        <button @click="emitPreviewTwice">preview twice</button>
        <button @click="emitPreviewSeries">preview 41</button>
        <button @click="$emit('preview', 'item-1')">preview first</button>
        <button @click="$emit('qualityCheck', 'item-1')">check quality</button>
        <span data-testid="quality-count">{{ Object.keys(qualityReports ?? {}).length }}</span>
        <button
          v-if="recognitionJob?.suggestions.some(item => item.id === 'suggestion-1')"
          :key="[recognitionJob.suggestions.find(item => item.id === 'suggestion-1')?.state, errorMessage ? 'error' : 'ready'].join('-')"
          data-recognition-edit-suggestion-id="suggestion-1"
          :disabled="recognitionOperationBusy"
          @click="$emit('recognitionEdit', 'suggestion-1')"
        >edit recognition</button>
        <button
          v-if="!detail"
          data-recognition-edit-suggestion-id="suggestion-1"
        >unrelated recognition action</button>
        <button @click="$emit('recognitionApply', ['suggestion-1'])">apply recognition</button>
        <button
          v-if="recognitionOperation"
          @click="$emit('recognitionRevert', recognitionOperation.operationId)"
        >undo recognition</button>
        <span data-testid="subjects">{{ subjectOptions?.join('、') }}</span>
        <span data-testid="active-batch">{{ detail?.batch.id ?? 'none' }}</span>
        <span data-testid="busy-state">{{ busy ? 'busy' : 'idle' }}</span>
        <span data-testid="save-state">{{ saveState }}</span>
        <span data-testid="error">{{ errorMessage }}</span>
        <span data-testid="commit-message">{{ commitMessage }}</span>
        <span data-testid="session">{{ lanSession?.sessionId ?? 'none' }}</span>
        <span data-testid="recognition-notice">{{ recognitionNotice }}</span>
        <span data-testid="preview-cache">{{ Object.keys(previews ?? {}).length }}</span>
        <span data-testid="preview-first">{{ previews?.['item-1'] ? 'loaded' : 'evicted' }}</span>
        <span data-testid="import-progress">{{ importProgress ? importProgress.completed + '/' + importProgress.total : 'none' }}</span>
      </div>
    `,
  },
}))

const batch = {
  id: 'batch-1', subject: '数学', state: 'collecting', itemCount: 0,
  draftCount: 0, readyCount: 0, updatedAtUtcMs: 1, revision: 1,
}
const detail = { batch, items: [], drafts: [], unassignedItemIds: [], pairSuggestions: [] }
const editableSourceItem = {
  id: 'item-1', sourceName: '题目.png', sourceSequence: 0, mediaType: 'image/png',
  byteLength: 100, width: 1200, height: 900, stagedRole: 'question', draftId: null,
  role: null, position: null, cropDerivationId: null, cropSourceItemId: null,
}
const editableDerivedItem = {
  ...editableSourceItem, id: 'derived-1', sourceSequence: 1,
  cropDerivationId: 'crop-1', cropSourceItemId: 'item-1',
}
const editableDetail = { ...detail, items: [editableSourceItem, editableDerivedItem] }
const organizingDetail = {
  ...detail,
  batch: { ...detail.batch, state: 'organizing' as const },
  items: [editableSourceItem],
}
const croppedDetail = {
  ...organizingDetail,
  batch: { ...organizingDetail.batch, revision: 2 },
  items: [editableDerivedItem],
}
function recognitionReviewJob(state: 'proposed' | 'accepted' = 'proposed') {
  return {
    id: 'job-1',
    batchId: 'batch-1',
    state: 'review' as const,
    totalItems: 1,
    processedItems: 1,
    suggestions: [{
      id: 'suggestion-1',
      itemId: 'item-1',
      regions: [{
        rect: { x: 0.1, y: 0.1, width: 0.8, height: 0.35 },
        role: 'question' as const,
        groupSlot: 0,
        confidenceBasisPoints: 7600,
      }],
      confidenceBasisPoints: 7600,
      reviewBand: 'review' as const,
      state,
      reasonCodes: ['weak_anchor' as const],
    }],
    createdAtUtcMs: 1,
    updatedAtUtcMs: state === 'accepted' ? 3 : 2,
  }
}
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

async function renderRoutedCapture() {
  const router = createAppRouter(createMemoryHistory())
  await router.push('/inbox')
  await router.isReady()
  render(RouterView, {
    global: {
      plugins: [router],
      provide: { [syncControllerKey as symbol]: syncController },
    },
  })
  await screen.findByRole('button', { name: 'open batch' })
  return router
}

async function renderGuardedRoutedCapture() {
  const router = createAppRouter(createMemoryHistory())
  const workspaceTransitionGuard = createWorkspaceTransitionGuard()
  await router.push('/inbox')
  await router.isReady()
  const view = render(RouterView, {
    global: {
      plugins: [router],
      provide: {
        [syncControllerKey as symbol]: syncController,
        [workspaceTransitionGuardKey as symbol]: workspaceTransitionGuard,
      },
    },
  })
  await screen.findByRole('button', { name: 'open batch' })
  return { router, view, workspaceTransitionGuard }
}

beforeEach(() => {
  vi.clearAllMocks()
  api.captureBatchList.mockResolvedValue(success([batch]))
  api.captureBatchDetail.mockResolvedValue(success(detail))
  api.captureImportSelect.mockResolvedValue(success({ importedItems: [], importedCount: 0 }))
  api.captureImportBytes.mockResolvedValue(success({ ...detail, items: [] }))
  api.captureLanAddresses.mockResolvedValue(success([{ label: 'Wi-Fi', address: '192.168.1.20' }]))
  api.captureLanPreflight.mockResolvedValue(success(readyRule))
  api.captureLanStatus.mockResolvedValue(success(null))
  api.captureLanStart.mockResolvedValue(success(session))
  api.captureLanStop.mockResolvedValue(success(true))
  api.captureCardMerge.mockResolvedValue(success(detail))
  api.capturePairSuggestionsApply.mockResolvedValue(success(detail))
  api.captureDraftDelete.mockResolvedValue(success(detail))
  api.captureItemMove.mockResolvedValue(success(detail))
  api.captureDraftUpdate.mockResolvedValue(success(detail))
  api.captureBatchAssignSubject.mockResolvedValue(success({ ...detail, batch: { ...batch, subject: '化学', revision: 2 } }))
  api.captureItemRemove.mockResolvedValue(success(detail))
  api.captureCropSourcePreview.mockResolvedValue(success({
    itemId: 'item-1', mediaType: 'image/png', dataUrl: 'data:image/png;base64,item-1',
  }))
  api.captureCropApply.mockResolvedValue(success({
    detail: croppedDetail,
    operationId: 'crop-operation-1',
    sourceItemId: 'item-1',
    derivedItemIds: ['derived-1'],
  }))
  api.captureCropRevert.mockResolvedValue(success(detail))
  api.captureItemPreview.mockImplementation((_batchId: string, itemId: string) =>
    Promise.resolve(success({ itemId, dataUrl: `data:image/png;base64,${itemId}` })))
  api.captureQualityCheck.mockResolvedValue(success({
    itemId: 'item-1', issues: ['skewed'], sharpnessScore: 0.4,
    darkFraction: 0.01, brightFraction: 0.5, contrastScore: 0.7,
    suggestedRotationDegrees: -2.1,
    suggestedCrop: { x: 0.12, y: 0.08, width: 0.76, height: 0.8 },
  }))
  api.captureCommitReady.mockResolvedValue(success({
    committedProblemIds: ['problem-1'],
    committedCount: 1,
    remainingDraftCount: 0,
  }))
  api.captureRecognitionStatus.mockResolvedValue(success(null))
  api.captureRecognitionLastOperation.mockResolvedValue(success(null))
  api.captureRecognitionReview.mockResolvedValue(success(recognitionReviewJob('accepted')))
  api.ocrCapabilityStatus.mockResolvedValue({
    status: 'ok',
    data: success({
      supported: true,
      detail: 'ready',
      components: [],
      recognitionFeature: {
        state: 'ready',
        requiredComponentId: 'opencv_preprocess',
        detail: 'ready',
      },
      automaticRecognitionEnabled: false,
    }),
  })
  api.subjectPreferencesGet.mockResolvedValue(success({
    enabledSubjects: ['数学', '化学'], customSubjects: [], captureSoundEnabled: true,
  }))
})

it('opens the batch without waiting for optional recognition capability status', async () => {
  let resolveCapability: (value: unknown) => void = () => undefined
  api.ocrCapabilityStatus.mockReturnValueOnce(new Promise(resolve => {
    resolveCapability = resolve
  }))
  render(CaptureView)

  await fireEvent.click(screen.getByRole('button', { name: 'open batch' }))

  await waitFor(() => expect(screen.getByTestId('active-batch')).toHaveTextContent('batch-1'))
  expect(api.ocrCapabilityStatus).toHaveBeenCalledOnce()
  resolveCapability({
    status: 'ok',
    data: success({
      supported: true,
      detail: 'ready',
      components: [],
      recognitionFeature: {
        state: 'ready',
        requiredComponentId: 'opencv_preprocess',
        detail: 'ready',
      },
      automaticRecognitionEnabled: false,
    }),
  })
  await nextTick()
})

it('requires explicit in-app confirmation for image deletion and crop reversion', async () => {
  api.captureBatchDetail.mockResolvedValue(success(editableDetail))
  api.captureItemRemove.mockResolvedValue(success(editableDetail))
  api.captureCropRevert.mockResolvedValue(success(editableDetail))
  render(CaptureView)
  await fireEvent.click(screen.getByRole('button', { name: 'open batch' }))
  await waitFor(() => expect(screen.getByTestId('active-batch')).toHaveTextContent('batch-1'))

  await fireEvent.click(screen.getByRole('button', { name: 'remove item' }))
  expect(screen.getByRole('alertdialog', { name: '删除这张采集图片？' })).toBeVisible()
  expect(api.captureItemRemove).not.toHaveBeenCalled()
  await fireEvent.click(screen.getByRole('button', { name: '保留图片' }))
  expect(api.captureItemRemove).not.toHaveBeenCalled()

  await fireEvent.click(screen.getByRole('button', { name: 'remove item' }))
  await fireEvent.click(screen.getByRole('button', { name: '删除图片' }))
  await waitFor(() => expect(api.captureItemRemove).toHaveBeenCalledWith('batch-1', 1, 'item-1'))

  await fireEvent.click(screen.getByRole('button', { name: 'revert crop' }))
  expect(screen.getByRole('alertdialog', { name: '恢复裁剪前的原图？' })).toBeVisible()
  expect(api.captureCropRevert).not.toHaveBeenCalled()
  await fireEvent.click(screen.getByRole('button', { name: '恢复原图' }))
  await waitFor(() => expect(api.captureCropRevert).toHaveBeenCalledWith({
    batchId: 'batch-1',
    expectedRevision: 1,
    derivationId: 'crop-1',
  }))
})

it('returns focus to the crop launcher after cancel and a successful save refresh', async () => {
  api.captureBatchDetail.mockResolvedValue(success(organizingDetail))
  render(CaptureView)
  await fireEvent.click(screen.getByRole('button', { name: 'open batch' }))
  await waitFor(() => expect(screen.getByTestId('active-batch')).toHaveTextContent('batch-1'))

  const firstLauncher = screen.getByRole('button', { name: 'crop item' })
  firstLauncher.focus()
  await fireEvent.click(firstLauncher)
  expect(await screen.findByRole('dialog', { name: '裁出真正需要的题目范围' })).toBeVisible()
  await fireEvent.click(screen.getByRole('button', { name: '取消' }))
  await waitFor(() => expect(firstLauncher).toHaveFocus())

  await fireEvent.click(firstLauncher)
  expect(await screen.findByRole('dialog', { name: '裁出真正需要的题目范围' })).toBeVisible()
  await fireEvent.click(screen.getByRole('button', { name: '生成 1 张裁剪图' }))

  await waitFor(() => expect(api.captureCropApply).toHaveBeenCalledOnce())
  await waitFor(() => expect(screen.queryByRole('dialog', { name: '裁出真正需要的题目范围' })).not.toBeInTheDocument())
  expect(screen.queryByRole('button', { name: 'crop item' })).not.toBeInTheDocument()
  const resultControl = screen.getByRole('button', { name: 'revert cropped item' })
  await waitFor(() => expect(resultControl).toHaveFocus())
})

it('restores the crop launcher when preview loading fails before the editor opens', async () => {
  api.captureBatchDetail.mockResolvedValue(success(organizingDetail))
  api.captureCropSourcePreview.mockResolvedValueOnce(failure('裁剪预览暂时不可用'))
  render(CaptureView)
  await fireEvent.click(screen.getByRole('button', { name: 'open batch' }))
  await waitFor(() => expect(screen.getByTestId('active-batch')).toHaveTextContent('batch-1'))

  const launcher = screen.getByRole('button', { name: 'crop item' })
  launcher.focus()
  await fireEvent.click(launcher)

  await waitFor(() => expect(screen.getByTestId('error')).toHaveTextContent('裁剪预览暂时不可用'))
  expect(screen.queryByRole('dialog', { name: '裁出真正需要的题目范围' })).not.toBeInTheDocument()
  const restoredLauncher = screen.getByRole('button', { name: 'crop item' })
  expect(restoredLauncher).not.toBe(launcher)
  await waitFor(() => expect(restoredLauncher).toHaveFocus())
})

it('keeps focus inside the crop editor when saving fails', async () => {
  api.captureBatchDetail.mockResolvedValue(success(organizingDetail))
  api.captureCropApply.mockResolvedValueOnce(failure('裁剪保存失败，请重试'))
  render(CaptureView)
  await fireEvent.click(screen.getByRole('button', { name: 'open batch' }))
  await waitFor(() => expect(screen.getByTestId('active-batch')).toHaveTextContent('batch-1'))

  const launcher = screen.getByRole('button', { name: 'crop item' })
  launcher.focus()
  await fireEvent.click(launcher)
  const dialog = await screen.findByRole('dialog', { name: '裁出真正需要的题目范围' })
  const applyButton = screen.getByRole('button', { name: '生成 1 张裁剪图' })
  applyButton.focus()
  await fireEvent.click(applyButton)

  await waitFor(() => expect(screen.getByTestId('error')).toHaveTextContent('裁剪保存失败，请重试'))
  expect(dialog).toBeVisible()
  expect(dialog).toContainElement(document.activeElement as HTMLElement)
  expect(screen.getByRole('button', { name: 'crop item' })).not.toHaveFocus()
})

it('cancels a pending crop focus restoration when leaving the detail', async () => {
  api.captureBatchDetail.mockResolvedValue(success(organizingDetail))
  render(CaptureView)
  await fireEvent.click(screen.getByRole('button', { name: 'open batch' }))
  await waitFor(() => expect(screen.getByTestId('active-batch')).toHaveTextContent('batch-1'))

  const launcher = screen.getByRole('button', { name: 'crop item' })
  launcher.focus()
  await fireEvent.click(launcher)
  expect(await screen.findByRole('dialog', { name: '裁出真正需要的题目范围' })).toBeVisible()

  const closing = fireEvent.click(screen.getByRole('button', { name: '取消' }))
  await fireEvent.click(screen.getByRole('button', { name: 'close batch' }))
  await closing

  await waitFor(() => expect(screen.getByTestId('active-batch')).toHaveTextContent('none'))
  const unrelatedAction = screen.getByRole('button', { name: 'unrelated inbox action' })
  expect(unrelatedAction).not.toHaveFocus()
})

it('returns focus to the recognition edit control after cancel and successful proposal save', async () => {
  api.captureBatchDetail.mockResolvedValue(success(organizingDetail))
  api.captureRecognitionStatus.mockResolvedValue(success(recognitionReviewJob()))
  render(CaptureView)
  await fireEvent.click(screen.getByRole('button', { name: 'open batch' }))
  const firstLauncher = await screen.findByRole('button', { name: 'edit recognition' })

  firstLauncher.focus()
  await fireEvent.click(firstLauncher)
  expect(await screen.findByRole('dialog', { name: '裁出真正需要的题目范围' })).toBeVisible()
  await fireEvent.click(screen.getByRole('button', { name: '取消' }))
  await waitFor(() => expect(firstLauncher).toHaveFocus())

  await fireEvent.click(firstLauncher)
  expect(await screen.findByRole('dialog', { name: '裁出真正需要的题目范围' })).toBeVisible()
  await fireEvent.click(screen.getByRole('button', { name: '保存 1 个建议区域' }))

  await waitFor(() => expect(api.captureRecognitionReview).toHaveBeenCalledOnce())
  await waitFor(() => expect(screen.queryByRole('dialog', { name: '裁出真正需要的题目范围' })).not.toBeInTheDocument())
  const refreshedLauncher = screen.getByRole('button', { name: 'edit recognition' })
  expect(refreshedLauncher).not.toBe(firstLauncher)
  await waitFor(() => expect(refreshedLauncher).toHaveFocus())
})

it('restores a replaced recognition edit control when proposal preview fails', async () => {
  api.captureBatchDetail.mockResolvedValue(success(organizingDetail))
  api.captureRecognitionStatus.mockResolvedValue(success(recognitionReviewJob()))
  api.captureCropSourcePreview.mockResolvedValueOnce(failure('建议来源图暂时不可用'))
  render(CaptureView)
  await fireEvent.click(screen.getByRole('button', { name: 'open batch' }))
  const launcher = await screen.findByRole('button', { name: 'edit recognition' })

  launcher.focus()
  await fireEvent.click(launcher)

  await waitFor(() => expect(screen.getByTestId('error')).toHaveTextContent('建议来源图暂时不可用'))
  expect(screen.queryByRole('dialog', { name: '裁出真正需要的题目范围' })).not.toBeInTheDocument()
  const restoredLauncher = screen.getByRole('button', { name: 'edit recognition' })
  expect(restoredLauncher).not.toBe(launcher)
  await waitFor(() => expect(restoredLauncher).toHaveFocus())
})

it('keeps focus inside the recognition proposal editor when saving fails', async () => {
  api.captureBatchDetail.mockResolvedValue(success(organizingDetail))
  api.captureRecognitionStatus.mockResolvedValue(success(recognitionReviewJob()))
  api.captureRecognitionReview.mockResolvedValueOnce(failure('建议边界没有保存，请重试'))
  render(CaptureView)
  await fireEvent.click(screen.getByRole('button', { name: 'open batch' }))
  const launcher = await screen.findByRole('button', { name: 'edit recognition' })

  launcher.focus()
  await fireEvent.click(launcher)
  const dialog = await screen.findByRole('dialog', { name: '裁出真正需要的题目范围' })
  const saveButton = screen.getByRole('button', { name: '保存 1 个建议区域' })
  saveButton.focus()
  await fireEvent.click(saveButton)

  await waitFor(() => expect(screen.getByTestId('error')).toHaveTextContent('建议边界没有保存，请重试'))
  expect(dialog).toBeVisible()
  expect(dialog).toContainElement(document.activeElement as HTMLElement)
  expect(screen.getByRole('button', { name: 'edit recognition' })).not.toHaveFocus()
})

it('cancels pending recognition focus restoration when leaving the detail', async () => {
  api.captureBatchDetail.mockResolvedValue(success(organizingDetail))
  api.captureRecognitionStatus.mockResolvedValue(success(recognitionReviewJob()))
  render(CaptureView)
  await fireEvent.click(screen.getByRole('button', { name: 'open batch' }))
  const launcher = await screen.findByRole('button', { name: 'edit recognition' })

  launcher.focus()
  await fireEvent.click(launcher)
  expect(await screen.findByRole('dialog', { name: '裁出真正需要的题目范围' })).toBeVisible()

  const closing = fireEvent.click(screen.getByRole('button', { name: '取消' }))
  await fireEvent.click(screen.getByRole('button', { name: 'close batch' }))
  await closing

  await waitFor(() => expect(screen.getByTestId('active-batch')).toHaveTextContent('none'))
  expect(screen.getByRole('button', { name: 'unrelated recognition action' })).not.toHaveFocus()
})

it('does not delete from a batch left while confirmation is open', async () => {
  api.captureBatchDetail.mockResolvedValue(success(editableDetail))
  render(CaptureView)
  await fireEvent.click(screen.getByRole('button', { name: 'open batch' }))
  await waitFor(() => expect(screen.getByTestId('active-batch')).toHaveTextContent('batch-1'))

  await fireEvent.click(screen.getByRole('button', { name: 'remove item' }))
  expect(screen.getByRole('alertdialog', { name: '删除这张采集图片？' })).toBeVisible()
  await fireEvent.click(screen.getByRole('button', { name: 'close batch' }))
  expect(screen.getByTestId('active-batch')).toHaveTextContent('none')
  await fireEvent.click(screen.getByRole('button', { name: '删除图片' }))

  expect(api.captureItemRemove).not.toHaveBeenCalled()
  expect(screen.getByTestId('active-batch')).toHaveTextContent('none')
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

it('checks quality only on request and caches the local report for the active batch', async () => {
  api.captureBatchDetail.mockResolvedValue(success(organizingDetail))
  render(CaptureView)
  expect(api.captureQualityCheck).not.toHaveBeenCalled()
  await fireEvent.click(screen.getByRole('button', { name: 'open batch' }))
  await waitFor(() => expect(screen.getByTestId('active-batch')).toHaveTextContent('batch-1'))
  expect(api.captureQualityCheck).not.toHaveBeenCalled()

  await fireEvent.click(screen.getByRole('button', { name: 'check quality' }))
  await waitFor(() => expect(api.captureQualityCheck).toHaveBeenCalledWith('batch-1', 'item-1'))
  await waitFor(() => expect(screen.getByTestId('quality-count')).toHaveTextContent('1'))
  await fireEvent.click(screen.getByRole('button', { name: 'check quality' }))
  expect(api.captureQualityCheck).toHaveBeenCalledOnce()

  await fireEvent.click(screen.getByRole('button', { name: 'crop item' }))
  const region = await screen.findByRole('group', { name: '裁剪区域 1' })
  expect(region).toHaveStyle({ left: '12%', top: '8%', width: '76%', height: '80%' })
  expect(screen.getByText(/质量检测建议约旋转 -2.1°/)).toBeVisible()
  await fireEvent.click(screen.getByRole('button', { name: '取消' }))

  await fireEvent.click(screen.getByRole('button', { name: 'close batch' }))
  expect(screen.getByTestId('quality-count')).toHaveTextContent('0')
})

it('does not cache a preview that returns after leaving the batch', async () => {
  let resolvePreview: (value: unknown) => void = () => undefined
  let previewReturned = false
  api.captureItemPreview.mockImplementationOnce(async () => {
    const result = await new Promise(resolve => { resolvePreview = resolve })
    previewReturned = true
    return result
  })
  render(CaptureView)
  await fireEvent.click(screen.getByRole('button', { name: 'open batch' }))
  await waitFor(() => expect(api.captureBatchDetail).toHaveBeenCalledWith('batch-1'))
  await fireEvent.click(screen.getByRole('button', { name: 'preview first' }))
  await waitFor(() => expect(api.captureItemPreview).toHaveBeenCalledOnce())

  await fireEvent.click(screen.getByRole('button', { name: 'close batch' }))
  resolvePreview(success({
    itemId: 'item-1',
    mediaType: 'image/png',
    dataUrl: 'data:image/png;base64,stale',
  }))
  await waitFor(() => expect(previewReturned).toBe(true))
  await nextTick()

  expect(screen.getByTestId('preview-cache')).toHaveTextContent('0')
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

it('refreshes stale pair suggestions and explains that existing organization was preserved', async () => {
  api.capturePairSuggestionsApply.mockResolvedValue({
    ok: false,
    error: {
      code: 'capture_input_invalid',
      userMessage: '采集内容不完整或超过长度限制，请检查后重试。',
      retryable: false,
      diagnosticId: 'diag-pair',
    },
  })
  render(CaptureView, {
    global: { provide: { [syncControllerKey as symbol]: syncController } },
  })
  await fireEvent.click(screen.getByRole('button', { name: 'open batch' }))
  await waitFor(() => expect(api.captureBatchDetail).toHaveBeenCalledTimes(1))

  await fireEvent.click(screen.getByRole('button', { name: 'apply pair' }))

  await waitFor(() => expect(api.captureBatchDetail).toHaveBeenCalledTimes(2))
  expect(api.capturePairSuggestionsApply).toHaveBeenCalledWith({
    batchId: 'batch-1',
    expectedRevision: 1,
    pairIds: ['pair-1'],
  })
  expect(screen.getByTestId('error')).toHaveTextContent(
    '这组题答素材刚刚被移动、改角色或已加入其他题卡，已刷新并保留你的现有整理。',
  )
  expect(syncController.scheduleMutation).not.toHaveBeenCalled()
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

  it('keeps the newest draft text when it arrives during a revision-conflict reload', async () => {
    let resolveReload: (value: unknown) => void = () => undefined
    const refreshedDetail = { ...detail, batch: { ...batch, revision: 2 } }
    api.captureBatchDetail
      .mockResolvedValueOnce(success(detail))
      .mockImplementationOnce(() => new Promise(resolve => { resolveReload = resolve }))
    api.captureDraftUpdate
      .mockResolvedValueOnce({
        ok: false,
        error: {
          code: 'capture_revision_conflict',
          userMessage: '批次已更新',
          retryable: true,
          diagnosticId: 'diag-race',
        },
      })
      .mockResolvedValueOnce(success({
        ...refreshedDetail,
        batch: { ...refreshedDetail.batch, revision: 3 },
      }))
    render(CaptureView)

    await fireEvent.click(screen.getByRole('button', { name: 'open batch' }))
    await waitFor(() => expect(api.captureBatchDetail).toHaveBeenCalledWith('batch-1'))
    await fireEvent.click(screen.getByRole('button', { name: 'update draft' }))
    await waitFor(() => expect(api.captureBatchDetail).toHaveBeenCalledTimes(2))
    await fireEvent.click(screen.getByRole('button', { name: 'update draft newest' }))
    resolveReload(success(refreshedDetail))

    await waitFor(() => expect(api.captureDraftUpdate).toHaveBeenCalledTimes(2))
    expect(api.captureDraftUpdate.mock.calls[1]![0]).toMatchObject({
      expectedRevision: 2,
      note: '冲突后的更新备注',
    })
  })

  it('blocks closing a batch until its in-flight draft save finishes', async () => {
    let resolveSave: (value: unknown) => void = () => undefined
    api.captureDraftUpdate.mockImplementationOnce(() => new Promise(resolve => {
      resolveSave = resolve
    }))
    render(CaptureView)

    await fireEvent.click(screen.getByRole('button', { name: 'open batch' }))
    await waitFor(() => expect(screen.getByTestId('active-batch')).toHaveTextContent('batch-1'))
    await fireEvent.click(screen.getByRole('button', { name: 'update draft' }))
    await waitFor(() => expect(api.captureDraftUpdate).toHaveBeenCalledOnce())
    await fireEvent.click(screen.getByRole('button', { name: 'close batch' }))
    expect(screen.getByTestId('active-batch')).toHaveTextContent('batch-1')
    expect(screen.getByTestId('error')).toHaveTextContent('最新草稿仍在保存，请等待完成后再离开。')

    resolveSave(success({ ...detail, batch: { ...batch, revision: 2 } }))

    await waitFor(() => expect(screen.getByTestId('busy-state')).toHaveTextContent('idle'))
    await fireEvent.click(screen.getByRole('button', { name: 'close batch' }))
    expect(screen.getByTestId('active-batch')).toHaveTextContent('none')
  })

  it('blocks route, close, and batch-switch exits until every queued draft is saved', async () => {
    let resolveFirst: (value: unknown) => void = () => undefined
    let resolveSecond: (value: unknown) => void = () => undefined
    api.captureDraftUpdate
      .mockImplementationOnce(() => new Promise(resolve => { resolveFirst = resolve }))
      .mockImplementationOnce(() => new Promise(resolve => { resolveSecond = resolve }))
    const router = await renderRoutedCapture()

    await fireEvent.click(screen.getByRole('button', { name: 'open batch' }))
    await waitFor(() => expect(screen.getByTestId('active-batch')).toHaveTextContent('batch-1'))
    await router.push({ name: 'inbox', query: { batchId: 'batch-1', section: 'organizing' } })
    expect(router.currentRoute.value.query.section).toBe('organizing')

    await fireEvent.click(screen.getByRole('button', { name: 'update draft' }))
    await fireEvent.click(screen.getByRole('button', { name: 'update draft newest' }))
    await waitFor(() => expect(api.captureDraftUpdate).toHaveBeenCalledOnce())

    await router.push({ name: 'dashboard' })
    expect(router.currentRoute.value.name).toBe('inbox')
    await fireEvent.click(screen.getByRole('button', { name: 'close batch' }))
    await fireEvent.click(screen.getByRole('button', { name: 'open batch two' }))
    expect(screen.getByTestId('active-batch')).toHaveTextContent('batch-1')
    expect(api.captureBatchDetail).not.toHaveBeenCalledWith('batch-2')
    expect(screen.getByTestId('error')).toHaveTextContent('最新草稿仍在保存，请等待完成后再离开。')
    expect(screen.queryByRole('alertdialog')).not.toBeInTheDocument()

    resolveFirst(success({ ...detail, batch: { ...batch, revision: 2 } }))
    await waitFor(() => expect(api.captureDraftUpdate).toHaveBeenCalledTimes(2))
    await router.push({ name: 'dashboard' })
    expect(router.currentRoute.value.name).toBe('inbox')

    resolveSecond(success({ ...detail, batch: { ...batch, revision: 3 } }))
    await waitFor(() => expect(screen.getByTestId('busy-state')).toHaveTextContent('idle'))
    await router.push({ name: 'dashboard' })
    expect(router.currentRoute.value.name).toBe('dashboard')
  })

  it('retains a failed draft snapshot and retries the exact input explicitly', async () => {
    api.captureDraftUpdate
      .mockResolvedValueOnce({
        ok: false,
        error: {
          code: 'capture_draft_update_failed',
          userMessage: '草稿保存失败，请重试。',
          retryable: true,
          diagnosticId: 'diag-draft-save',
        },
      })
      .mockResolvedValueOnce(success({ ...detail, batch: { ...batch, revision: 2 } }))
    await renderRoutedCapture()

    await fireEvent.click(screen.getByRole('button', { name: 'open batch' }))
    await waitFor(() => expect(screen.getByTestId('active-batch')).toHaveTextContent('batch-1'))
    await fireEvent.click(screen.getByRole('button', { name: 'update draft' }))
    const retry = await screen.findByRole('button', { name: 'retry draft save' })
    const firstInput = api.captureDraftUpdate.mock.calls[0]![0]

    expect(screen.getByTestId('save-state')).toHaveTextContent('error')
    await fireEvent.click(retry)
    await waitFor(() => expect(api.captureDraftUpdate).toHaveBeenCalledTimes(2))
    expect(api.captureDraftUpdate.mock.calls[1]![0]).toEqual(firstInput)
    await waitFor(() => expect(screen.queryByRole('button', { name: 'retry draft save' })).not.toBeInTheDocument())
    expect(screen.getByTestId('save-state')).toHaveTextContent('saved')
  })

  it('keeps an older failed draft visible after another draft saves successfully', async () => {
    api.captureDraftUpdate
      .mockResolvedValueOnce({
        ok: false,
        error: {
          code: 'capture_draft_update_failed',
          userMessage: '第一张草稿保存失败。',
          retryable: true,
          diagnosticId: 'diag-first-draft',
        },
      })
      .mockResolvedValueOnce(success({ ...detail, batch: { ...batch, revision: 2 } }))
    render(CaptureView)

    await fireEvent.click(screen.getByRole('button', { name: 'open batch' }))
    await waitFor(() => expect(screen.getByTestId('active-batch')).toHaveTextContent('batch-1'))
    await fireEvent.click(screen.getByRole('button', { name: 'update draft' }))
    await screen.findByRole('button', { name: 'retry draft save' })
    await fireEvent.click(screen.getByRole('button', { name: 'update second draft' }))

    await waitFor(() => expect(api.captureDraftUpdate).toHaveBeenCalledTimes(2))
    await waitFor(() => expect(screen.getByTestId('busy-state')).toHaveTextContent('idle'))
    expect(screen.getByTestId('save-state')).toHaveTextContent('error')
    expect(screen.getByRole('button', { name: 'retry draft save' })).toBeEnabled()
  })

  it('synchronizes clean browser history changes between capture batch ids', async () => {
    const secondBatch = { ...batch, id: 'batch-2', subject: '物理', revision: 4 }
    const secondDetail = { ...detail, batch: secondBatch }
    api.captureBatchDetail.mockImplementation((batchId: string) =>
      Promise.resolve(success(batchId === 'batch-2' ? secondDetail : detail)))
    const router = await renderRoutedCapture()

    await fireEvent.click(screen.getByRole('button', { name: 'open batch' }))
    await waitFor(() => expect(router.currentRoute.value.query.batchId).toBe('batch-1'))
    await router.push({ name: 'inbox', query: { batchId: 'batch-2' } })
    await waitFor(() => expect(api.captureBatchDetail).toHaveBeenCalledWith('batch-2'))
    await waitFor(() => expect(screen.getByTestId('active-batch')).toHaveTextContent('batch-2'))

    await router.back()
    await waitFor(() => expect(screen.getByTestId('active-batch')).toHaveTextContent('batch-1'))
  })

  it('requires explicit confirmation to abandon a retained failed draft', async () => {
    api.captureDraftUpdate.mockResolvedValueOnce({
      ok: false,
      error: {
        code: 'capture_draft_update_failed',
        userMessage: '草稿保存失败，请重试。',
        retryable: true,
        diagnosticId: 'diag-draft-abandon',
      },
    })
    const router = await renderRoutedCapture()

    await fireEvent.click(screen.getByRole('button', { name: 'open batch' }))
    await waitFor(() => expect(screen.getByTestId('active-batch')).toHaveTextContent('batch-1'))
    await fireEvent.click(screen.getByRole('button', { name: 'update draft' }))
    await screen.findByRole('button', { name: 'retry draft save' })
    const unload = new Event('beforeunload', { cancelable: true })
    window.dispatchEvent(unload)
    expect(unload.defaultPrevented).toBe(true)

    const cancelledNavigation = router.push({ name: 'dashboard' })
    expect(await screen.findByRole('alertdialog', { name: '放弃尚未保存的采集草稿？' })).toBeVisible()
    await fireEvent.click(screen.getByRole('button', { name: '继续留在采集箱' }))
    await cancelledNavigation
    expect(router.currentRoute.value.name).toBe('inbox')
    expect(screen.getByTestId('active-batch')).toHaveTextContent('batch-1')

    const confirmedNavigation = router.push({ name: 'dashboard' })
    await fireEvent.click(await screen.findByRole('button', { name: '放弃草稿修改并离开' }))
    await confirmedNavigation
    expect(router.currentRoute.value.name).toBe('dashboard')

    const afterUnmount = new Event('beforeunload', { cancelable: true })
    window.dispatchEvent(afterUnmount)
    expect(afterUnmount.defaultPrevented).toBe(false)
  })

  it('does not stack a draft-leave dialog over an existing destructive confirmation', async () => {
    api.captureBatchDetail.mockResolvedValue(success(editableDetail))
    api.captureDraftUpdate.mockResolvedValueOnce({
      ok: false,
      error: {
        code: 'capture_draft_update_failed',
        userMessage: '草稿保存失败，请重试。',
        retryable: true,
        diagnosticId: 'diag-dialog-stack',
      },
    })
    const router = await renderRoutedCapture()

    await fireEvent.click(screen.getByRole('button', { name: 'open batch' }))
    await waitFor(() => expect(screen.getByTestId('active-batch')).toHaveTextContent('batch-1'))
    await fireEvent.click(screen.getByRole('button', { name: 'update draft' }))
    await screen.findByRole('button', { name: 'retry draft save' })
    await fireEvent.click(screen.getByRole('button', { name: 'remove item' }))
    expect(screen.getByRole('alertdialog', { name: '删除这张采集图片？' })).toBeVisible()

    await router.push({ name: 'dashboard' })
    expect(router.currentRoute.value.name).toBe('inbox')
    expect(screen.getAllByRole('alertdialog')).toHaveLength(1)
    expect(screen.queryByRole('alertdialog', { name: '放弃尚未保存的采集草稿？' })).not.toBeInTheDocument()
    expect(screen.getByTestId('error')).toHaveTextContent('请先完成当前确认操作，再离开采集箱。')
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

  it('does not reopen a batch when an organizer mutation finishes after leaving', async () => {
    let resolveMerge: (value: unknown) => void = () => undefined
    api.captureCardMerge.mockImplementationOnce(() => new Promise(resolve => { resolveMerge = resolve }))
    render(CaptureView)
    await fireEvent.click(screen.getByRole('button', { name: 'open batch' }))
    await waitFor(() => expect(screen.getByTestId('active-batch')).toHaveTextContent('batch-1'))
    await fireEvent.click(screen.getByRole('button', { name: 'merge card' }))
    await waitFor(() => expect(api.captureCardMerge).toHaveBeenCalledOnce())
    await fireEvent.click(screen.getByRole('button', { name: 'close batch' }))
    expect(screen.getByTestId('active-batch')).toHaveTextContent('none')

    resolveMerge(success({ ...detail, batch: { ...batch, revision: 2 } }))
    await waitFor(() => expect(screen.getByTestId('busy-state')).toHaveTextContent('idle'))
    expect(screen.getByTestId('active-batch')).toHaveTextContent('none')
  })

  it('does not surface a detail error after leaving the requested batch', async () => {
    let rejectDetail: (reason?: unknown) => void = () => undefined
    const request = new Promise((_resolve, reject) => { rejectDetail = reject })
    api.captureBatchDetail.mockReturnValueOnce(request)
    render(CaptureView)
    await fireEvent.click(screen.getByRole('button', { name: 'open batch' }))
    await waitFor(() => expect(api.captureBatchDetail).toHaveBeenCalledWith('batch-1'))
    await fireEvent.click(screen.getByRole('button', { name: 'close batch' }))
    expect(screen.getByTestId('active-batch')).toHaveTextContent('none')

    rejectDetail(new Error('late detail failure'))
    await request.catch(() => undefined)
    await nextTick()
    expect(screen.getByTestId('error')).toBeEmptyDOMElement()
  })

  it('keeps a batch closed when commit finishes after leaving while preserving sync', async () => {
    let resolveCommit: (value: unknown) => void = () => undefined
    api.captureCommitReady.mockImplementationOnce(() => new Promise(resolve => { resolveCommit = resolve }))
    render(CaptureView, {
      global: { provide: { [syncControllerKey as symbol]: syncController } },
    })
    await fireEvent.click(screen.getByRole('button', { name: 'open batch' }))
    await waitFor(() => expect(screen.getByTestId('active-batch')).toHaveTextContent('batch-1'))
    await fireEvent.click(screen.getByRole('button', { name: 'commit ready' }))
    await waitFor(() => expect(api.captureCommitReady).toHaveBeenCalledWith('batch-1', 1))
    await fireEvent.click(screen.getByRole('button', { name: 'close batch' }))
    expect(screen.getByTestId('active-batch')).toHaveTextContent('none')

    resolveCommit(success({
      committedProblemIds: ['problem-1'], committedCount: 1, remainingDraftCount: 0,
    }))
    await waitFor(() => expect(screen.getByTestId('busy-state')).toHaveTextContent('idle'))
    expect(screen.getByTestId('active-batch')).toHaveTextContent('none')
    expect(api.captureBatchDetail).toHaveBeenCalledTimes(1)
    expect(syncController.scheduleMutation).toHaveBeenCalledOnce()
  })

  it('clears committed feedback when leaving during the detail refresh', async () => {
    let resolveRefresh: (value: unknown) => void = () => undefined
    api.captureBatchDetail
      .mockResolvedValueOnce(success(detail))
      .mockImplementationOnce(() => new Promise(resolve => { resolveRefresh = resolve }))
    render(CaptureView, {
      global: { provide: { [syncControllerKey as symbol]: syncController } },
    })
    await fireEvent.click(screen.getByRole('button', { name: 'open batch' }))
    await waitFor(() => expect(screen.getByTestId('active-batch')).toHaveTextContent('batch-1'))
    await fireEvent.click(screen.getByRole('button', { name: 'commit ready' }))
    await waitFor(() => expect(screen.getByTestId('commit-message')).toHaveTextContent('已将 1 道题加入题库。'))

    await fireEvent.click(screen.getByRole('button', { name: 'close batch' }))
    expect(screen.getByTestId('active-batch')).toHaveTextContent('none')
    expect(screen.getByTestId('commit-message')).toBeEmptyDOMElement()

    resolveRefresh(success(detail))
    await waitFor(() => expect(screen.getByTestId('busy-state')).toHaveTextContent('idle'))
    expect(screen.getByTestId('active-batch')).toHaveTextContent('none')
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

  it('blocks route, workspace, and window transitions during a capture mutation', async () => {
    let resolveAssignment: (value: unknown) => void = () => undefined
    api.captureBatchAssignSubject.mockImplementationOnce(() =>
      new Promise(resolve => { resolveAssignment = resolve }))
    const { router, view, workspaceTransitionGuard } = await renderGuardedRoutedCapture()

    await fireEvent.click(screen.getByRole('button', { name: 'open batch' }))
    await waitFor(() => expect(screen.getByTestId('active-batch')).toHaveTextContent('batch-1'))
    await fireEvent.click(screen.getByRole('button', { name: 'assign subject' }))
    await waitFor(() => expect(api.captureBatchAssignSubject).toHaveBeenCalledOnce())

    await expect(workspaceTransitionGuard.attempt()).resolves.toBe(false)
    expect(screen.getByTestId('error')).toHaveTextContent('采集操作正在完成，请等待完成后再离开。')
    const busyUnload = new Event('beforeunload', { cancelable: true })
    window.dispatchEvent(busyUnload)
    expect(busyUnload.defaultPrevented).toBe(true)
    await router.push({ name: 'dashboard' })
    expect(router.currentRoute.value.name).toBe('inbox')

    resolveAssignment(success({ ...detail, batch: { ...batch, subject: '化学', revision: 2 } }))
    await waitFor(() => expect(screen.getByTestId('busy-state')).toHaveTextContent('idle'))
    expect(screen.getByTestId('error')).toBeEmptyDOMElement()
    await expect(workspaceTransitionGuard.attempt()).resolves.toBe(true)
    const idleUnload = new Event('beforeunload', { cancelable: true })
    window.dispatchEvent(idleUnload)
    expect(idleUnload.defaultPrevented).toBe(false)
    await router.push({ name: 'dashboard' })
    expect(router.currentRoute.value.name).toBe('dashboard')

    view.unmount()
  })

  it('makes custom subjects from settings available in the capture workspace', async () => {
    api.subjectPreferencesGet.mockResolvedValue(success({
      enabledSubjects: ['数学'], customSubjects: ['编程', '竞赛'], captureSoundEnabled: true,
    }))
    render(CaptureView)

    await waitFor(() => expect(screen.getByTestId('subjects')).toHaveTextContent('数学、编程、竞赛'))
  })

  it('does not reopen a batch when the system picker finishes after leaving', async () => {
    let resolveSelect: (value: unknown) => void = () => undefined
    api.captureImportSelect.mockImplementationOnce(() => new Promise(resolve => { resolveSelect = resolve }))
    render(CaptureView)
    await fireEvent.click(screen.getByRole('button', { name: 'open batch' }))
    await waitFor(() => expect(screen.getByTestId('active-batch')).toHaveTextContent('batch-1'))
    await fireEvent.click(screen.getByRole('button', { name: 'import select' }))
    await waitFor(() => expect(api.captureImportSelect).toHaveBeenCalledWith('batch-1'))
    await fireEvent.click(screen.getByRole('button', { name: 'close batch' }))
    expect(screen.getByTestId('active-batch')).toHaveTextContent('none')

    resolveSelect(success({ importedItems: [], importedCount: 1 }))
    await waitFor(() => expect(screen.getByTestId('busy-state')).toHaveTextContent('idle'))
    expect(screen.getByTestId('active-batch')).toHaveTextContent('none')
    expect(api.captureBatchDetail).toHaveBeenCalledTimes(1)
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

  it('keeps the workspace busy until the post-import refresh finishes', async () => {
    let resolveRefresh: (value: unknown) => void = () => undefined
    api.captureBatchDetail
      .mockResolvedValueOnce(success(detail))
      .mockImplementationOnce(() => new Promise(resolve => { resolveRefresh = resolve }))
    render(CaptureView)

    await fireEvent.click(screen.getByRole('button', { name: 'open batch' }))
    await waitFor(() => expect(screen.getByTestId('active-batch')).toHaveTextContent('batch-1'))
    await fireEvent.click(screen.getByRole('button', { name: 'import files' }))
    await waitFor(() => expect(api.captureBatchDetail).toHaveBeenCalledTimes(2))

    expect(screen.getByTestId('busy-state')).toHaveTextContent('busy')
    resolveRefresh(success(detail))
    await waitFor(() => expect(screen.getByTestId('busy-state')).toHaveTextContent('idle'))
  })

  it('does not reopen a batch when file imports finish after leaving', async () => {
    let resolveFirst: (value: unknown) => void = () => undefined
    let resolveSecond: (value: unknown) => void = () => undefined
    api.captureImportBytes
      .mockImplementationOnce(() => new Promise(resolve => { resolveFirst = resolve }))
      .mockImplementationOnce(() => new Promise(resolve => { resolveSecond = resolve }))
    render(CaptureView)

    await fireEvent.click(screen.getByRole('button', { name: 'open batch' }))
    await waitFor(() => expect(screen.getByTestId('active-batch')).toHaveTextContent('batch-1'))
    await fireEvent.click(screen.getByRole('button', { name: 'import files' }))
    await waitFor(() => expect(api.captureImportBytes).toHaveBeenCalledTimes(2))
    expect(screen.getByTestId('import-progress')).toHaveTextContent('0/2')
    await fireEvent.click(screen.getByRole('button', { name: 'close batch' }))
    expect(screen.getByTestId('active-batch')).toHaveTextContent('none')
    expect(screen.getByTestId('import-progress')).toHaveTextContent('none')

    resolveFirst(success({}))
    resolveSecond(success({}))

    await waitFor(() => expect(screen.getByTestId('busy-state')).toHaveTextContent('idle'))
    expect(screen.getByTestId('active-batch')).toHaveTextContent('none')
  })

  it('explains the 150-image batch limit instead of silently truncating', async () => {
    render(CaptureView)

    await fireEvent.click(screen.getByRole('button', { name: 'open batch' }))
    await waitFor(() => expect(api.captureBatchDetail).toHaveBeenCalledWith('batch-1'))
    await fireEvent.click(screen.getByRole('button', { name: 'import overflow' }))

    await waitFor(() => expect(api.captureImportBytes).toHaveBeenCalledTimes(150))
    await waitFor(() => expect(screen.getByTestId('error')).toHaveTextContent('本批最多保存 150 张'))
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

  it('does not restore a stopped session from an older status poll', async () => {
    let resolveStatus: (value: unknown) => void = () => undefined
    let statusReturned = false
    api.captureLanStatus.mockImplementationOnce(async () => {
      const result = await new Promise(resolve => { resolveStatus = resolve })
      statusReturned = true
      return result
    })
    render(CaptureView)

    await openBatchAndScan()
    await waitFor(() => expect(screen.getByTestId('session')).toHaveTextContent('session-1'))
    await fireEvent.click(screen.getByRole('button', { name: 'stop mobile' }))
    await waitFor(() => expect(screen.getByTestId('session')).toHaveTextContent('none'))

    resolveStatus(success(session))
    await waitFor(() => expect(statusReturned).toBe(true))
    await nextTick()

    expect(screen.getByTestId('session')).toHaveTextContent('none')
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
