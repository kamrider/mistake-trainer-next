import { describe, expect, it, vi } from 'vitest'
import type { ProblemStatusFilter, ProblemUpdateInput } from '../../../shared/api/bindings'
import { failure, success, type AppResult } from '../../../shared/api/app-result'
import { useLibraryProblemActions } from './useLibraryProblemActions'

const updateInput = {
  problemId: 'problem-1', subject: '数学', note: '更新后的笔记', tags: ['函数'], timeLimitSeconds: 90,
} satisfies ProblemUpdateInput

function deferred<T>() {
  let resolve!: (value: T) => void
  let reject!: (reason?: unknown) => void
  const promise = new Promise<T>((finish, fail) => { resolve = finish; reject = fail })
  return { promise, resolve, reject }
}

function harness() {
  let activeProblemId: string | undefined = 'problem-1'
  let saving = false
  const order: string[] = []
  const operations = {
    update: vi.fn(async () => { order.push('update'); return success(true) }),
    changeStatus: vi.fn(async () => { order.push('status'); return success(1) }),
  }
  const onSavingChange = vi.fn((value: boolean) => { saving = value })
  const onError = vi.fn()
  const onUpdateSuccess = vi.fn()
  const refresh = vi.fn(async () => { order.push('refresh') })
  const reloadDetail = vi.fn(async () => { order.push('reload') })
  const closeDetail = vi.fn(() => { order.push('close'); activeProblemId = undefined })
  const scheduleSync = vi.fn(() => { order.push('sync') })
  const controller = useLibraryProblemActions({
    activeProblemId: () => activeProblemId,
    isSaving: () => saving,
    onSavingChange,
    onError,
    onUpdateSuccess,
    refresh,
    reloadDetail,
    closeDetail,
    scheduleSync,
    operations,
  })
  return {
    controller, operations, order, onSavingChange, onError, onUpdateSuccess, refresh, reloadDetail,
    closeDetail, scheduleSync,
    setActiveProblemId: (value?: string) => { activeProblemId = value },
    setSaving: (value: boolean) => { saving = value },
  }
}

