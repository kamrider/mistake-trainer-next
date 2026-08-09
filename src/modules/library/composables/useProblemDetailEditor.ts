import { computed, ref, watch } from 'vue'
import type { ProblemDetail, ProblemUpdateInput } from '../../../shared/api/bindings'

interface ProblemEditorDraft {
  subject: string
  note: string
  tags: string[]
  timeLimit: string
}

const emptyDraft = (): ProblemEditorDraft => ({
  subject: '',
  note: '',
  tags: [],
  timeLimit: '',
})

const draftFromDetail = (detail: ProblemDetail): ProblemEditorDraft => ({
  subject: detail.subject,
  note: detail.note,
  tags: [...detail.tags],
  timeLimit: String(detail.timeLimitSeconds ?? ''),
})

const sameDraft = (left: ProblemEditorDraft, right: ProblemEditorDraft) =>
  left.subject === right.subject
  && left.note === right.note
  && left.timeLimit === right.timeLimit
  && left.tags.length === right.tags.length
  && left.tags.every((tag, index) => tag === right.tags[index])

export function useProblemDetailEditor(detailSource: () => ProblemDetail | undefined) {
  const editing = ref(false)
  const editSubject = ref('')
  const editNote = ref('')
  const editTags = ref<string[]>([])
  const editTimeLimit = ref('')
  let activeProblemId: string | undefined
  let baseline = emptyDraft()
  let submitted: ProblemEditorDraft | undefined

  const currentDraft = (): ProblemEditorDraft => ({
    subject: editSubject.value,
    note: editNote.value,
    tags: [...editTags.value],
    timeLimit: editTimeLimit.value,
  })

  const hydrate = (draft: ProblemEditorDraft) => {
    editSubject.value = draft.subject
    editNote.value = draft.note
    editTags.value = [...draft.tags]
    editTimeLimit.value = draft.timeLimit
  }

  const dirty = computed(() =>
    Boolean(activeProblemId && editing.value && !sameDraft(currentDraft(), baseline)))

  watch(detailSource, (detail) => {
    const nextProblemId = detail?.id
    if (nextProblemId !== activeProblemId) {
      activeProblemId = nextProblemId
      baseline = detail ? draftFromDetail(detail) : emptyDraft()
      submitted = undefined
      hydrate(baseline)
      editing.value = false
      return
    }
    if (!detail) return

    const authoritative = draftFromDetail(detail)
    const local = currentDraft()
    const wasDirty = !sameDraft(local, baseline)
    const acknowledgesSubmission = Boolean(submitted && sameDraft(authoritative, submitted))
    const hasNewerInput = Boolean(submitted && !sameDraft(local, submitted))
    baseline = authoritative

    if (acknowledgesSubmission) {
      submitted = undefined
      if (!hasNewerInput) {
        hydrate(authoritative)
        editing.value = false
      }
      return
    }

    if (!editing.value || !wasDirty) hydrate(authoritative)
  }, { immediate: true })

  function startEditing() {
    if (activeProblemId) editing.value = true
  }

  function prepareSubmission(): ProblemUpdateInput | undefined {
    if (!activeProblemId) return undefined
    submitted = currentDraft()
    return {
      problemId: activeProblemId,
      subject: submitted.subject,
      note: submitted.note,
      tags: [...submitted.tags],
      timeLimitSeconds: submitted.timeLimit === '' ? null : Number(submitted.timeLimit),
    }
  }

  return {
    editing,
    editSubject,
    editNote,
    editTags,
    editTimeLimit,
    dirty,
    startEditing,
    prepareSubmission,
  }
}
