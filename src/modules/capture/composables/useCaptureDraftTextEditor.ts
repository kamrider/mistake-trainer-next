import { ref, watch, type Ref } from 'vue'
import type { CaptureDraftSummary } from '../../../shared/api/bindings'

export interface CaptureDraftTextUpdate {
  draft: CaptureDraftSummary
  subject: string
  tags: string[]
  note: string
}

export interface CaptureDraftTextEditor {
  tagsText: Ref<string>
  noteText: Ref<string>
  markTagsDirty: () => void
  markNoteDirty: () => void
  prepareSave: () => CaptureDraftTextUpdate | undefined
}

interface SubmittedField<T> {
  editVersion: number
  value: T
}

function parseTags(value: string) {
  return value
    .split(/[，,]/)
    .map(tag => tag.trim())
    .filter(Boolean)
}

function formatTags(tags: string[]) {
  return tags.join('，')
}

function canonicalTags(tags: string[]) {
  const seen = new Set<string>()
  return tags.filter(tag => {
    if (seen.has(tag)) return false
    seen.add(tag)
    return true
  })
}

function sameStrings(left: string[], right: string[]) {
  return left.length === right.length && left.every((value, index) => value === right[index])
}

export function useCaptureDraftTextEditor(
  selectedDraft: Readonly<Ref<CaptureDraftSummary | undefined>>,
): CaptureDraftTextEditor {
  const tagsText = ref('')
  const noteText = ref('')
  let activeDraftId = ''
  let tagsDirty = false
  let noteDirty = false
  let tagsEditVersion = 0
  let noteEditVersion = 0
  let submittedTags: SubmittedField<string[]> | undefined
  let submittedNote: SubmittedField<string> | undefined

  function reset(draft?: CaptureDraftSummary) {
    activeDraftId = draft?.id ?? ''
    tagsText.value = formatTags(draft?.tags ?? [])
    noteText.value = draft?.note ?? ''
    tagsDirty = false
    noteDirty = false
    tagsEditVersion = 0
    noteEditVersion = 0
    submittedTags = undefined
    submittedNote = undefined
  }

  watch(selectedDraft, (draft) => {
    if (!draft || draft.id !== activeDraftId) {
      reset(draft)
      return
    }

    if (!tagsDirty) {
      tagsText.value = formatTags(draft.tags)
    }
    else if (
      submittedTags?.editVersion === tagsEditVersion
      && sameStrings(draft.tags, submittedTags.value)
    ) {
      tagsText.value = formatTags(draft.tags)
      tagsDirty = false
      submittedTags = undefined
    }

    if (!noteDirty) {
      noteText.value = draft.note
    }
    else if (
      submittedNote?.editVersion === noteEditVersion
      && draft.note === submittedNote.value
    ) {
      noteText.value = draft.note
      noteDirty = false
      submittedNote = undefined
    }
  }, { immediate: true })

  function markTagsDirty() {
    tagsDirty = true
    tagsEditVersion += 1
    submittedTags = undefined
  }

  function markNoteDirty() {
    noteDirty = true
    noteEditVersion += 1
    submittedNote = undefined
  }

  function prepareSave(): CaptureDraftTextUpdate | undefined {
    const draft = selectedDraft.value
    if (!draft) return undefined
    const tags = parseTags(tagsText.value)
    const note = noteText.value.trim()
    if (tagsDirty) {
      submittedTags = {
        editVersion: tagsEditVersion,
        value: canonicalTags(tags),
      }
    }
    if (noteDirty) {
      submittedNote = {
        editVersion: noteEditVersion,
        value: note,
      }
    }
    return {
      draft,
      subject: draft.subject.trim(),
      tags,
      note,
    }
  }

  return {
    tagsText,
    noteText,
    markTagsDirty,
    markNoteDirty,
    prepareSave,
  }
}
