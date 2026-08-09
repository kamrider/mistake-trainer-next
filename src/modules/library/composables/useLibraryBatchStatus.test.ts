import { describe, expect, it, vi } from 'vitest'
import type { ProblemStatusFilter } from '../../../shared/api/bindings'
import { failure, success, type AppResult } from '../../../shared/api/app-result'
import { useLibraryBatchStatus } from './useLibraryBatchStatus'

function deferred<T>() {
  let resolve!: (value: T) => void
  let reject!: (reason?: unknown) => void
  const promise = new Promise<T>((finish, fail) => { resolve = finish; reject = fail })
  return { promise, resolve, reject }
}

function harness() {
  let selectedProblemIds = ['problem-1', 'problem-2']
  let changingStatus: ProblemStatusFilter | null = null
  const order: string[] = []
  const operation = vi.fn(async () => { order.push('operation'); return success(2) })
  const onSelectionChange = vi.fn((ids: string[]) => {
    order.push('selection')
    selectedProblemIds = ids
  })
  const onChangingStatusChange = vi.fn((status: ProblemStatusFilter | null) => {
    changingStatus = status
  })
  const onError = vi.fn()
  const scheduleSync = vi.fn(() => { order.push('sync') })
  const refresh = vi.fn(async () => { order.push('refresh') })
  const controller = useLibraryBatchStatus({
    selectedProblemIds: () => selectedProblemIds,
    onSelectionChange,
    changingStatus: () => changingStatus,
    onChangingStatusChange,
    onError,
    scheduleSync,
    refresh,
    operation,
  })

  return {
    controller,
    operation,
    order,
    onSelectionChange,
    onChangingStatusChange,
    onError,
    scheduleSync,
    refresh,
    selection: () => selectedProblemIds,
    setSelection: (ids: string[]) => { selectedProblemIds = ids },
  }
}

describe('useLibraryBatchStatus', () => {
  it('captures the submitted IDs and keeps durable success side effects ordered', async () => {
    const current = harness()

    await current.controller.changeBatchStatus('archived')

    expect(current.operation).toHaveBeenCalledWith({
      problemIds: ['problem-1', 'problem-2'],
      targetStatus: 'archived',
    })
    expect(current.order).toEqual(['operation', 'sync', 'selection', 'refresh'])
    expect(current.selection()).toEqual([])
    expect(current.onChangingStatusChange.mock.calls).toEqual([['archived'], [null]])
    expect(current.onError).toHaveBeenCalledWith('')
  })

  it('allows only one command in flight and preserves selections added during it', async () => {
    const gate = deferred<AppResult<number>>()
    const current = harness()
    current.operation.mockReturnValue(gate.promise)

    const first = current.controller.changeBatchStatus('trashed')
    await vi.waitFor(() => expect(current.operation).toHaveBeenCalledOnce())
    current.setSelection(['problem-1', 'problem-2', 'problem-3'])
    await current.controller.changeBatchStatus('archived')

    expect(current.operation).toHaveBeenCalledOnce()
    expect(current.onChangingStatusChange).toHaveBeenNthCalledWith(1, 'trashed')
    gate.resolve(success(2))
    await first

    expect(current.selection()).toEqual(['problem-3'])
    expect(current.scheduleSync).toHaveBeenCalledOnce()
    expect(current.refresh).toHaveBeenCalledOnce()
  })

  it('keeps the selection and surfaces recoverable command errors', async () => {
    const current = harness()
    current.operation.mockResolvedValue(failure(
      'problem_status_failed',
      '题目状态没有改变。',
      true,
      'diag-batch-status',
    ))

    await current.controller.changeBatchStatus('trashed')

    expect(current.selection()).toEqual(['problem-1', 'problem-2'])
    expect(current.onError).toHaveBeenLastCalledWith('题目状态没有改变。')
    expect(current.scheduleSync).not.toHaveBeenCalled()
    expect(current.refresh).not.toHaveBeenCalled()
    expect(current.onChangingStatusChange).toHaveBeenLastCalledWith(null)
  })

  it('uses stable fallback copy for thrown failures and ignores empty selections', async () => {
    const failed = harness()
    failed.operation.mockRejectedValue(new Error('offline'))
    await failed.controller.changeBatchStatus('active')
    expect(failed.onError).toHaveBeenLastCalledWith('批量操作没有完成，请稍后重试。')
    expect(failed.selection()).toEqual(['problem-1', 'problem-2'])

    const empty = harness()
    empty.setSelection([])
    await empty.controller.changeBatchStatus('trashed')
    expect(empty.operation).not.toHaveBeenCalled()
    expect(empty.onChangingStatusChange).not.toHaveBeenCalled()
  })
})
