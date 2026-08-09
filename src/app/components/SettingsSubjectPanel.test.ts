import { render, screen } from '@testing-library/vue'
import userEvent from '@testing-library/user-event'
import { describe, expect, it } from 'vitest'
import SettingsSubjectPanel from './SettingsSubjectPanel.vue'

const preferences = {
  enabledSubjects: ['语文', '数学', '编程'],
  customSubjects: ['编程'],
  captureSoundEnabled: true,
}

const baseProps = {
  preferences,
  builtinSubjects: ['语文', '数学', '英语'],
  customSubject: '',
  saving: false,
  message: '',
}

describe('SettingsSubjectPanel', () => {
  it('renders checked subjects and emits immutable toggle intentions', async () => {
    const view = render(SettingsSubjectPanel, { props: baseProps })

    expect(screen.getByRole('checkbox', { name: '语文' })).toBeChecked()
    expect(screen.getByRole('checkbox', { name: '英语' })).not.toBeChecked()

    await userEvent.click(screen.getByRole('checkbox', { name: '英语' }))

    expect(view.emitted().toggleSubject).toEqual([['英语', true]])
    expect(preferences.enabledSubjects).toEqual(['语文', '数学', '编程'])
  })

  it('keeps the final enabled subject guarded', () => {
    render(SettingsSubjectPanel, {
      props: {
        ...baseProps,
        preferences: {
          enabledSubjects: ['语文'],
          customSubjects: [],
          captureSoundEnabled: true,
        },
      },
    })

    expect(screen.getByRole('checkbox', { name: '语文' })).toBeDisabled()
    expect(screen.getByText('至少保留一个常用科目；最后一个已选科目会保持启用。')).toBeVisible()
  })

  it('emits custom-subject, sound, remove, and save intentions', async () => {
    const view = render(SettingsSubjectPanel, { props: baseProps })

    const input = screen.getByRole('textbox', { name: '自定义科目名称' })
    await userEvent.type(input, '竞赛数学')
    await view.rerender({ customSubject: '竞赛数学' })
    await userEvent.click(screen.getByRole('button', { name: '添加自定义科目' }))
    await userEvent.click(screen.getByRole('button', { name: '删除自定义科目 编程' }))
    await userEvent.click(screen.getByRole('checkbox', { name: /拖放成功音效/ }))
    await userEvent.click(screen.getByRole('button', { name: '保存科目配置' }))

    expect(view.emitted().updateCustomSubject?.at(-1)).toEqual(['竞赛数学'])
    expect(view.emitted().addCustomSubject).toHaveLength(1)
    expect(view.emitted().removeCustomSubject).toEqual([['编程']])
    expect(view.emitted().updateCaptureSound).toEqual([[false]])
    expect(view.emitted().save).toHaveLength(1)
  })

  it('disables an empty custom-subject submission and associates status feedback', () => {
    render(SettingsSubjectPanel, {
      props: {
        ...baseProps,
        customSubject: '   ',
        message: '“数学”已在科目列表中。',
      },
    })

    expect(screen.getByRole('button', { name: '添加自定义科目' })).toBeDisabled()
    expect(screen.getByRole('textbox', { name: '自定义科目名称' })).toHaveAttribute(
      'aria-describedby',
      'subject-message',
    )
    expect(screen.getByRole('status')).toHaveTextContent('“数学”已在科目列表中。')
  })

  it('announces status copy and disables save while pending', () => {
    render(SettingsSubjectPanel, {
      props: {
        ...baseProps,
        saving: true,
        message: '科目配置正在保存。',
      },
    })

    expect(screen.getByRole('status')).toHaveTextContent('科目配置正在保存。')
    expect(screen.getByRole('button', { name: '保存中…' })).toBeDisabled()
  })
})