describe('useLibraryProblemActions', () => {
  it('forwards exact inputs and preserves successful action ordering', async () => {
    const update = harness()
    await update.controller.updateProblem(updateInput)
    expect(update.operations.update).toHaveBeenCalledWith(updateInput)
    expect(update.onUpdateSuccess).toHaveBeenCalledWith(updateInput)
    expect(update.order).toEqual(['update', 'sync', 'refresh', 'reload'])
    expect(update.reloadDetail).toHaveBeenCalledWith('problem-1')
    expect(update.onSavingChange.mock.calls).toEqual([[true], [false]])

    const status = harness()
    await status.controller.changeProblemStatus('problem-1', 'archived')
    expect(status.operations.changeStatus).toHaveBeenCalledWith({
      problemIds: ['problem-1'], targetStatus: 'archived',
    })
    expect(status.order).toEqual(['status', 'sync', 'close', 'refresh'])
    expect(status.closeDetail).toHaveBeenCalledWith('problem-1')
  })

  it('forwards recoverable command errors without durable side effects', async () => {
    const update = harness()
    update.operations.update.mockResolvedValue(
      failure('problem_update_failed', '修改被拒绝', true, 'diag-update'),
    )
    await update.controller.updateProblem(updateInput)
    expect(update.onError).toHaveBeenLastCalledWith('修改被拒绝')
    expect(update.scheduleSync).not.toHaveBeenCalled()
    expect(update.onUpdateSuccess).not.toHaveBeenCalled()
    expect(update.refresh).not.toHaveBeenCalled()

    const status = harness()
    status.operations.changeStatus.mockResolvedValue(
      failure('problem_status_failed', '状态被拒绝', true, 'diag-status'),
    )
    await status.controller.changeProblemStatus('problem-1', 'trashed')
    expect(status.onError).toHaveBeenLastCalledWith('状态被拒绝')
    expect(status.closeDetail).not.toHaveBeenCalled()
    expect(status.refresh).not.toHaveBeenCalled()
  })

  it('uses stable fallback copy for active update and status failures', async () => {
    const update = harness()
    update.operations.update.mockRejectedValue(new Error('offline'))
    await update.controller.updateProblem(updateInput)
    expect(update.onError).toHaveBeenLastCalledWith('修改没有保存，请稍后重试。')

    const status = harness()
    status.operations.changeStatus.mockRejectedValue(new Error('offline'))
    await status.controller.changeProblemStatus('problem-1', 'trashed')
    expect(status.onError).toHaveBeenLastCalledWith('题目状态没有改变，请稍后重试。')
  })

  it('keeps durable update side effects but does not reopen a closed detail', async () => {
    const gate = deferred<AppResult<boolean>>()
    const current = harness()
    current.operations.update.mockReturnValue(gate.promise)
    const updating = current.controller.updateProblem(updateInput)
    await vi.waitFor(() => expect(current.operations.update).toHaveBeenCalledOnce())
    current.setActiveProblemId(undefined)
    gate.resolve(success(true))
    await updating
    expect(current.scheduleSync).toHaveBeenCalledOnce()
    expect(current.refresh).toHaveBeenCalledOnce()
    expect(current.reloadDetail).not.toHaveBeenCalled()
    expect(current.onError).toHaveBeenCalledTimes(1)
    expect(current.onError).toHaveBeenLastCalledWith('')
  })

  it('does not close a different detail when a status change finishes late', async () => {
    const gate = deferred<AppResult<number>>()
    const current = harness()
    current.operations.changeStatus.mockReturnValue(gate.promise)
    const changing = current.controller.changeProblemStatus('problem-1', 'archived')
    await vi.waitFor(() => expect(current.operations.changeStatus).toHaveBeenCalledOnce())
    current.setActiveProblemId('problem-2')
    gate.resolve(success(1))
    await changing
    expect(current.scheduleSync).toHaveBeenCalledOnce()
    expect(current.refresh).toHaveBeenCalledOnce()
    expect(current.closeDetail).not.toHaveBeenCalled()
  })

  it('ignores late command errors and thrown failures after navigation', async () => {
    const updateGate = deferred<AppResult<boolean>>()
    const update = harness()
    update.operations.update.mockReturnValue(updateGate.promise)
    const updating = update.controller.updateProblem(updateInput)
    await vi.waitFor(() => expect(update.operations.update).toHaveBeenCalledOnce())
    update.setActiveProblemId(undefined)
    updateGate.resolve(failure('problem_update_failed', '过期更新错误', true, 'diag-late-update'))
    await updating
    expect(update.onError).toHaveBeenCalledTimes(1)
    expect(update.onError).toHaveBeenLastCalledWith('')

    const statusGate = deferred<AppResult<number>>()
    const status = harness()
    status.operations.changeStatus.mockReturnValue(statusGate.promise)
    const changing = status.controller.changeProblemStatus('problem-1', 'trashed')
    await vi.waitFor(() => expect(status.operations.changeStatus).toHaveBeenCalledOnce())
    status.setActiveProblemId('problem-2')
    statusGate.reject(new Error('late offline'))
    await changing
    expect(status.onError).toHaveBeenCalledTimes(1)
    expect(status.onError).toHaveBeenLastCalledWith('')
  })

  it('ignores mismatched or already-saving actions', async () => {
    const mismatch = harness()
    await mismatch.controller.updateProblem({ ...updateInput, problemId: 'problem-2' })
    await mismatch.controller.changeProblemStatus('problem-2', 'archived')
    expect(mismatch.operations.update).not.toHaveBeenCalled()
    expect(mismatch.operations.changeStatus).not.toHaveBeenCalled()

    const saving = harness()
    saving.setSaving(true)
    await saving.controller.updateProblem(updateInput)
    await saving.controller.changeProblemStatus('problem-1', 'archived' as ProblemStatusFilter)
    expect(saving.operations.update).not.toHaveBeenCalled()
    expect(saving.operations.changeStatus).not.toHaveBeenCalled()
  })
})
