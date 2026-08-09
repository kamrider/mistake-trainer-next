import { ref } from 'vue'
import { describe, expect, it } from 'vitest'
import type { ProblemDetail } from '../../../shared/api/bindings'
import { useProblemDetailEditor } from './useProblemDetailEditor'

const detail = (overrides: Partial<ProblemDetail> = {}): ProblemDetail => ({
  id: 'problem-1',
  subject: '数学',
  note: '',
  tags: [],
  status: 'active',
  timeLimitSeconds: null,
  updatedAtUtcMs: 1,
  assets: [],
  ...overrides,
})

describe('useProblemDetailEditor', () => {
  it('preserves a dirty draft on same-problem authoritative refresh', async () => {
    const source = ref<ProblemDetail | undefined>(detail())
    const editor = useProblemDetailEditor(() => source.value)
    editor.startEditing()
    editor.editSubject.value = '数学竞赛'

    source.value = detail({ updatedAtUtcMs: 2 })
    await Promise.resolve()

    expect(editor.editing.value).toBe(true)
    expect(editor.editSubject.value).toBe('数学竞赛')
    expect(editor.dirty.value).toBe(true)
  })

  it('hydrates a clean same-problem editor without closing it', async () => {
    const source = ref<ProblemDetail | undefined>(detail())
    const editor = useProblemDetailEditor(() => source.value)
    editor.startEditing()

    source.value = detail({ note: '服务端新笔记', updatedAtUtcMs: 2 })
    await Promise.resolve()

    expect(editor.editing.value).toBe(true)
    expect(editor.editNote.value).toBe('服务端新笔记')
    expect(editor.dirty.value).toBe(false)
  })

  it('closes after refreshed detail acknowledges the submitted draft', async () => {
    const source = ref<ProblemDetail | undefined>(detail())
    const editor = useProblemDetailEditor(() => source.value)
    editor.startEditing()
    editor.editSubject.value = '数学竞赛'

    expect(editor.prepareSubmission()).toMatchObject({
      problemId: 'problem-1',
      subject: '数学竞赛',
    })
    source.value = detail({ subject: '数学竞赛', updatedAtUtcMs: 2 })
    await Promise.resolve()

    expect(editor.editing.value).toBe(false)
    expect(editor.dirty.value).toBe(false)
  })

  it('preserves newer input when an older submission is acknowledged', async () => {
    const source = ref<ProblemDetail | undefined>(detail())
    const editor = useProblemDetailEditor(() => source.value)
    editor.startEditing()
    editor.editSubject.value = '数学竞赛'
    editor.prepareSubmission()
    editor.editSubject.value = '数学竞赛进阶'

    source.value = detail({ subject: '数学竞赛', updatedAtUtcMs: 2 })
    await Promise.resolve()

    expect(editor.editing.value).toBe(true)
    expect(editor.editSubject.value).toBe('数学竞赛进阶')
    expect(editor.dirty.value).toBe(true)
  })

  it('resets draft state when the problem identity changes', async () => {
    const source = ref<ProblemDetail | undefined>(detail())
    const editor = useProblemDetailEditor(() => source.value)
    editor.startEditing()
    editor.editSubject.value = '数学竞赛'
    editor.prepareSubmission()

    source.value = detail({ id: 'problem-2', subject: '物理' })
    await Promise.resolve()

    expect(editor.editing.value).toBe(false)
    expect(editor.editSubject.value).toBe('物理')
    expect(editor.dirty.value).toBe(false)
  })
})
