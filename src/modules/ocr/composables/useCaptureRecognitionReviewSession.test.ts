import { nextTick, ref } from 'vue'
import { describe, expect, it } from 'vitest'
import type {
  CaptureRecognitionJob,
  CaptureRecognitionSuggestion,
} from '../../../shared/api/bindings'
import { useCaptureRecognitionReviewSession } from './useCaptureRecognitionReviewSession'

function suggestion(
  id: string,
  reviewBand: CaptureRecognitionSuggestion['reviewBand'],
  state: CaptureRecognitionSuggestion['state'] = 'proposed',
): CaptureRecognitionSuggestion {
  return {
    id,
    itemId: `item-${id}`,
    confidenceBasisPoints: 7600,
    reviewBand,
    state,
    reasonCodes: ['weak_anchor'],
    regions: [{
      rect: { x: 0.1, y: 0.1, width: 0.8, height: 0.35 },
      role: 'question',
      groupSlot: 0,
      confidenceBasisPoints: 7600,
    }],
  }
}

function job(
  id: string,
  suggestions: CaptureRecognitionSuggestion[],
): CaptureRecognitionJob {
  return {
    id,
    batchId: 'batch-1',
    state: 'review',
    totalItems: suggestions.length,
    processedItems: suggestions.length,
    suggestions,
    createdAtUtcMs: 1,
    updatedAtUtcMs: 2,
  }
}

describe('useCaptureRecognitionReviewSession', () => {
  it('opens the first non-empty category including stale-only jobs', () => {
    const currentJob = ref(job('job-1', [suggestion('stale', 'high', 'stale')]))

    const session = useCaptureRecognitionReviewSession(() => currentJob.value)

    expect(session.filter.value).toBe('stale')
    expect(session.current.value?.id).toBe('stale')
  })

  it('preserves the current suggestion by identity across same-job reordering', async () => {
    const first = suggestion('first', 'review')
    const second = suggestion('second', 'review')
    const currentJob = ref(job('job-1', [first, second]))
    const session = useCaptureRecognitionReviewSession(() => currentJob.value)
    session.move(1)
    expect(session.current.value?.id).toBe('second')

    currentJob.value = job('job-1', [{ ...second }, { ...first }])
    await nextTick()

    expect(session.current.value?.id).toBe('second')
    expect(session.currentIndex.value).toBe(0)
  })

  it('resets for a new job and reconciles authoritative decision rollback', async () => {
    const proposed = suggestion('review', 'review')
    const currentJob = ref(job('job-1', [proposed]))
    const session = useCaptureRecognitionReviewSession(() => currentJob.value)
    session.recordDecision('review', 'accepted')
    expect(session.decisionState(proposed)).toBe('accepted')
    expect(session.acceptedIds.value).toEqual(['review'])

    currentJob.value = job('job-1', [{ ...proposed, state: 'proposed' }])
    await nextTick()
    expect(session.decisionState(currentJob.value.suggestions[0]!)).toBe('proposed')
    expect(session.acceptedIds.value).toEqual([])

    currentJob.value = job('job-2', [suggestion('high', 'high')])
    await nextTick()
    expect(session.filter.value).toBe('high')
    expect(session.currentIndex.value).toBe(0)
    expect(session.current.value?.id).toBe('high')
  })
})
