import type { ProblemStatusFilter, ProblemStatusInput } from '../../../shared/api/bindings'
import type { AppResult } from '../../../shared/api/app-result'

interface LibraryBatchStatusOptions {
  selectedProblemIds: () => string[]
  onSelectionChange: (problemIds: string[]) => void
  changingStatus: () => ProblemStatusFilter | null
  onChangingStatusChange: (status: ProblemStatusFilter | null) => void
  onError: (message: string) => void
  scheduleSync: () => void
  refresh: () => Promise<void>
  operation: (input: ProblemStatusInput) => Promise<AppResult<number>>
}

export function useLibraryBatchStatus(options: LibraryBatchStatusOptions) {
  async function changeBatchStatus(targetStatus: ProblemStatusFilter) {
    const requestedIds = [...options.selectedProblemIds()]
    if (requestedIds.length === 0 || options.changingStatus()) return

    options.onChangingStatusChange(targetStatus)
    options.onError('')
    try {
      const result = await options.operation({
        problemIds: requestedIds,
        targetStatus,
      })
      if (!result.ok) {
        options.onError(result.error.userMessage)
        return
      }

      options.scheduleSync()
      const processedIds = new Set(requestedIds)
      options.onSelectionChange(options.selectedProblemIds().filter(id => !processedIds.has(id)))
      await options.refresh()
    }
    catch {
      options.onError('批量操作没有完成，请稍后重试。')
    }
    finally {
      options.onChangingStatusChange(null)
    }
  }

  return { changeBatchStatus }
}
