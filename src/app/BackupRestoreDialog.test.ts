import { render, screen, waitFor } from '@testing-library/vue'
import userEvent from '@testing-library/user-event'
import { describe, expect, it } from 'vitest'
import BackupRestoreDialog from './BackupRestoreDialog.vue'

const candidate = {
  id: 'restore-candidate-safe',
  summary: {
    formatVersion: 1,
    createdAtUtcMs: 1_700_000_000_000,
    assetCount: 12,
    encryptedBytes: 2048,
    label: '安全备份',
    readyForRestore: true,
  },
  expiresAtUtcMs: null,
}

describe('BackupRestoreDialog', () => {
  it('owns and releases the modal document boundary while returning focus', async () => {
    const previousOverflow = document.body.style.overflow
    const trigger = document.createElement('button')
    trigger.textContent = '打开恢复确认'
    document.body.append(trigger)
    trigger.focus()

    try {
      const view = render(BackupRestoreDialog, {
        props: { candidate, busy: false },
      })

      await waitFor(() => expect(screen.getByRole('button', { name: '取消，保持现状' })).toHaveFocus())
      expect(trigger).toHaveAttribute('inert')
      expect(document.body.style.overflow).toBe('hidden')

      view.unmount()

      expect(trigger).not.toHaveAttribute('inert')
      expect(document.body.style.overflow).toBe(previousOverflow)
      expect(trigger).toHaveFocus()
    }
    finally {
      trigger.remove()
      document.body.style.overflow = previousOverflow
    }
  })

  it('keeps focus on the dialog boundary while restore startup disables every control', async () => {
    const user = userEvent.setup()
    const view = render(BackupRestoreDialog, {
      props: { candidate, busy: false },
    })
    await view.rerender({ candidate, busy: true })

    const dialog = screen.getByRole('dialog')
    await waitFor(() => expect(dialog).toHaveFocus())
    await user.tab()

    expect(dialog).toHaveFocus()
    expect(screen.getByRole('button', { name: '正在准备重启…' })).toBeDisabled()
  })

  it('keeps acknowledgement and safe cancellation behavior unchanged', async () => {
    const user = userEvent.setup()
    const view = render(BackupRestoreDialog, {
      props: { candidate, busy: false },
    })
    const confirm = screen.getByRole('button', { name: '确认恢复并重启' })

    expect(confirm).toBeDisabled()
    await user.click(screen.getByRole('checkbox', { name: /确认后当前题库会由上述备份替换/ }))
    await user.click(confirm)
    expect(view.emitted().confirm).toHaveLength(1)

    await user.keyboard('{Escape}')
    expect(view.emitted().cancel).toHaveLength(1)
  })
})
