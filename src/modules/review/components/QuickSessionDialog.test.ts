import { render, screen, waitFor } from '@testing-library/vue'
import userEvent from '@testing-library/user-event'
import { describe, expect, it } from 'vitest'
import QuickSessionDialog from './QuickSessionDialog.vue'

describe('QuickSessionDialog', () => {
  it('explains preset limits and emits trimmed optional filters', async () => {
    const user = userEvent.setup()
    const view = render(QuickSessionDialog, { props: { open: true } })

    expect(screen.getByText('最多 8 道，适合课间快速回看')).toBeVisible()
    expect(screen.getByText('最多 10 道，完成一个清晰小目标')).toBeVisible()
    expect(screen.getByText('最多 20 道，重看近 30 天答错的题')).toBeVisible()
    await user.click(screen.getByRole('radio', { name: /十道题专注/ }))
    await user.type(screen.getByRole('textbox', { name: '科目（可选）' }), ' 数学 ')
    await user.type(screen.getByRole('textbox', { name: '标签（可选）' }), ' 函数 ')
    await user.click(screen.getByRole('button', { name: '开始这轮训练' }))

    expect(view.emitted('start')).toEqual([[{
      preset: 'ten_problems', subject: '数学', tag: '函数',
    }]])
  })

  it('keeps actionable empty-state errors visible and locks duplicate starts', () => {
    render(QuickSessionDialog, {
      props: { open: true, busy: true, errorMessage: '当前没有符合条件的题目，可以调整科目或标签后再试。' },
    })
    expect(screen.getByRole('alert')).toHaveTextContent('调整科目或标签')
    expect(screen.getByRole('button', { name: '正在准备训练…' })).toBeDisabled()
    expect(screen.getByRole('button', { name: '取消' })).toBeDisabled()
  })

  it('moves focus inside the modal and closes with Escape', async () => {
    const user = userEvent.setup()
    const view = render(QuickSessionDialog, { props: { open: true } })
    const firstPreset = screen.getByRole('radio', { name: /五分钟热身/ })

    await waitFor(() => expect(firstPreset).toHaveFocus())
    await user.keyboard('{Escape}')

    expect(view.emitted('close')).toEqual([[]])
  })
})
