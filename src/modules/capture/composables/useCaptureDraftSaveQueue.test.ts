import { describe, expect, it, vi } from 'vitest'
import {
  useCaptureDraftSaveQueue,
  type CaptureDraftSaveOutcome,
  type CaptureDraftSaveQueueState,
  type CaptureDraftSaveUpdate,
} from './useCaptureDraftSaveQueue'

function deferred<T>() {
  let resolve!: (value: T) => void
  const promise = new Promise<T>((finish) => {
    resolve = finish
  })
  return { promise, resolve }
}

function update(note: string, batchId = 'batch-1', draftId = 'draft-1'): CaptureDraftSaveUpdate {
  return {
    batchId,
    draftId,
    subject: '数学',
    tags: ['函数'],
    note,
  }
}

function createHarness(
  perform: (input: CaptureDraftSaveUpdate) => Promise<CaptureDraftSaveOutcome>,
) {
  let activeBatchId: string | undefined = 'batch-1'
  let blocked = false
  const onSaving = vi.fn()
  const onSaved = vi.fn()
  const onFailed = vi.fn()
  const onBusyChange = vi.fn()
  const states: CaptureDraftSaveQueueState[] = []
  const onStateChange = vi.fn((state: CaptureDraftSaveQueueState) => {
    states.push({ ...state })
  })
  const refresh = vi.fn().mockResolvedValue(undefined)
  const queue = useCaptureDraftSaveQueue({
    activeBatchId: () => activeBatchId,
    isBlocked: () => blocked,
    perform,
    refresh,
    onSaving,
    onSaved,
    onFailed,
    onBusyChange,
    onStateChange,
  })
  return {
    queue,
    refresh,
    onSaving,
    onSaved,
    onFailed,
    onBusyChange,
    onStateChange,
    states,
    setActiveBatchId: (value: string | undefined) => { activeBatchId = value },
    setBlocked: (value: boolean) => { blocked = value },
  }
}

