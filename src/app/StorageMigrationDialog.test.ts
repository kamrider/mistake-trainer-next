import { fireEvent, render, screen, waitFor } from '@testing-library/vue'
import userEvent from '@testing-library/user-event'
import { describe, expect, it } from 'vitest'
import StorageMigrationDialog from './StorageMigrationDialog.vue'

describe('StorageMigrationDialog', () => {
  it('owns and releases the modal document boundary while recovering escaped focus', async () => {
    const launcher = document.createElement('button')
    document.body.append(launcher)
    const previousOverflow = document.body.style.overflow
    document.body.style.overflow = 'auto'
    const view = render(StorageMigrationDialog, {
      props: { busy: false },
    })

    try {
      const dialog = screen.getByRole('dialog')
      const cancel = screen.getByRole('button', { name: '取消，保持原位置' })
      const close = screen.getByRole('button', { name: '关闭存储迁移确认' })
      await waitFor(() => expect(cancel).toHaveFocus())
      expect(launcher).toHaveAttribute('inert')
      expect(document.body.style.overflow).toBe('hidden')

      launcher.focus()
      await fireEvent.keyDown(dialog, { key: 'Tab' })
      expect(close).toHaveFocus()
      expect(dialog).toContainElement(document.activeElement as HTMLElement)

      view.unmount()
      expect(launcher).not.toHaveAttribute('inert')
      expect(document.body.style.overflow).toBe('auto')
    }
    finally {
      view.unmount()
      document.body.style.overflow = previousOverflow
      launcher.remove()
    }
  })

  it('starts on the safe action, explains the transaction, and confirms once', async () => {
    const user = userEvent.setup()
    const view = render(StorageMigrationDialog, {
      props: { busy: false },
    })

    await waitFor(() => expect(screen.getByRole('button', { name: '取消，保持原位置' })).toHaveFocus())
    expect(screen.getByText(/原资料库保持不变/)).toBeVisible()
    expect(screen.getByText(/校验成功后自动重启/)).toBeVisible()

    await user.click(screen.getByRole('button', { name: '选择文件夹并开始迁移' }))
    expect(view.emitted('confirm')).toHaveLength(1)
  })

  it('closes with Escape only while idle and traps focus inside the dialog', async () => {
    const user = userEvent.setup()
    const view = render(StorageMigrationDialog, {
      props: { busy: false },
    })

    const cancel = screen.getByRole('button', { name: '取消，保持原位置' })
    const confirm = screen.getByRole('button', { name: '选择文件夹并开始迁移' })
    const close = screen.getByRole('button', { name: '关闭存储迁移确认' })
    await waitFor(() => expect(cancel).toHaveFocus())
    await user.tab({ shift: true })
    expect(close).toHaveFocus()
    await user.tab()
    expect(cancel).toHaveFocus()
    await user.tab()
    expect(confirm).toHaveFocus()
    await user.tab()
    expect(close).toHaveFocus()

    await user.keyboard('{Escape}')
    expect(view.emitted('cancel')).toHaveLength(1)

    await view.rerender({ busy: true })
    const dialog = screen.getByRole('dialog')
    await waitFor(() => expect(dialog).toHaveFocus())
    await user.keyboard('{Escape}')
    expect(view.emitted('cancel')).toHaveLength(1)
    expect(screen.getByRole('button', { name: '正在复制并校验…' })).toBeDisabled()
  })

  it('keeps a migration failure inside the decision surface', () => {
    render(StorageMigrationDialog, {
      props: {
        busy: false,
        errorMessage: '目标磁盘空间不足，原资料库保持不变。',
      },
    })

    expect(screen.getByRole('alert')).toHaveTextContent('目标磁盘空间不足，原资料库保持不变。')
    expect(screen.getByRole('button', { name: '选择文件夹并开始迁移' })).toBeEnabled()
  })
})
