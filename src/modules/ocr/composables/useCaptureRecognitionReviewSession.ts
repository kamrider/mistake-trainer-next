import { computed, ref, watch } from 'vue'
import type {
  CaptureRecognitionJob,
  CaptureRecognitionSuggestion,
} from '../../../shared/api/bindings'

export type CaptureRecognitionReviewFilter = 'review' | 'high' | 'low' | 'stale'

function suggestionsFor(
  job: CaptureRecognitionJob,
  filter: CaptureRecognitionReviewFilter,
) {
  if (filter === 'stale') {
    return job.suggestions.filter(item => item.state === 'stale')
  }
  return job.suggestions.filter(item =>
    item.state !== 'stale' && item.reviewBand === filter,
  )
}

function countFor(job: CaptureRecognitionJob, filter: CaptureRecognitionReviewFilter) {
  return suggestionsFor(job, filter).length
}

function initialFilter(job: CaptureRecognitionJob): CaptureRecognitionReviewFilter {
  const priority: CaptureRecognitionReviewFilter[] = ['review', 'high', 'low', 'stale']
  return priority.find(candidate => countFor(job, candidate) > 0) ?? 'review'
}

export function useCaptureRecognitionReviewSession(
  job: () => CaptureRecognitionJob,
) {
  const filter = ref<CaptureRecognitionReviewFilter>('review')
  const currentIndex = ref(0)
  const locallyAccepted = ref(new Set<string>())
  const locallyRejected = ref(new Set<string>())

  const counts = computed(() => ({
    review: countFor(job(), 'review'),
    high: countFor(job(), 'high'),
    low: countFor(job(), 'low'),
    stale: countFor(job(), 'stale'),
  }))
  const filtered = computed(() => suggestionsFor(job(), filter.value))
  const current = computed(() => filtered.value[currentIndex.value])
  const acceptedIds = computed(() => job().suggestions
    .filter(item => item.state === 'accepted' || locallyAccepted.value.has(item.id))
    .filter(item => !locallyRejected.value.has(item.id) && item.state !== 'stale')
    .map(item => item.id))

  watch(job, (nextJob, previousJob) => {
    const sameJob = previousJob?.id === nextJob.id
    const previousCurrentId = sameJob
      ? suggestionsFor(previousJob, filter.value)[currentIndex.value]?.id
      : undefined

    locallyAccepted.value = new Set(
      nextJob.suggestions
        .filter(item => item.state === 'accepted')
        .map(item => item.id),
    )
    locallyRejected.value = new Set(
      nextJob.suggestions
        .filter(item => item.state === 'rejected')
        .map(item => item.id),
    )

    if (!sameJob) {
      filter.value = initialFilter(nextJob)
      currentIndex.value = 0
      return
    }

    const nextSuggestions = suggestionsFor(nextJob, filter.value)
    const preservedIndex = previousCurrentId
      ? nextSuggestions.findIndex(item => item.id === previousCurrentId)
      : -1
    currentIndex.value = preservedIndex >= 0
      ? preservedIndex
      : Math.min(currentIndex.value, Math.max(0, nextSuggestions.length - 1))
  }, { immediate: true })

  watch(filter, () => {
    currentIndex.value = 0
  })

  function decisionState(suggestion: CaptureRecognitionSuggestion) {
    if (locallyAccepted.value.has(suggestion.id)) return 'accepted'
    if (locallyRejected.value.has(suggestion.id)) return 'rejected'
    return suggestion.state
  }

  function recordDecision(
    suggestionId: string,
    decision: 'accepted' | 'rejected',
  ) {
    const accepted = new Set(locallyAccepted.value)
    const rejected = new Set(locallyRejected.value)
    if (decision === 'accepted') {
      accepted.add(suggestionId)
      rejected.delete(suggestionId)
    }
    else {
      rejected.add(suggestionId)
      accepted.delete(suggestionId)
    }
    locallyAccepted.value = accepted
    locallyRejected.value = rejected
  }

  function recordAcceptedMany(suggestionIds: string[]) {
    const accepted = new Set(locallyAccepted.value)
    const rejected = new Set(locallyRejected.value)
    for (const suggestionId of suggestionIds) {
      accepted.add(suggestionId)
      rejected.delete(suggestionId)
    }
    locallyAccepted.value = accepted
    locallyRejected.value = rejected
  }

  function move(offset: number) {
    if (!filtered.value.length) return currentIndex.value
    currentIndex.value = Math.min(
      filtered.value.length - 1,
      Math.max(0, currentIndex.value + offset),
    )
    return currentIndex.value
  }

  function selectFilter(nextFilter: CaptureRecognitionReviewFilter) {
    filter.value = nextFilter
  }

  return {
    filter,
    currentIndex,
    counts,
    filtered,
    current,
    acceptedIds,
    decisionState,
    recordDecision,
    recordAcceptedMany,
    move,
    selectFilter,
  }
}
