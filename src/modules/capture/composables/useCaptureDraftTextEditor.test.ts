import { nextTick, ref } from 'vue'
import { describe, expect, it } from 'vitest'
import type { CaptureDraftSummary } from '../../../shared/api/bindings'
import { useCaptureDraftTextEditor } from './useCaptureDraftTextEditor'

function draft(
  id: string,
  subject: string,
  tags: string[],
  note: string,
): CaptureDraftSummary {
  return {
    id,
    position: 0,
    subject,
    tags,
    note,
    questionItemIds: ['question-1'],
    answerItemIds: ['answer-1'],
    ready: true,
  }
}

describe('useCaptureDraftTextEditor', () => {
  it('preserves dirty fields across same-draft refreshes and saves with the latest subject', async () => {
    const selectedDraft = ref<CaptureDraftSummary | undefined>(
      draft('draft-1', '数学', ['旧标签'], '旧笔记'),
    )
    const editor = useCaptureDraftTextEditor(selectedDraft)

    editor.tagsText.value = '本地标签，新标签'
    editor.markTagsDirty()
    editor.noteText.value = '仍在输入的笔记'
    editor.markNoteDirty()

    selectedDraft.value = draft('draft-1', '物理', ['旧标签'], '旧笔记')
    await nextTick()

    expect(editor.tagsText.value).toBe('本地标签，新标签')
    expect(editor.noteText.value).toBe('仍在输入的笔记')
    expect(editor.prepareSave()).toMatchObject({
      subject: '物理',
      tags: ['本地标签', '新标签'],
      note: '仍在输入的笔记',
    })
  })

  it('accepts matching authoritative values but never lets an old submission erase a newer edit', async () => {
    const selectedDraft = ref<CaptureDraftSummary | undefined>(
      draft('draft-1', '数学', [], ''),
    )
    const editor = useCaptureDraftTextEditor(selectedDraft)

    editor.tagsText.value = '函数, 易错'
    editor.markTagsDirty()
    editor.noteText.value = '第一版'
    editor.markNoteDirty()
    editor.prepareSave()

    editor.noteText.value = '第二版'
    editor.markNoteDirty()
    selectedDraft.value = draft('draft-1', '数学', ['函数', '易错'], '第一版')
    await nextTick()

    expect(editor.tagsText.value).toBe('函数，易错')
    expect(editor.noteText.value).toBe('第二版')

    editor.prepareSave()
    selectedDraft.value = draft('draft-1', '数学', ['函数', '易错'], '第二版')
    await nextTick()

    expect(editor.noteText.value).toBe('第二版')
    selectedDraft.value = draft('draft-1', '数学', ['服务端标签'], '服务端笔记')
    await nextTick()
    expect(editor.tagsText.value).toBe('服务端标签')
    expect(editor.noteText.value).toBe('服务端笔记')
  })

  it('settles to the backend canonical value when duplicate tags are removed', async () => {
    const selectedDraft = ref<CaptureDraftSummary | undefined>(
      draft('draft-1', '数学', [], ''),
    )
    const editor = useCaptureDraftTextEditor(selectedDraft)

    editor.tagsText.value = '函数, 函数, 易错'
    editor.markTagsDirty()
    expect(editor.prepareSave()?.tags).toEqual(['函数', '函数', '易错'])

    selectedDraft.value = draft('draft-1', '数学', ['函数', '易错'], '')
    await nextTick()
    expect(editor.tagsText.value).toBe('函数，易错')

    selectedDraft.value = draft('draft-1', '数学', ['服务端标签'], '')
    await nextTick()
    expect(editor.tagsText.value).toBe('服务端标签')
  })

  it('loads a different draft and clears the buffer when selection disappears', async () => {
    const selectedDraft = ref<CaptureDraftSummary | undefined>(
      draft('draft-1', '数学', ['第一题'], '第一题笔记'),
    )
    const editor = useCaptureDraftTextEditor(selectedDraft)

    editor.noteText.value = '第一题本地编辑'
    editor.markNoteDirty()
    selectedDraft.value = draft('draft-2', '语文', ['第二题'], '第二题笔记')
    await nextTick()

    expect(editor.tagsText.value).toBe('第二题')
    expect(editor.noteText.value).toBe('第二题笔记')

    selectedDraft.value = undefined
    await nextTick()
    expect(editor.tagsText.value).toBe('')
    expect(editor.noteText.value).toBe('')
    expect(editor.prepareSave()).toBeUndefined()
  })
})
