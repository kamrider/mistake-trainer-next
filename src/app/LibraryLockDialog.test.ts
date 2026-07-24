import { render, screen, waitFor } from '@testing-library/vue'
import userEvent from '@testing-library/user-event'
import { describe, expect, it } from 'vitest'
import LibraryLockDialog from './LibraryLockDialog.vue'

describe('LibraryLockDialog', () => {
  it('starts on the safe action, closes with Escape, and returns one lock confirmation', async () => {
    const user = userEvent.setup()
    const view = render(LibraryLockDialog, {
      props: { mode: 'lock', busy: false },
    })

    await waitFor(() => expect(screen.getByRole('button', { name: '取消，继续使用' })).toHaveFocus())
    expect(screen.getByText(/不会被删除/)).toBeVisible()
    const confirm = screen.getByRole('button', { name: '立即锁定' })
    const close = screen.getByRole('button', { name: '关闭锁定确认' })
    await user.click(confirm)
    expect(view.emitted('confirm')).toHaveLength(1)

    await user.tab()
    expect(close).toHaveFocus()
    await user.tab({ shift: true })
    expect(confirm).toHaveFocus()

    await user.keyboard('{Escape}')
    expect(view.emitted('cancel')).toHaveLength(1)
  })

  it('makes cloud sign-out, local lock, and offline behavior explicit', () => {
    render(LibraryLockDialog, {
      props: { mode: 'sign-out', busy: false },
    })

    expect(screen.getByRole('heading', { name: '退出云端并锁定本机？' })).toBeVisible()
    expect(screen.getByText(/只会退出这台电脑的云端会话/)).toBeVisible()
    expect(screen.getByText(/其他设备保持登录/)).toBeVisible()
    expect(screen.getByText(/断网也不会阻止本机清除登录凭据/)).toBeVisible()
    expect(screen.getByRole('button', { name: '退出并锁定' })).toBeVisible()
  })

  it('keeps focus on the modal boundary while the restart command is busy', async () => {
    const user = userEvent.setup()
    const view = render(LibraryLockDialog, {
      props: { mode: 'lock', busy: false },
    })
    await view.rerender({ mode: 'lock', busy: true })

    const dialog = screen.getByRole('dialog')
    await waitFor(() => expect(dialog).toHaveFocus())
    await user.tab()

    expect(dialog).toHaveFocus()
    expect(screen.getByRole('button', { name: '正在锁定…' })).toBeDisabled()
  })
})