describe('useCaptureDraftSaveQueue', () => {
  it('keeps a newer edit queued while the previous save is in flight', async () => {
    const first = deferred<CaptureDraftSaveOutcome>()
    const perform = vi.fn()
      .mockReturnValueOnce(first.promise)
      .mockResolvedValueOnce({ kind: 'saved' })
    const { queue } = createHarness(perform)

    queue.enqueue(update('first'))
    await vi.waitFor(() => expect(perform).toHaveBeenCalledTimes(1))
    queue.enqueue(update('latest'))
    first.resolve({ kind: 'saved' })

    await vi.waitFor(() => expect(perform).toHaveBeenCalledTimes(2))
    expect(perform).toHaveBeenNthCalledWith(2, expect.objectContaining({ note: 'latest' }))
  })

  it('does not let an older conflict retry overwrite a newer queued edit', async () => {
    const refreshGate = deferred<void>()
    const perform = vi.fn()
      .mockResolvedValueOnce({ kind: 'revision_conflict', message: '批次已更新' })
      .mockResolvedValueOnce({ kind: 'saved' })
    const harness = createHarness(perform)
    harness.refresh.mockReturnValueOnce(refreshGate.promise)

    harness.queue.enqueue(update('old'))
    await vi.waitFor(() => expect(harness.refresh).toHaveBeenCalledOnce())
    harness.queue.enqueue(update('new'))
    refreshGate.resolve()

    await vi.waitFor(() => expect(perform).toHaveBeenCalledTimes(2))
    expect(perform).toHaveBeenNthCalledWith(2, expect.objectContaining({ note: 'new' }))
  })

  it('retries one revision conflict once when no newer edit exists', async () => {
    const perform = vi.fn()
      .mockResolvedValueOnce({ kind: 'revision_conflict', message: '批次已更新' })
      .mockResolvedValueOnce({ kind: 'saved' })
    const harness = createHarness(perform)

    harness.queue.enqueue(update('retry me'))

    await vi.waitFor(() => expect(perform).toHaveBeenCalledTimes(2))
    expect(harness.refresh).toHaveBeenCalledOnce()
    expect(perform).toHaveBeenNthCalledWith(2, expect.objectContaining({ note: 'retry me' }))
  })

  it('reports a failed save without retrying it', async () => {
    const perform = vi.fn().mockResolvedValue({
      kind: 'failed',
      message: '草稿没有保存',
    })
    const harness = createHarness(perform)

    harness.queue.enqueue(update('failed'))

    await vi.waitFor(() => expect(harness.onFailed).toHaveBeenCalledWith('草稿没有保存'))
    expect(perform).toHaveBeenCalledOnce()
    expect(harness.refresh).not.toHaveBeenCalled()
  })

  it('reports a revision conflict when the single retry also conflicts', async () => {
    const perform = vi.fn()
      .mockResolvedValueOnce({ kind: 'revision_conflict', message: '批次已更新' })
      .mockResolvedValueOnce({ kind: 'revision_conflict', message: '请重新编辑后再试' })
    const harness = createHarness(perform)

    harness.queue.enqueue(update('retry once'))

    await vi.waitFor(() => {
      expect(harness.onFailed).toHaveBeenCalledWith('请重新编辑后再试')
    })
    expect(perform).toHaveBeenCalledTimes(2)
    expect(harness.refresh).toHaveBeenCalledOnce()
  })

  it('waits while blocked and resumes explicitly', async () => {
    const perform = vi.fn().mockResolvedValue({ kind: 'saved' })
    const harness = createHarness(perform)
    harness.setBlocked(true)

    harness.queue.enqueue(update('waiting'))
    await Promise.resolve()
    expect(perform).not.toHaveBeenCalled()

    harness.setBlocked(false)
    await harness.queue.flush()
    expect(perform).toHaveBeenCalledWith(expect.objectContaining({ note: 'waiting' }))
  })

  it('drops inactive batches and ignores work after disposal', async () => {
    const perform = vi.fn().mockResolvedValue({ kind: 'saved' })
    const harness = createHarness(perform)
    harness.setBlocked(true)
    harness.queue.enqueue(update('stale', 'batch-1'))
    harness.queue.enqueue(update('active', 'batch-2'))
    harness.setActiveBatchId('batch-2')
    harness.queue.retainBatch('batch-2')
    harness.setBlocked(false)

    await harness.queue.flush()
    expect(perform).toHaveBeenCalledOnce()
    expect(perform).toHaveBeenCalledWith(expect.objectContaining({
      batchId: 'batch-2',
      note: 'active',
    }))

    harness.setBlocked(true)
    harness.queue.enqueue(update('disposed', 'batch-2'))
    harness.queue.dispose()
    harness.setBlocked(false)
    await harness.queue.flush()
    harness.queue.enqueue(update('ignored', 'batch-2'))
    await Promise.resolve()
    expect(perform).toHaveBeenCalledOnce()
  })

  it('ignores an in-flight completion after the active batch changes', async () => {
    const saveGate = deferred<CaptureDraftSaveOutcome>()
    const perform = vi.fn().mockReturnValue(saveGate.promise)
    const harness = createHarness(perform)

    harness.queue.enqueue(update('leaving'))
    await vi.waitFor(() => expect(perform).toHaveBeenCalledOnce())
    harness.setActiveBatchId('batch-2')
    harness.queue.retainBatch('batch-2')
    saveGate.resolve({ kind: 'saved' })

    await vi.waitFor(() => {
      expect(harness.onBusyChange).toHaveBeenLastCalledWith(false)
    })
    expect(harness.onSaved).not.toHaveBeenCalled()
    expect(harness.onFailed).not.toHaveBeenCalled()
  })

  it('does not requeue an old conflict after leaving during refresh', async () => {
    const refreshGate = deferred<void>()
    const perform = vi.fn().mockResolvedValue({
      kind: 'revision_conflict',
      message: '批次已更新',
    })
    const harness = createHarness(perform)
    harness.refresh.mockReturnValueOnce(refreshGate.promise)

    harness.queue.enqueue(update('leave during refresh'))
    await vi.waitFor(() => expect(harness.refresh).toHaveBeenCalledOnce())
    harness.setActiveBatchId('batch-2')
    harness.queue.retainBatch('batch-2')
    refreshGate.resolve()
    await vi.waitFor(() => {
      expect(harness.onBusyChange).toHaveBeenLastCalledWith(false)
    })

    harness.setActiveBatchId('batch-1')
    await harness.queue.flush()
    expect(perform).toHaveBeenCalledOnce()
  })

  it('publishes pending and running state until every queued revision is saved', async () => {
    const first = deferred<CaptureDraftSaveOutcome>()
    const second = deferred<CaptureDraftSaveOutcome>()
    const perform = vi.fn()
      .mockReturnValueOnce(first.promise)
      .mockReturnValueOnce(second.promise)
    const harness = createHarness(perform)

    expect(harness.states.at(-1)).toEqual({
      pending: false,
      running: false,
      retryRequired: false,
    })
    harness.queue.enqueue(update('first'))
    await vi.waitFor(() => expect(perform).toHaveBeenCalledOnce())
    expect(harness.states.at(-1)).toEqual({
      pending: false,
      running: true,
      retryRequired: false,
    })

    harness.queue.enqueue(update('latest'))
    expect(harness.states.at(-1)).toEqual({
      pending: true,
      running: true,
      retryRequired: false,
    })
    first.resolve({ kind: 'saved' })
    await vi.waitFor(() => expect(perform).toHaveBeenCalledTimes(2))
    expect(harness.states.at(-1)).toEqual({
      pending: false,
      running: true,
      retryRequired: false,
    })

    second.resolve({ kind: 'saved' })
    await vi.waitFor(() => expect(harness.states.at(-1)).toEqual({
      pending: false,
      running: false,
      retryRequired: false,
    }))
  })

  it('retains a failed snapshot without looping and retries it explicitly', async () => {
    const perform = vi.fn()
      .mockResolvedValueOnce({ kind: 'failed', message: '网络中断，草稿尚未保存' })
      .mockResolvedValueOnce({ kind: 'saved' })
    const harness = createHarness(perform)

    harness.queue.enqueue(update('retry exactly'))
    await vi.waitFor(() => expect(harness.onFailed).toHaveBeenCalledOnce())
    expect(perform).toHaveBeenCalledOnce()
    expect(harness.states.at(-1)).toEqual({
      pending: false,
      running: false,
      retryRequired: true,
    })

    await harness.queue.retry()
    expect(perform).toHaveBeenCalledTimes(2)
    expect(perform).toHaveBeenNthCalledWith(2, expect.objectContaining({
      batchId: 'batch-1',
      draftId: 'draft-1',
      note: 'retry exactly',
    }))
    expect(harness.states.at(-1)).toEqual({
      pending: false,
      running: false,
      retryRequired: false,
    })
  })

  it('retains an unexpected failure for retry and clears it after success', async () => {
    const perform = vi.fn()
      .mockRejectedValueOnce(new Error('offline'))
      .mockResolvedValueOnce({ kind: 'saved' })
    const harness = createHarness(perform)

    harness.queue.enqueue(update('survive exception'))
    await vi.waitFor(() => expect(harness.onFailed).toHaveBeenCalledOnce())
    expect(harness.states.at(-1)?.retryRequired).toBe(true)

    await harness.queue.retry()
    expect(perform).toHaveBeenCalledTimes(2)
    expect(harness.states.at(-1)?.retryRequired).toBe(false)
  })

  it('replaces a failed snapshot when the same draft is edited again', async () => {
    const perform = vi.fn()
      .mockResolvedValueOnce({ kind: 'failed', message: '旧版本没有保存' })
      .mockResolvedValueOnce({ kind: 'saved' })
    const harness = createHarness(perform)

    harness.queue.enqueue(update('failed old'))
    await vi.waitFor(() => expect(harness.states.at(-1)?.retryRequired).toBe(true))
    harness.queue.enqueue(update('new edit'))

    await vi.waitFor(() => expect(perform).toHaveBeenCalledTimes(2))
    expect(perform).toHaveBeenNthCalledWith(2, expect.objectContaining({ note: 'new edit' }))
    await vi.waitFor(() => expect(harness.states.at(-1)).toEqual({
      pending: false,
      running: false,
      retryRequired: false,
    }))
  })

  it('does not restore an older failure over a newer queued edit', async () => {
    const first = deferred<CaptureDraftSaveOutcome>()
    const perform = vi.fn()
      .mockReturnValueOnce(first.promise)
      .mockResolvedValueOnce({ kind: 'saved' })
    const harness = createHarness(perform)

    harness.queue.enqueue(update('old in flight'))
    await vi.waitFor(() => expect(perform).toHaveBeenCalledOnce())
    harness.queue.enqueue(update('new queued'))
    first.resolve({ kind: 'failed', message: '旧版本失败' })

    await vi.waitFor(() => expect(perform).toHaveBeenCalledTimes(2))
    expect(perform).toHaveBeenNthCalledWith(2, expect.objectContaining({ note: 'new queued' }))
    await vi.waitFor(() => expect(harness.states.at(-1)?.retryRequired).toBe(false))
  })

  it('drops failed snapshots that do not belong to the retained batch', async () => {
    const perform = vi.fn().mockResolvedValue({
      kind: 'failed',
      message: '批次一保存失败',
    })
    const harness = createHarness(perform)

    harness.queue.enqueue(update('batch one failed'))
    await vi.waitFor(() => expect(harness.states.at(-1)?.retryRequired).toBe(true))
    harness.setActiveBatchId('batch-2')
    harness.queue.retainBatch('batch-2')

    expect(harness.states.at(-1)).toEqual({
      pending: false,
      running: false,
      retryRequired: false,
    })
    await harness.queue.retry()
    expect(perform).toHaveBeenCalledOnce()
  })

  it('keeps a failed snapshot when retaining its own batch', async () => {
    const perform = vi.fn().mockResolvedValue({
      kind: 'failed',
      message: '仍需重试',
    })
    const harness = createHarness(perform)

    harness.queue.enqueue(update('same batch failure'))
    await vi.waitFor(() => expect(harness.states.at(-1)?.retryRequired).toBe(true))
    harness.queue.retainBatch('batch-1')

    expect(harness.states.at(-1)?.retryRequired).toBe(true)
  })

  it('keeps a queued update when there is no active batch and saves it after activation', async () => {
    const perform = vi.fn().mockResolvedValue({ kind: 'saved' })
    const harness = createHarness(perform)
    harness.setActiveBatchId(undefined)

    harness.queue.enqueue(update('wait for active batch'))
    await harness.queue.flush()
    expect(perform).not.toHaveBeenCalled()
    expect(harness.states.at(-1)?.pending).toBe(true)

    harness.setActiveBatchId('batch-1')
    await harness.queue.flush()
    expect(perform).toHaveBeenCalledOnce()
    expect(harness.states.at(-1)?.pending).toBe(false)
  })

  it('keeps inactive failures untouched until their batch is active for retry', async () => {
    const perform = vi.fn()
      .mockResolvedValueOnce({ kind: 'failed', message: '稍后重试' })
      .mockResolvedValueOnce({ kind: 'saved' })
    const harness = createHarness(perform)

    harness.queue.enqueue(update('inactive retry'))
    await vi.waitFor(() => expect(harness.states.at(-1)?.retryRequired).toBe(true))
    harness.setActiveBatchId('batch-2')
    await harness.queue.retry()
    expect(perform).toHaveBeenCalledOnce()
    expect(harness.states.at(-1)?.retryRequired).toBe(true)

    harness.setActiveBatchId('batch-1')
    await harness.queue.retry()
    expect(perform).toHaveBeenCalledTimes(2)
    expect(harness.states.at(-1)?.retryRequired).toBe(false)
  })

  it('ignores retry without an active batch and after disposal', async () => {
    const perform = vi.fn().mockResolvedValue({ kind: 'saved' })
    const harness = createHarness(perform)

    harness.setActiveBatchId(undefined)
    await harness.queue.retry()
    expect(perform).not.toHaveBeenCalled()

    harness.queue.dispose()
    harness.setActiveBatchId('batch-1')
    await harness.queue.retry()
    expect(perform).not.toHaveBeenCalled()
  })

  it('does not retain or report an in-flight exception after disposal', async () => {
    let rejectSave: (reason?: unknown) => void = () => undefined
    const perform = vi.fn(() => new Promise<CaptureDraftSaveOutcome>((_resolve, reject) => {
      rejectSave = reject
    }))
    const harness = createHarness(perform)

    harness.queue.enqueue(update('dispose while running'))
    await vi.waitFor(() => expect(perform).toHaveBeenCalledOnce())
    harness.queue.dispose()
    rejectSave(new Error('late failure'))

    await vi.waitFor(() => expect(harness.states.at(-1)?.running).toBe(false))
    expect(harness.onFailed).not.toHaveBeenCalled()
    expect(harness.states.at(-1)?.retryRequired).toBe(false)
  })
})
