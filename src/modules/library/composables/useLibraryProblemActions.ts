import type {
  ProblemStatusFilter,
  ProblemStatusInput,
  ProblemUpdateInput,
} from '../../../shared/api/bindings'
import type { AppResult } from '../../../shared/api/app-result'

interface LibraryProblemOperations {
  update: (input: ProblemUpdateInput) => Promise<AppResult<boolean>>
  changeStatus: (input: ProblemStatusInput) => Promise<AppResult<number>>
}

interface LibraryProblemActionOptions {
  activeProblemId: () => string | undefined
  isSaving: () => boolean
  onSavingChange: (saving: boolean) => void
  onError: (message: string) => void
  onUpdateSuccess: (input: ProblemUpdateInput) => void
  refresh: () => Promise<void>
  reloadDetail: (problemId: string) => Promise<void>
  closeDetail: (problemId: string) => void
  scheduleSync: () => void
  operations: LibraryProblemOperations
}

export function useLibraryProblemActions(options: LibraryProblemActionOptions) {
  const isActive = (problemId: string) => options.activeProblemId() === problemId

  async function updateProblem(input: ProblemUpdateInput) {
    if (!isActive(input.problemId) || options.isSaving()) return
    options.onSavingChange(true)
    options.onError('')
    try {
      const result = await options.operations.update(input)
      if (!result.ok) {
        if (isActive(input.problemId)) options.onError(result.error.userMessage)
        return
      }
      options.onUpdateSuccess(input)
      options.scheduleSync()
      await options.refresh()
      if (isActive(input.problemId)) await options.reloadDetail(input.problemId)
    }
    catch {
      if (isActive(input.problemId)) options.onError('修改没有保存，请稍后重试。')
    }
    finally {
      options.onSavingChange(false)
    }
  }

  async function changeProblemStatus(problemId: string, targetStatus: ProblemStatusFilter) {
    if (!isActive(problemId) || options.isSaving()) return
    options.onSavingChange(true)
    options.onError('')
    try {
      const result = await options.operations.changeStatus({
        problemIds: [problemId],
        targetStatus,
      })
      if (!result.ok) {
        if (isActive(problemId)) options.onError(result.error.userMessage)
        return
      }
      options.scheduleSync()
      if (isActive(problemId)) options.closeDetail(problemId)
      await options.refresh()
    }
    catch {
      if (isActive(problemId)) options.onError('题目状态没有改变，请稍后重试。')
    }
    finally {
      options.onSavingChange(false)
    }
  }

  return { updateProblem, changeProblemStatus }
}
