import { render, screen } from '@testing-library/vue'
import userEvent from '@testing-library/user-event'
import { describe, expect, it } from 'vitest'
import LibraryFreshStartDialog from './LibraryFreshStartDialog.vue'

describe('LibraryFreshStartDialog', () => {
  it('requires the exact destructive confirmation text', async () => {
    const user = userEvent.setup()
    const view = render(LibraryFreshStartDialog, { props: { busy: false } })
    const confirm = screen.getByRole('button', { name: '确认放弃并重新开始' })
    const input = screen.getByRole('textbox')

    expect(confirm).toBeDisabled()
    await user.type(input, '永久放弃')
    expect(confirm).toBeDisabled()
    await user.clear(input)
    await user.type(input, '永久放弃原资料库')
    expect(confirm).toBeEnabled()
    await user.click(confirm)

    expect(view.emitted('confirm')).toEqual([['永久放弃原资料库']])
  })

  it('cancels with Escape', async () => {
    const user = userEvent.setup()
    const view = render(LibraryFreshStartDialog, { props: { busy: false } })

    await user.keyboard('{Escape}')

    expect(view.emitted('cancel')).toHaveLength(1)
  })

  it('shows a backend rejection inside the active dialog', () => {
    render(LibraryFreshStartDialog, {
      props: {
        busy: false,
        message: '资料库状态已经变化；没有删除任何凭据。',
      },
    })

    expect(screen.getByRole('alert')).toHaveTextContent(
      '资料库状态已经变化；没有删除任何凭据。',
    )
  })
})
