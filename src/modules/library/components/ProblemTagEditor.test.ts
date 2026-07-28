import { render, screen } from '@testing-library/vue'
import userEvent from '@testing-library/user-event'
import { describe, expect, it } from 'vitest'
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
})
