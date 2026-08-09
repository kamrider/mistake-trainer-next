import type {
  CaptureBatchDetail,
  CaptureBatchSubjectInput,
  CaptureCardMergeInput,
  CaptureItemMoveInput,
  CaptureItemStageRoleInput,
  CaptureLayoutInput,
  CaptureLayoutMode,
} from '../../../shared/api/bindings'
import type { AppResult } from '../../../shared/api/app-result'

export type CaptureOrganizerSaveState = 'idle' | 'saving' | 'saved' | 'error'

export interface CaptureOrganizerMoveTarget {
  itemId: string
  targetDraftId: string | null
  targetRole: 'question' | 'answer' | null
  targetPosition: number
}

interface CaptureOrganizerOperations {
  applyLayout: (input: CaptureLayoutInput) => Promise<AppResult<CaptureBatchDetail>>
  assignSubject: (input: CaptureBatchSubjectInput) => Promise<AppResult<CaptureBatchDetail>>
  moveItem: (input: CaptureItemMoveInput) => Promise<AppResult<CaptureBatchDetail>>
  stageRole: (input: CaptureItemStageRoleInput) => Promise<AppResult<CaptureBatchDetail>>
  mergeCard: (input: CaptureCardMergeInput) => Promise<AppResult<CaptureBatchDetail>>
  deleteDraft: (batchId: string, expectedRevision: number, draftId: string) => Promise<AppResult<CaptureBatchDetail>>
}

interface CaptureOrganizerOptions {
  desktopAvailable: boolean
  activeDetail: () => CaptureBatchDetail | undefined
  isBlocked: () => boolean
  onBusyChange: (busy: boolean) => void
  onSaveStateChange: (state: CaptureOrganizerSaveState) => void
  onDetailChange: (detail: CaptureBatchDetail) => void
  onError: (message: string) => void
  reloadDetail: (batchId: string) => Promise<void>
  operations: CaptureOrganizerOperations
}

interface MutationSpec {
  fallbackMessage: string
  trackSave: boolean
  clearError: boolean
  call: (detail: CaptureBatchDetail) => Promise<AppResult<CaptureBatchDetail>>
}

export function useCaptureOrganizerActions(options: CaptureOrganizerOptions) {
  async function run(spec: MutationSpec) {
    const current = options.activeDetail()
    if (!options.desktopAvailable || !current || options.isBlocked()) return
    const batchId = current.batch.id
    const expectedRevision = current.batch.revision
    const stillCurrent = () => {
      const active = options.activeDetail()?.batch
      return active?.id === batchId && active.revision === expectedRevision
    }
    options.onBusyChange(true)
    if (spec.trackSave) options.onSaveStateChange('saving')
    if (spec.clearError) options.onError('')
    try {
      const result = await spec.call(current)
      if (!stillCurrent()) {
        if (spec.trackSave) options.onSaveStateChange('idle')
        return
      }
      if (result.ok) {
        options.onDetailChange(result.data)
        if (spec.trackSave) options.onSaveStateChange('saved')
      }
      else {
        if (spec.trackSave) options.onSaveStateChange('error')
        options.onError(result.error.userMessage)
        if (result.error.code === 'capture_revision_conflict') {
          await options.reloadDetail(batchId)
        }
      }
    }
    catch {
      if (!stillCurrent()) {
        if (spec.trackSave) options.onSaveStateChange('idle')
        return
      }
      if (spec.trackSave) options.onSaveStateChange('error')
      options.onError(spec.fallbackMessage)
    }
    finally {
      options.onBusyChange(false)
    }
  }

  return {
    applyLayout: (mode: CaptureLayoutMode, questions: number, answers: number, splitIndex: number | null) => run({
      fallbackMessage: '整理模板没有应用，原有分组仍会保留。', trackSave: false, clearError: false,
      call: current => options.operations.applyLayout({ batchId: current.batch.id, expectedRevision: current.batch.revision, mode, questionImagesPerDraft: questions, answerImagesPerDraft: answers, splitIndex }),
    }),
    assignBatchSubject: (subject: string) => subject.trim() ? run({
      fallbackMessage: '整批科目没有保存成功，原有题卡保持不变。', trackSave: true, clearError: true,
      call: current => options.operations.assignSubject({ batchId: current.batch.id, expectedRevision: current.batch.revision, subject }),
    }) : Promise.resolve(),
    moveItem: (target: CaptureOrganizerMoveTarget) => run({
      fallbackMessage: '图片没有移动成功，请刷新批次后重试。', trackSave: true, clearError: false,
      call: current => options.operations.moveItem({ batchId: current.batch.id, expectedRevision: current.batch.revision, ...target }),
    }),
    stageItemRole: (itemId: string, stagedRole: 'question' | 'answer') => run({
      fallbackMessage: '图片角色没有保存成功，请重试。', trackSave: true, clearError: true,
      call: current => options.operations.stageRole({ batchId: current.batch.id, expectedRevision: current.batch.revision, itemId, stagedRole }),
    }),
    mergeCard: (itemIds: string[], targetDraftId: string | null, newDraftSubject: string | null) => itemIds.length ? run({
      fallbackMessage: '题卡没有保存成功，图片仍保留在原位置。', trackSave: true, clearError: true,
      call: current => options.operations.mergeCard({ batchId: current.batch.id, expectedRevision: current.batch.revision, targetDraftId, itemIds, newDraftSubject }),
    }) : Promise.resolve(),
    deleteDraft: (draftId: string) => run({
      fallbackMessage: '题卡没有撤销成功，原有图片和分组仍会保留。', trackSave: true, clearError: true,
      call: current => options.operations.deleteDraft(current.batch.id, current.batch.revision, draftId),
    }),
  }
}
