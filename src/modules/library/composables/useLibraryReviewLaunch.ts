import type { ReviewExamStartInput, ReviewManualStartInput } from '../../../shared/api/bindings'
import type { AppResult } from '../../../shared/api/app-result'

export type ReviewExperience = 'review' | 'exam'

interface LibraryReviewLaunchOptions {
  selectedProblemIds: () => string[]
  startingExperience: () => ReviewExperience | null
  ownsRoute: () => boolean
  onSelectionChange: (problemIds: string[]) => void
  onStartingExperienceChange: (experience: ReviewExperience | null) => void
  onListError: (message: string) => void
  onDetailError: (message: string) => void
  startManual: (input: ReviewManualStartInput) => Promise<AppResult<unknown>>
  startExam: (input: ReviewExamStartInput) => Promise<AppResult<unknown>>
  navigate: (experience: ReviewExperience) => Promise<void>
}

export function useLibraryReviewLaunch(options: LibraryReviewLaunchOptions) {
  const setError = (fromDetail: boolean, message: string) => {
    if (fromDetail) options.onDetailError(message)
    else options.onListError(message)
  }

  async function startReview(
    problemIds = options.selectedProblemIds(),
    fromDetail = false,
    experience: ReviewExperience = 'review',
  ) {
    const requestedIds = [...problemIds]
    if (requestedIds.length === 0 || options.startingExperience() || !options.ownsRoute()) return

    options.onStartingExperienceChange(experience)
    setError(fromDetail, '')
    let persisted = false
    let startingExperienceReleased = false
    try {
      const input = { problemIds: requestedIds }
      const result = experience === 'exam'
        ? await options.startExam(input)
        : await options.startManual(input)
      if (!result.ok) {
        if (options.ownsRoute()) setError(fromDetail, result.error.userMessage)
        return
      }

      persisted = true
      if (!options.ownsRoute()) return
      if (!fromDetail) {
        const submittedIds = new Set(requestedIds)
        options.onSelectionChange(options.selectedProblemIds().filter(id => !submittedIds.has(id)))
      }
      options.onStartingExperienceChange(null)
      startingExperienceReleased = true
      await options.navigate(experience)
    }
    catch {
      if (!options.ownsRoute()) return
      const label = experience === 'exam' ? '模拟考试' : '训练卡组'
      setError(fromDetail, persisted
        ? `${label}已安全保存，可从侧边栏“训练室”继续。`
        : `${label}没有创建成功，请保持当前选择并稍后重试。`)
    }
    finally {
      if (!startingExperienceReleased) options.onStartingExperienceChange(null)
    }
  }

  return { startReview }
}
