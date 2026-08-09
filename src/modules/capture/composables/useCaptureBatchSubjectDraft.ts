import { ref, watch, type Ref } from 'vue'
import type { CaptureBatchState } from '../../../shared/api/bindings'

export interface CaptureBatchSubjectSource {
  id: string
  state: CaptureBatchState
  subject: string
}

export interface CaptureBatchSubjectDraft {
  collectingSubject: Ref<string>
  pendingSubject: Ref<string>
  markCollectingDirty: () => void
  selectPendingSubject: (subject: string) => void
}

export function useCaptureBatchSubjectDraft(
  source: () => CaptureBatchSubjectSource | undefined,
): CaptureBatchSubjectDraft {
  const collectingSubject = ref('')
  const pendingSubject = ref('')
  let activeBatchId = ''
  let activeState: CaptureBatchState | undefined
  let collectingDirty = false
  let pendingDirty = false

  function reset(value?: CaptureBatchSubjectSource) {
    activeBatchId = value?.id ?? ''
    activeState = value?.state
    collectingSubject.value = value?.subject ?? ''
    pendingSubject.value = value?.subject ?? ''
    collectingDirty = false
    pendingDirty = false
  }

  watch(source, (value) => {
    if (
      !value
      || value.id !== activeBatchId
      || value.state !== activeState
    ) {
      reset(value)
      return
    }

    if (!collectingDirty || collectingSubject.value === value.subject) {
      collectingSubject.value = value.subject
      collectingDirty = false
    }
    if (!pendingDirty || pendingSubject.value === value.subject) {
      pendingSubject.value = value.subject
      pendingDirty = false
    }
  }, { immediate: true })

  function markCollectingDirty() {
    collectingDirty = true
  }

  function selectPendingSubject(subject: string) {
    pendingSubject.value = subject
    pendingDirty = true
  }

  return {
    collectingSubject,
    pendingSubject,
    markCollectingDirty,
    selectPendingSubject,
  }
}
