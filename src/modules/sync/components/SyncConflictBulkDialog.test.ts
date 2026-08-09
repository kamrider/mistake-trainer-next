import { fireEvent, render, screen, waitFor } from '@testing-library/vue'
import userEvent from '@testing-library/user-event'
import { describe, expect, it } from 'vitest'
import SyncConflictBulkDialog from './SyncConflictBulkDialog.vue'

function renderDialog(
  choice: 'local' | 'remote' = 'local',
  includesRemoteDeletion = false,
) {
  return render(SyncConflictBulkDialog, {
    props: {
      entityLabel: '数学',
      conflictCount: 2,
      choice,
      includesRemoteDeletion,
    },
  })
}

describe('SyncConflictBulkDialog', () => {
  it('defaults focus to cancel and names a local bulk overwrite', async () => {
    renderDialog()

    expect(screen.getByRole('dialog', { name: '确认数学的批量选择' }))
      .toHaveTextContent('2 处冲突全部采用本机版本')
    await waitFor(() => {
      expect(screen.getByRole('button', { name: '取消，逐项确认' })).toHaveFocus()
    })
  })

  it('states that a remote deletion removes the local entity', () => {
    renderDialog('remote', true)

    expect(screen.getByRole('dialog')).toHaveTextContent('本机这条内容将被删除')
    expect(screen.getByRole('button', { name: '确认采用云端并删除本机内容' })).toBeVisible()
  })

  it('emits explicit cancel and confirm decisions', async () => {
    const cancelView = renderDialog()
    const cancelDialog = screen.getByRole('dialog')
    await fireEvent.keyDown(cancelDialog, { key: 'Escape' })
    expect(cancelView.emitted().cancel).toHaveLength(1)
    cancelView.unmount()

    const confirmView = renderDialog('remote')
    await userEvent.click(screen.getByRole('button', { name: '确认全部采用云端版本' }))
    expect(confirmView.emitted().confirm).toHaveLength(1)
  })

  it('keeps keyboard focus inside the dialog', async () => {
    renderDialog()
    const dialog = screen.getByRole('dialog')
    const close = screen.getByRole('button', { name: '关闭批量选择确认' })
    const cancel = screen.getByRole('button', { name: '取消，逐项确认' })
    const confirm = screen.getByRole('button', { name: '确认全部采用本机版本' })

    await waitFor(() => expect(cancel).toHaveFocus())
    confirm.focus()
    await fireEvent.keyDown(dialog, { key: 'Tab' })
    expect(close).toHaveFocus()

    await fireEvent.keyDown(dialog, { key: 'Tab', shiftKey: true })
    expect(confirm).toHaveFocus()
  })
})
