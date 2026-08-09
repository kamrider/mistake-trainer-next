import { describe, expect, it, vi } from 'vitest'
import type {
  CaptureBatchDetail,
  CaptureBatchSummary,
  CaptureCommitReport,
} from '../../../shared/api/bindings'
import { failure, success, type AppResult } from '../../../shared/api/app-result'
import { useCaptureBatchLifecycle } from './useCaptureBatchLifecycle'

const batch = {
  id: 'batch-1', subject: '数学', state: 'collecting', itemCount: 1,
  draftCount: 1, readyCount: 1, updatedAtUtcMs: 1, revision: 7,
} as CaptureBatchSummary
const detail = {
  batch, items: [], drafts: [], unassignedItemIds: [], pairSuggestions: [],
} as CaptureBatchDetail
const created = { ...batch, id: 'batch-new', revision: 1 }
const committed = {
  committedProblemIds: ['problem-1'], committedCount: 1, remainingDraftCount: 0,
} satisfies CaptureCommitReport

function deferred<T>() {
  let resolve!: (value: T) => void
  let reject!: (reason?: unknown) => void
  const promise = new Promise<T>((finish, fail) => { resolve = finish; reject = fail })
  return { promise, resolve, reject }
}

function harness() {
  let current: CaptureBatchDetail | undefined = detail
  let blocked = false
  let lanBatchId: string | undefined
  const order: string[] = []
  const operations = {
    create: vi.fn(async () => { order.push('create'); return success(created) }),
    discard: vi.fn(async () => { order.push('discard'); return success(true) }),
    update: vi.fn(async () => { order.push('update'); return success(batch) }),
    commit: vi.fn(async () => { order.push('commit'); return success(committed) }),
  }
  const onBusyChange = vi.fn((value: boolean) => { blocked = value })
  const onError = vi.fn()
  const onCommitMessage = vi.fn()
  const onActiveBatchDiscarded = vi.fn(() => { current = undefined })
  const loadBatches = vi.fn(async () => { order.push('load-batches') })
  const loadDetail = vi.fn(async () => { order.push('load-detail') })
  const stopMobileCapture = vi.fn(async () => { order.push('stop-lan') })
  const scheduleSyncMutation = vi.fn(() => { order.push('schedule-sync') })
  const controller = useCaptureBatchLifecycle({
    desktopAvailable: true,
    activeDetail: () => current,
    activeLanBatchId: () => lanBatchId,
    isBlocked: () => blocked,
    onBusyChange,
    onError,
    onCommitMessage,
    onActiveBatchDiscarded,
    loadBatches,
    loadDetail,
    stopMobileCapture,
    scheduleSyncMutation,
    operations,
  })
  return {
    controller, operations, order, onBusyChange, onError, onCommitMessage,
    onActiveBatchDiscarded, loadBatches, loadDetail, stopMobileCapture,
    scheduleSyncMutation,
    setCurrent: (value?: CaptureBatchDetail) => { current = value },
    setLanBatchId: (value?: string) => { lanBatchId = value },
    setBlocked: (value: boolean) => { blocked = value },
  }
}

