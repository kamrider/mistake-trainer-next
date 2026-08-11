import type { ProblemAnswerState, ProblemReviewState } from '../../../shared/api/bindings'

export interface LibraryAdvancedFilters {
  subjects: string[]
  tags: string[]
  reviewState: ProblemReviewState
  answerState: ProblemAnswerState
}

export const EMPTY_LIBRARY_FILTERS: LibraryAdvancedFilters = {
  subjects: [],
  tags: [],
  reviewState: 'any',
  answerState: 'any',
}

export function hasLibraryFilters(filters: LibraryAdvancedFilters) {
  return filters.subjects.length > 0
    || filters.tags.length > 0
    || filters.reviewState !== 'any'
    || filters.answerState !== 'any'
}
