import { nextTick, ref } from 'vue'
import { describe, expect, it } from 'vitest'
import type { CaptureBatchState } from '../../../shared/api/bindings'
import {
  useCaptureBatchSubjectDraft,
  type CaptureBatchSubjectSource,
} from './useCaptureBatchSubjectDraft'

function sourceBatch(
  id: string,
  state: CaptureBatchState,
  subject: string,
): CaptureBatchSubjectSource {
  return { id, state, subject }
}

describe('useCaptureBatchSubjectDraft', () => {
  it('preserves collecting and organizing choices across same-session refreshes', async () => {
    const source = ref<CaptureBatchSubjectSource | undefined>(
      sourceBatch('batch-1', 'collecting', '数学'),
    )
    const draft = useCaptureBatchSubjectDraft(() => source.value)

    draft.collectingSubject.value = '物理'
    draft.markCollectingDirty()
    source.value = sourceBatch('batch-1', 'collecting', '数学')
    await nextTick()
    expect(draft.collectingSubject.value).toBe('物理')

    source.value = sourceBatch('batch-1', 'organizing', '数学')
    await nextTick()
    draft.selectPendingSubject('化学')
    source.value = sourceBatch('batch-1', 'organizing', '数学')
    await nextTick()
    expect(draft.pendingSubject.value).toBe('化学')
  })

  it('settles a matching pending choice and accepts later authoritative changes', async () => {
    const source = ref<CaptureBatchSubjectSource | undefined>(
      sourceBatch('batch-1', 'organizing', '数学'),
    )
    const draft = useCaptureBatchSubjectDraft(() => source.value)

    draft.selectPendingSubject('化学')
    source.value = sourceBatch('batch-1', 'organizing', '化学')
    await nextTick()
    expect(draft.pendingSubject.value).toBe('化学')

    source.value = sourceBatch('batch-1', 'organizing', '物理')
    await nextTick()
    expect(draft.pendingSubject.value).toBe('物理')
  })

  it('resets both fields when the batch or lifecycle state changes', async () => {
    const source = ref<CaptureBatchSubjectSource | undefined>(
      sourceBatch('batch-1', 'collecting', '数学'),
    )
    const draft = useCaptureBatchSubjectDraft(() => source.value)

    draft.collectingSubject.value = '物理'
    draft.markCollectingDirty()
    source.value = sourceBatch('batch-2', 'collecting', '英语')
    await nextTick()
    expect(draft.collectingSubject.value).toBe('英语')
    expect(draft.pendingSubject.value).toBe('英语')

    draft.collectingSubject.value = '化学'
    draft.markCollectingDirty()
    source.value = sourceBatch('batch-2', 'organizing', '英语')
    await nextTick()
    expect(draft.collectingSubject.value).toBe('英语')
    expect(draft.pendingSubject.value).toBe('英语')
  })

  it('clears both fields when the active batch disappears', async () => {
    const source = ref<CaptureBatchSubjectSource | undefined>(
      sourceBatch('batch-1', 'organizing', '数学'),
    )
    const draft = useCaptureBatchSubjectDraft(() => source.value)

    draft.selectPendingSubject('化学')
    source.value = undefined
    await nextTick()
    expect(draft.collectingSubject.value).toBe('')
    expect(draft.pendingSubject.value).toBe('')
  })
})
