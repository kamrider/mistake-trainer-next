import { fireEvent, render, screen, waitFor } from '@testing-library/vue'
import userEvent from '@testing-library/user-event'
import { describe, expect, it } from 'vitest'
import ActionConfirmDialog from './ActionConfirmDialog.vue'

const request = {
  eyebrow: '删除操作 · 最后确认',
  title: '删除这个批次？',
  description: '没有其他引用的图片会被一并清理。',
  confirmLabel: '删除批次',
  cancelLabel: '保留批次',
  tone: 'danger' as const,
}

describe('ActionConfirmDialog', () => {
  it('labels the risk, focuses the safe action, and traps keyboard focus', async () => {
    const user = userEvent.setup()
    render(ActionConfirmDialog, { props: { request } })

    const dialog = screen.getByRole('alertdialog', { name: request.title })
    expect(dialog).toHaveAttribute('aria-modal', 'true')
    expect(dialog).toHaveAccessibleDescription(request.description)

    const cancel = screen.getByRole('button', { name: '保留批次' })
    const confirm = screen.getByRole('button', { name: '删除批次' })
    await waitFor(() => expect(cancel).toHaveFocus())
    expect(confirm).toHaveClass('danger')

    await user.keyboard('{Shift>}{Tab}{/Shift}')
    expect(confirm).toHaveFocus()
    await user.keyboard('{Tab}')
    expect(cancel).toHaveFocus()
  })

  it('cancels with Escape or the backdrop and confirms explicitly', async () => {
    const user = userEvent.setup()
    const view = render(ActionConfirmDialog, { props: { request } })

    await user.keyboard('{Escape}')
    expect(view.emitted('cancel')).toHaveLength(1)

    const backdrop = screen.getByRole('alertdialog').parentElement
    expect(backdrop).not.toBeNull()
    await fireEvent.mouseDown(backdrop!)
    expect(view.emitted('cancel')).toHaveLength(2)

    await user.click(screen.getByRole('button', { name: '删除批次' }))
    expect(view.emitted('confirm')).toHaveLength(1)
  })

  it('restores focus to the launcher when it unmounts', async () => {
    const launcher = document.createElement('button')
    document.body.append(launcher)
    launcher.focus()
    document.body.style.overflow = 'auto'
    const view = render(ActionConfirmDialog, { props: { request } })

    await waitFor(() => expect(screen.getByRole('button', { name: '保留批次' })).toHaveFocus())
    expect(document.body.style.overflow).toBe('hidden')
    view.unmount()

    expect(launcher).toHaveFocus()
    expect(document.body.style.overflow).toBe('auto')
    document.body.style.removeProperty('overflow')
    launcher.remove()
  })
})