describe('useCaptureBatchLifecycle', () => {
  it('builds exact command inputs and preserves refresh ordering', async () => {
    const create = harness()
    await create.controller.createBatch('物理')
    expect(create.operations.create).toHaveBeenCalledWith({ subject: '物理' })
    expect(create.order).toEqual(['create', 'load-batches', 'load-detail'])
    expect(create.loadDetail).toHaveBeenCalledWith('batch-new')

    const discard = harness()
    discard.setLanBatchId('batch-1')
    await discard.controller.discardBatch('batch-1')
    expect(discard.operations.discard).toHaveBeenCalledWith('batch-1')
    expect(discard.order).toEqual(['stop-lan', 'discard', 'load-batches'])
    expect(discard.stopMobileCapture).toHaveBeenCalledWith(true)
    expect(discard.onActiveBatchDiscarded).toHaveBeenCalledWith('batch-1')

    const finish = harness()
    finish.setLanBatchId('batch-1')
    await finish.controller.finishCollecting('化学')
    expect(finish.operations.update).toHaveBeenCalledWith({
      batchId: 'batch-1', expectedRevision: 7, subject: '化学', finishCollecting: true,
    })
    expect(finish.order).toEqual(['stop-lan', 'update', 'load-detail'])
    expect(finish.loadDetail).toHaveBeenCalledWith('batch-1')

    const commit = harness()
    await commit.controller.commitReady()
    expect(commit.operations.commit).toHaveBeenCalledWith('batch-1', 7)
    expect(commit.order).toEqual(['commit', 'schedule-sync', 'load-detail', 'load-batches'])
  })

  it('owns busy state and uses command or action-specific failure copy', async () => {
    const rejected = harness()
    rejected.operations.update.mockResolvedValue(
      failure('capture_revision_conflict', '批次已更新', true, 'diag-conflict'),
    )
    await rejected.controller.finishCollecting('数学')
    expect(rejected.onBusyChange.mock.calls).toEqual([[true], [false]])
    expect(rejected.onError).toHaveBeenCalledWith('批次已更新')
    expect(rejected.loadDetail).not.toHaveBeenCalled()

    const cases = [
      ['createBatch', '新批次没有创建成功，请稍后重试。'],
      ['discardBatch', '批次没有删除成功，原有图片仍会保留。'],
      ['finishCollecting', '没有结束采集，请稍后重试。'],
      ['commitReady', '批量入库没有完成，所有草稿仍保持原样，可以直接重试。'],
    ] as const
    for (const [action, copy] of cases) {
      const current = harness()
      if (action === 'createBatch') current.operations.create.mockRejectedValue(new Error('offline'))
      if (action === 'discardBatch') current.operations.discard.mockRejectedValue(new Error('offline'))
      if (action === 'finishCollecting') current.operations.update.mockRejectedValue(new Error('offline'))
      if (action === 'commitReady') current.operations.commit.mockRejectedValue(new Error('offline'))
      if (action === 'createBatch') await current.controller.createBatch('数学')
      if (action === 'discardBatch') await current.controller.discardBatch('batch-1')
      if (action === 'finishCollecting') await current.controller.finishCollecting('数学')
      if (action === 'commitReady') await current.controller.commitReady()
      expect(current.onError).toHaveBeenLastCalledWith(copy)
    }
  })

  it('forwards recoverable command errors for every lifecycle mutation', async () => {
    const create = harness()
    create.operations.create.mockResolvedValue(
      failure('capture_busy', 'create-failed', true, 'diag-create'),
    )
    await create.controller.createBatch('数学')
    expect(create.onError).toHaveBeenLastCalledWith('create-failed')

    const discard = harness()
    discard.operations.discard.mockResolvedValue(
      failure('capture_busy', 'discard-failed', true, 'diag-discard'),
    )
    await discard.controller.discardBatch('batch-1')
    expect(discard.onError).toHaveBeenLastCalledWith('discard-failed')

    const commit = harness()
    commit.operations.commit.mockResolvedValue(
      failure('capture_busy', 'commit-failed', true, 'diag-commit'),
    )
    await commit.controller.commitReady()
    expect(commit.onError).toHaveBeenLastCalledWith('commit-failed')
  })

  it('reports committed counts and schedules sync only for durable additions', async () => {
    const positive = harness()
    await positive.controller.commitReady()
    expect(positive.onCommitMessage).toHaveBeenNthCalledWith(1, '')
    expect(positive.onCommitMessage).toHaveBeenLastCalledWith('已将 1 道题加入题库。')
    expect(positive.scheduleSyncMutation).toHaveBeenCalledOnce()

    const empty = harness()
    empty.operations.commit.mockResolvedValue(success({
      committedProblemIds: [], committedCount: 0, remainingDraftCount: 1,
    }))
    await empty.controller.commitReady()
    expect(empty.onCommitMessage).toHaveBeenLastCalledWith('没有可加入题库的完整题卡。')
    expect(empty.scheduleSyncMutation).not.toHaveBeenCalled()
  })

  it('keeps durable side effects but ignores stale detail and message updates', async () => {
    const gate = deferred<AppResult<CaptureCommitReport>>()
    const current = harness()
    current.operations.commit.mockReturnValue(gate.promise)
    const committing = current.controller.commitReady()
    await vi.waitFor(() => expect(current.operations.commit).toHaveBeenCalledOnce())
    current.setCurrent(undefined)
    gate.resolve(success(committed))
    await committing
    expect(current.onCommitMessage).toHaveBeenCalledTimes(1)
    expect(current.onCommitMessage).toHaveBeenLastCalledWith('')
    expect(current.loadDetail).not.toHaveBeenCalled()
    expect(current.loadBatches).toHaveBeenCalledOnce()
    expect(current.scheduleSyncMutation).toHaveBeenCalledOnce()
  })

  it('ignores late failures and same-batch stale revisions', async () => {
    const failedGate = deferred<AppResult<CaptureCommitReport>>()
    const failed = harness()
    failed.operations.commit.mockReturnValue(failedGate.promise)
    const committing = failed.controller.commitReady()
    await vi.waitFor(() => expect(failed.operations.commit).toHaveBeenCalledOnce())
    failed.setCurrent(undefined)
    failedGate.reject(new Error('late offline'))
    await committing
    expect(failed.onError).not.toHaveBeenCalled()

    const finishGate = deferred<AppResult<CaptureBatchSummary>>()
    const stale = harness()
    stale.operations.update.mockReturnValue(finishGate.promise)
    const finishing = stale.controller.finishCollecting('数学')
    await vi.waitFor(() => expect(stale.operations.update).toHaveBeenCalledOnce())
    stale.setCurrent({ ...detail, batch: { ...batch, revision: 8 } })
    finishGate.resolve(success({ ...batch, state: 'organizing', revision: 8 }))
    await finishing
    expect(stale.loadDetail).not.toHaveBeenCalled()
    expect(stale.loadBatches).toHaveBeenCalledOnce()
  })

  it('ignores unavailable, blocked, and batchless actions', async () => {
    const blocked = harness()
    blocked.setBlocked(true)
    await blocked.controller.createBatch('数学')
    await blocked.controller.discardBatch('batch-1')
    await blocked.controller.finishCollecting('数学')
    await blocked.controller.commitReady()
    expect(blocked.operations.create).not.toHaveBeenCalled()
    expect(blocked.operations.discard).not.toHaveBeenCalled()
    expect(blocked.operations.update).not.toHaveBeenCalled()
    expect(blocked.operations.commit).not.toHaveBeenCalled()

    const batchless = harness()
    batchless.setCurrent(undefined)
    await batchless.controller.finishCollecting('数学')
    await batchless.controller.commitReady()
    expect(batchless.operations.update).not.toHaveBeenCalled()
    expect(batchless.operations.commit).not.toHaveBeenCalled()
  })
})
