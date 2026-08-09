import { render, screen, waitFor } from '@testing-library/vue'
import userEvent from '@testing-library/user-event'
import { describe, expect, it } from 'vitest'
import LegacyImportDialog from './LegacyImportDialog.vue'

describe('LegacyImportDialog', () => {
  it('requires acknowledgement, focuses cancel first and closes with Escape', async () => {
    const view = render(LegacyImportDialog, {
      props: { mode: 'import', busy: false, memberCount: 2, problemCount: 18 },
    })

    const cancel = screen.getByRole('button', { name: '取消，保持现状' })
    const confirm = screen.getByRole('button', { name: '确认并开始导入' })
    await waitFor(() => expect(cancel).toHaveFocus())
    expect(confirm).toBeDisabled()
    await userEvent.click(screen.getByRole('checkbox', { name: '我确认：导入只复制数据，不会修改旧目录' }))
    expect(confirm).toBeEnabled()

    await userEvent.keyboard('{Escape}')
    expect(view.emitted().cancel).toHaveLength(1)
  })

  it('explains ownership-safe rollback before enabling it', async () => {
    const view = render(LegacyImportDialog, {
      props: { mode: 'rollback', busy: false, memberCount: 1, problemCount: 7 },
    })
    expect(screen.getByText(/已被其他题目复用或后来修改的数据会保留/)).toBeVisible()
    const confirm = screen.getByRole('button', { name: '确认撤销这次导入' })
    expect(confirm).toBeDisabled()
    await userEvent.click(screen.getByRole('checkbox', { name: /撤销本次导入/ }))
    await userEvent.click(confirm)
    expect(view.emitted().confirm).toHaveLength(1)
  })

  it('keeps focus inside the dialog when busy disables every control', () => {
    render(LegacyImportDialog, {
      props: { mode: 'import', busy: true, memberCount: 2, problemCount: 18 },
    })
    const dialog = screen.getByRole('dialog')
    const tab = new KeyboardEvent('keydown', {
      key: 'Tab',
      bubbles: true,
      cancelable: true,
    })

    dialog.dispatchEvent(tab)

    expect(tab.defaultPrevented).toBe(true)
    expect(dialog).toHaveFocus()
  })
})
