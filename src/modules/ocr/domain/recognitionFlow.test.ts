import { describe, expect, it } from 'vitest'
import { recognitionPrimaryAction } from './recognitionFlow'

const action = recognitionPrimaryAction

describe('recognitionPrimaryAction', () => {
  it('stays out of collection and completed batches', () => {
    expect(action({
      batchState: 'collecting',
      unassignedCount: 4,
      featureState: 'ready',
    })).toBe('hidden')
    expect(action({
      batchState: 'completed',
      unassignedCount: 4,
      featureState: 'ready',
    })).toBe('hidden')
  })

  it('does not compete with manual organization when there is no eligible material', () => {
    expect(action({
      batchState: 'organizing',
      unassignedCount: 0,
      featureState: 'ready',
    })).toBe('hidden')
  })

  it('explains the evidence gate without presenting a broken action', () => {
    expect(action({
      batchState: 'organizing',
      unassignedCount: 4,
      featureState: 'evidence_gate_pending',
    })).toBe('explain_gate')
  })

  it('explains an unpublished runtime without offering a dead model download', () => {
    expect(action({
      batchState: 'organizing',
      unassignedCount: 4,
      featureState: 'runtime_missing',
    })).toBe('explain_runtime')
  })

  it('routes model setup separately from starting recognition', () => {
    expect(action({
      batchState: 'organizing',
      unassignedCount: 4,
      featureState: 'model_missing',
    })).toBe('open_setup')
    expect(action({
      batchState: 'organizing',
      unassignedCount: 4,
      featureState: 'ready',
    })).toBe('start')
  })

  it.each(['queued', 'running', 'review'] as const)(
    'resumes an active %s job even when no unassigned material remains',
    (activeJobState) => {
      expect(action({
        batchState: 'organizing',
        unassignedCount: 0,
        featureState: 'ready',
        activeJobState,
      })).toBe('resume')
    },
  )

  it.each(['applied', 'cancelled', 'failed'] as const)(
    'treats a terminal %s job as inactive',
    (activeJobState) => {
      expect(action({
        batchState: 'organizing',
        unassignedCount: 2,
        featureState: 'ready',
        activeJobState,
      })).toBe('start')
    },
  )
})
