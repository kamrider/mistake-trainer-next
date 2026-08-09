import { ref } from 'vue'
import { describe, expect, it, vi } from 'vitest'
import type { SubjectPreferences } from '../../shared/api/bindings'
import { useSubjectPreferenceDraft } from './useSubjectPreferenceDraft'

function harness(initial: SubjectPreferences = {
  enabledSubjects: ['语文', '数学'],
  customSubjects: [],
  captureSoundEnabled: true,
}) {
  const preferences = ref<SubjectPreferences | undefined>({
    ...initial,
    enabledSubjects: [...initial.enabledSubjects],
    customSubjects: [...initial.customSubjects],
  })
  const onChanged = vi.fn()
  return {
    preferences,
    onChanged,
    controller: useSubjectPreferenceDraft({
      preferences,
      builtinSubjects: ['语文', '数学', '英语'],
      onChanged,
    }),
  }
}

describe('useSubjectPreferenceDraft', () => {
  it('adds a trimmed custom subject, enables it, and dirties once', () => {
    const h = harness()
    const original = h.preferences.value
    h.controller.updateCustomSubject('  竞赛数学  ')

    expect(h.controller.addCustomSubject()).toBe(true)

    expect(h.preferences.value).not.toBe(original)
    expect(h.preferences.value?.customSubjects).toEqual(['竞赛数学'])
    expect(h.preferences.value?.enabledSubjects).toEqual(['语文', '数学', '竞赛数学'])
    expect(h.controller.customSubject.value).toBe('')
    expect(h.controller.message.value).toBe('')
    expect(h.onChanged).toHaveBeenCalledOnce()
  })

  it('rejects blank, overlong, built-in, and case-insensitive custom duplicates', () => {
    const h = harness({
      enabledSubjects: ['语文', 'Programming'],
      customSubjects: ['Programming'],
      captureSoundEnabled: true,
    })
    const original = h.preferences.value

    h.controller.updateCustomSubject('   ')
    expect(h.controller.addCustomSubject()).toBe(false)
    expect(h.controller.message.value).toBe('请输入自定义科目名称。')

    h.controller.updateCustomSubject('超'.repeat(41))
    expect(h.controller.addCustomSubject()).toBe(false)
    expect(h.controller.message.value).toBe('自定义科目名称最多 40 个字符。')

    h.controller.updateCustomSubject(' 数学 ')
    expect(h.controller.addCustomSubject()).toBe(false)
    expect(h.controller.message.value).toBe('“数学”已在科目列表中。')

    h.controller.updateCustomSubject('programming')
    expect(h.controller.addCustomSubject()).toBe(false)
    expect(h.controller.message.value).toBe('“programming”已在科目列表中。')

    expect(h.preferences.value).toBe(original)
    expect(h.onChanged).not.toHaveBeenCalled()
  })

  it('retains the 20-custom-subject limit without dirtying the draft', () => {
    const customSubjects = Array.from({ length: 20 }, (_, index) => `自定义${index + 1}`)
    const h = harness({
      enabledSubjects: ['语文', ...customSubjects],
      customSubjects,
      captureSoundEnabled: true,
    })
    h.controller.updateCustomSubject('第 21 个')

    expect(h.controller.addCustomSubject()).toBe(false)

    expect(h.controller.message.value).toBe('自定义科目最多 20 个。')
    expect(h.preferences.value?.customSubjects).toHaveLength(20)
    expect(h.onChanged).not.toHaveBeenCalled()
  })

  it('never disables or deletes the sole enabled subject', () => {
    const builtin = harness({
      enabledSubjects: ['语文'],
      customSubjects: [],
      captureSoundEnabled: true,
    })
    expect(builtin.controller.toggleSubject('语文', false)).toBe(false)
    expect(builtin.controller.message.value).toBe('至少保留一个常用科目。')
    expect(builtin.preferences.value?.enabledSubjects).toEqual(['语文'])
    expect(builtin.onChanged).not.toHaveBeenCalled()

    const custom = harness({
      enabledSubjects: ['编程'],
      customSubjects: ['编程'],
      captureSoundEnabled: true,
    })
    expect(custom.controller.removeCustomSubject('编程')).toBe(false)
    expect(custom.controller.message.value).toBe(
      '至少保留一个常用科目；请先启用其他科目，再删除“编程”。',
    )
    expect(custom.preferences.value?.customSubjects).toEqual(['编程'])
    expect(custom.preferences.value?.enabledSubjects).toEqual(['编程'])
    expect(custom.onChanged).not.toHaveBeenCalled()
  })

  it('applies toggle, removal, and sound changes immutably and suppresses no-ops', () => {
    const h = harness({
      enabledSubjects: ['语文', '编程'],
      customSubjects: ['编程'],
      captureSoundEnabled: true,
    })
    const first = h.preferences.value

    expect(h.controller.toggleSubject('英语', true)).toBe(true)
    expect(h.preferences.value).not.toBe(first)
    expect(h.controller.toggleSubject('英语', true)).toBe(false)
    expect(h.controller.updateCaptureSound(true)).toBe(false)
    expect(h.controller.updateCaptureSound(false)).toBe(true)
    expect(h.controller.removeCustomSubject('编程')).toBe(true)
    expect(h.controller.removeCustomSubject('编程')).toBe(false)

    expect(h.preferences.value).toEqual({
      enabledSubjects: ['语文', '英语'],
      customSubjects: [],
      captureSoundEnabled: false,
    })
    expect(h.onChanged).toHaveBeenCalledTimes(3)
  })

  it('clears stale local validation when the input changes or save begins', () => {
    const h = harness()
    h.controller.updateCustomSubject('数学')
    h.controller.addCustomSubject()
    expect(h.controller.message.value).not.toBe('')

    h.controller.updateCustomSubject('竞赛数学')
    expect(h.controller.message.value).toBe('')
    h.controller.updateCustomSubject('数学')
    h.controller.addCustomSubject()
    h.controller.clearMessage()
    expect(h.controller.message.value).toBe('')
    expect(h.onChanged).not.toHaveBeenCalled()
  })
})
