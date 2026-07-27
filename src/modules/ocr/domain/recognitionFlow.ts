import type {
  CaptureBatchState,
  OcrRecognitionFeatureState,
} from '../../../shared/api/bindings'

export type RecognitionPrimaryAction =
  | 'hidden'
  | 'explain_gate'
  | 'explain_runtime'
  | 'open_setup'
  | 'start'
  | 'resume'

export type RecognitionFlowJobState =
  | 'queued'
  | 'running'
  | 'review'
  | 'applied'
  | 'cancelled'
  | 'failed'

export function recognitionPrimaryAction(input: {
  batchState: CaptureBatchState
  unassignedCount: number
  featureState: OcrRecognitionFeatureState
  activeJobState?: RecognitionFlowJobState
}): RecognitionPrimaryAction {
  if (input.batchState !== 'organizing') return 'hidden'
  if (input.activeJobState === 'queued'
    || input.activeJobState === 'running'
    || input.activeJobState === 'review') {
    return 'resume'
  }
  if (input.unassignedCount <= 0) return 'hidden'
  if (input.featureState === 'evidence_gate_pending') return 'explain_gate'
  if (input.featureState === 'runtime_missing') return 'explain_runtime'
  if (input.featureState === 'model_missing') return 'open_setup'
  return 'start'
}
