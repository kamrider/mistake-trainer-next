import { render, screen, within } from '@testing-library/vue'
import userEvent from '@testing-library/user-event'
import { describe, expect, it } from 'vitest'
import { MISTAKE_REASON_TAGS } from '../domain/mistakeReasons'
import ProblemTagEditor from './ProblemTagEditor.vue'

describe('ProblemTagEditor', () => {
  it('adds a trimmed tag with Enter and removes it with an accessible action', async () => {
    const user = userEvent.setup()
    const view = render(ProblemTagEditor, { props: { modelValue: ['函数'] } })

    await user.type(screen.getByRole('textbox', { name: '标签' }), '  粗心  {Enter}')
    expect(view.emitted('update:modelValue')?.at(-1)).toEqual([['函数', '粗心']])

    await user.click(screen.getByRole('button', { name: '删除标签 函数' }))
    expect(view.emitted('update:modelValue')?.at(-1)).toEqual([[]])
  })

  it('uses Backspace on an empty input to remove the last tag', async () => {
    const user = userEvent.setup()
    const view = render(ProblemTagEditor, { props: { modelValue: ['函数', '粗心'] } })

    await user.click(screen.getByRole('textbox', { name: '标签' }))
    await user.keyboard('{Backspace}')
    expect(view.emitted('update:modelValue')?.at(-1)).toEqual([['函数']])
  })

  it('rejects tags longer than thirty characters without emitting an update', async () => {
    const user = userEvent.setup()
    const view = render(ProblemTagEditor, { props: { modelValue: [] } })

    await user.type(screen.getByRole('textbox', { name: '标签' }), `${'长'.repeat(31)}{Enter}`)
    expect(screen.getByRole('alert')).toHaveTextContent('每个标签最多 30 个字。')
    expect(view.emitted('update:modelValue')).toBeUndefined()
  })

  it('toggles suggested mistake reasons without clearing free tags', async () => {
    const user = userEvent.setup()
    const view = render(ProblemTagEditor, {
      props: {
        modelValue: ['函数', '错因·审题遗漏'],
        suggestions: MISTAKE_REASON_TAGS,
      },
    })

    const suggestions = screen.getByRole('group', { name: '常见错因（可多选）' })
    const concept = within(suggestions).getByRole('button', { name: /概念混淆/ })
    const reading = within(suggestions).getByRole('button', { name: /审题遗漏/ })
    expect(concept).toHaveAttribute('aria-pressed', 'false')
    expect(reading).toHaveAttribute('aria-pressed', 'true')
    expect(screen.getByText('漏看条件、单位、范围或题目要求')).toBeVisible()

    await user.click(concept)
    expect(view.emitted('update:modelValue')?.at(-1)).toEqual([[
      '函数',
      '错因·概念混淆',
      '错因·审题遗漏',
    ]])
  })

  it('disables reason suggestions and refuses a twenty-first tag', async () => {
    const user = userEvent.setup()
    const disabled = render(ProblemTagEditor, {
      props: { modelValue: [], suggestions: MISTAKE_REASON_TAGS, disabled: true },
    })
    expect(screen.getByRole('button', { name: /概念混淆/ })).toBeDisabled()
    disabled.unmount()

    const full = Array.from({ length: 20 }, (_, index) => `标签${index + 1}`)
    const view = render(ProblemTagEditor, {
      props: { modelValue: full, suggestions: MISTAKE_REASON_TAGS },
    })
    await user.click(screen.getByRole('button', { name: /概念混淆/ }))
    expect(screen.getByRole('alert')).toHaveTextContent('每道题最多添加 20 个标签。')
    expect(view.emitted('update:modelValue')).toBeUndefined()
  })
})
