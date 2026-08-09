import { describe, expect, it, vi } from 'vitest'
import { failure, success, type AppResult } from '../../../shared/api/app-result'
import { useLibraryReviewLaunch, type ReviewExperience } from './useLibraryReviewLaunch'

function deferred<T>() {
  let resolve!: (value: T) => void
  let reject!: (reason?: unknown) => void
  const promise = new Promise<T>((finish, fail) => { resolve = finish; reject = fail })
  return { promise, resolve, reject }
}

function harness() {
  let selectedProblemIds = ['problem-2', 'problem-1']
  let startingExperience: ReviewExperience | null = null
  let ownsRoute = true
  const order: string[] = []
  const startManual = vi.fn(async () => { order.push('manual'); return success<unknown>({ sessionId: 'manual-1' }) })
  const startExam = vi.fn(async () => { order.push('exam'); return success<unknown>({ sessionId: 'exam-1' }) })
  const onSelectionChange = vi.fn((ids: string[]) => {
    order.push('selection')
    selectedProblemIds = ids
  })
  const onStartingExperienceChange = vi.fn((experience: ReviewExperience | null) => {
    startingExperience = experience
  })
  const onListError = vi.fn()
  const onDetailError = vi.fn()
  const navigate = vi.fn(async (experience: ReviewExperience) => { order.push(`navigate:${experience}`) })
  const controller = useLibraryReviewLaunch({
    selectedProblemIds: () => selectedProblemIds,
    startingExperience: () => startingExperience,
    ownsRoute: () => ownsRoute,
    onSelectionChange,
    onStartingExperienceChange,
    onListError,
    onDetailError,
    startManual,
    startExam,
    navigate,
  })

  return {
    controller,
    startManual,
    startExam,
    onSelectionChange,
    onStartingExperienceChange,
    onListError,
    onDetailError,
    navigate,
    order,
    selection: () => selectedProblemIds,
    setSelection: (ids: string[]) => { selectedProblemIds = ids },
    setOwnsRoute: (value: boolean) => { ownsRoute = value },
  }
}

describe('useLibraryReviewLaunch', () => {
  it('starts an ordered manual deck, removes only submitted ids, and navigates', async () => {
    const gate = deferred<AppResult<unknown>>()
    const current = harness()
    current.startManual.mockReturnValue(gate.promise)

    const launching = current.controller.startReview()
    await vi.waitFor(() => expect(current.startManual).toHaveBeenCalledWith({
      problemIds: ['problem-2', 'problem-1'],
    }))
    current.setSelection(['problem-2', 'problem-1', 'problem-3'])
    gate.resolve(success({ sessionId: 'manual-1' }))
    await launching

    expect(current.startExam).not.toHaveBeenCalled()
    expect(current.selection()).toEqual(['problem-3'])
    expect(current.order).toEqual(['selection', 'navigate:review'])
    expect(current.onStartingExperienceChange.mock.calls).toEqual([['review'], [null]])
    expect(current.onListError).toHaveBeenCalledWith('')
  })

  it('routes exam detail errors without clearing the list selection', async () => {
    const current = harness()
    current.startExam.mockResolvedValue(failure(
      'review_exam_selection_invalid',
      '所选题目已经变化。',
      false,
      'diag-exam',
    ))

    await current.controller.startReview(['problem-1'], true, 'exam')

    expect(current.startExam).toHaveBeenCalledWith({ problemIds: ['problem-1'] })
    expect(current.onDetailError.mock.calls).toEqual([[''], ['所选题目已经变化。']])
    expect(current.onListError).not.toHaveBeenCalled()
    expect(current.selection()).toEqual(['problem-2', 'problem-1'])
    expect(current.navigate).not.toHaveBeenCalled()
  })

  it('allows only one launch in flight', async () => {
    const gate = deferred<AppResult<unknown>>()
    const current = harness()
    current.startManual.mockReturnValue(gate.promise)

    const first = current.controller.startReview(undefined, false, 'review')
    await vi.waitFor(() => expect(current.startManual).toHaveBeenCalledOnce())
    await current.controller.startReview(undefined, false, 'exam')

    expect(current.startManual).toHaveBeenCalledOnce()
    expect(current.startExam).not.toHaveBeenCalled()
    gate.resolve(success({ sessionId: 'manual-1' }))
    await first
  })

  it('does not navigate, clear selection, or surface errors after route ownership is lost', async () => {
    const successGate = deferred<AppResult<unknown>>()
    const succeeded = harness()
    succeeded.startManual.mockReturnValue(successGate.promise)
    const successLaunch = succeeded.controller.startReview()
    await vi.waitFor(() => expect(succeeded.startManual).toHaveBeenCalledOnce())
    succeeded.setOwnsRoute(false)
    successGate.resolve(success({ sessionId: 'manual-1' }))
    await successLaunch
    expect(succeeded.navigate).not.toHaveBeenCalled()
    expect(succeeded.onSelectionChange).not.toHaveBeenCalled()
    expect(succeeded.onListError.mock.calls).toEqual([['']])

    const failureGate = deferred<AppResult<unknown>>()
    const failed = harness()
    failed.startManual.mockReturnValue(failureGate.promise)
    const failedLaunch = failed.controller.startReview()
    await vi.waitFor(() => expect(failed.startManual).toHaveBeenCalledOnce())
    failed.setOwnsRoute(false)
    failureGate.reject(new Error('offline'))
    await failedLaunch
    expect(failed.onListError.mock.calls).toEqual([['']])
  })

  it('distinguishes command failure from navigation failure after persistence', async () => {
    const command = harness()
    command.startManual.mockRejectedValue(new Error('offline'))
    await command.controller.startReview()
    expect(command.onListError).toHaveBeenLastCalledWith(
      '训练卡组没有创建成功，请保持当前选择并稍后重试。',
    )
    expect(command.selection()).toEqual(['problem-2', 'problem-1'])

    const navigation = harness()
    navigation.navigate.mockRejectedValue(new Error('navigation cancelled'))
    await navigation.controller.startReview(undefined, false, 'exam')
    expect(navigation.onListError).toHaveBeenLastCalledWith(
      '模拟考试已安全保存，可从侧边栏“训练室”继续。',
    )
    expect(navigation.selection()).toEqual([])
  })

  it('ignores empty or out-of-route launch requests', async () => {
    const empty = harness()
    empty.setSelection([])
    await empty.controller.startReview()
    expect(empty.startManual).not.toHaveBeenCalled()

    const stale = harness()
    stale.setOwnsRoute(false)
    await stale.controller.startReview()
    expect(stale.startManual).not.toHaveBeenCalled()
    expect(stale.onStartingExperienceChange).not.toHaveBeenCalled()
  })
})
