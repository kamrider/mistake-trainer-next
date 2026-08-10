import { fireEvent, render, screen, waitFor } from '@testing-library/vue'
import userEvent from '@testing-library/user-event'
import { describe, expect, it } from 'vitest'
import StartupUpdateDialog from './StartupUpdateDialog.vue'

const report = {
  available: true,
  currentVersion: '0.1.0',
  version: '0.2.0',
  publishedAt: '2026-08-10T00:00:00Z',
}

const defaultProps = {
  report,
  installing: false,
  message: '',
  publicationLabel: '2026年8月10日 08:00',
}

describe('StartupUpdateDialog', () => {
  it('presents only user-safe version details and focuses the non-install action', async () => {
    render(StartupUpdateDialog, { props: defaultProps })

    const dialog = screen.getByRole('dialog', { name: '新版本 0.2.0 已准备好' })
    expect(dialog).toHaveAttribute('aria-modal', 'true')
    expect(dialog).toHaveAccessibleDescription(/当前版本 0.1.0/)
    expect(dialog).toHaveTextContent('可选更新 0.2.0')
    expect(dialog).toHaveTextContent('发布时间 2026年8月10日 08:00')
    expect(dialog).not.toHaveTextContent(/github\.com|latest\.json|signature|diagnostic/i)
    await waitFor(() => expect(screen.getByRole('button', { name: /^稍后$/ })).toHaveFocus())
  })

  it('traps keyboard focus and restores the launcher on unmount', async () => {
    const user = userEvent.setup()
    const launcher = document.createElement('button')
    launcher.textContent = 'launcher'
    document.body.append(launcher)
    launcher.focus()
    document.body.style.overflow = 'auto'
    const view = render(StartupUpdateDialog, { props: defaultProps })
    const close = screen.getByRole('button', { name: '稍后更新' })
    const install = screen.getByRole('button', { name: '立即更新至 0.2.0' })

    await waitFor(() => expect(screen.getByRole('button', { name: /^稍后$/ })).toHaveFocus())
    expect(document.body.style.overflow).toBe('hidden')
    close.focus()
    await user.keyboard('{Shift>}{Tab}{/Shift}')
    expect(install).toHaveFocus()
    await user.keyboard('{Tab}')
    expect(close).toHaveFocus()

    view.unmount()
    expect(launcher).toHaveFocus()
    expect(document.body.style.overflow).toBe('auto')
    document.body.style.removeProperty('overflow')
    launcher.remove()
  })

  it('dismisses with safe controls and installs only on explicit confirmation', async () => {
    const user = userEvent.setup()
    const view = render(StartupUpdateDialog, { props: defaultProps })

    await user.keyboard('{Escape}')
    expect(view.emitted('dismiss')).toHaveLength(1)

    const backdrop = screen.getByRole('dialog').parentElement
    expect(backdrop).not.toBeNull()
    await fireEvent.mouseDown(backdrop!)
    expect(view.emitted('dismiss')).toHaveLength(2)

    await user.click(screen.getByRole('button', { name: '立即更新至 0.2.0' }))
    expect(view.emitted('install')).toHaveLength(1)
  })

  it('cannot be dismissed or submitted again while installation is active', async () => {
    const user = userEvent.setup()
    const view = render(StartupUpdateDialog, {
      props: defaultProps,
    })
    await view.rerender({
      ...defaultProps,
      installing: true,
      message: '正在下载并验证更新；安装开始时应用会关闭。',
    })

    expect(screen.getByRole('status')).toHaveTextContent('正在下载并验证更新')
    await waitFor(() => expect(screen.getByRole('dialog')).toHaveFocus())
    expect(screen.getByRole('button', { name: '稍后更新' })).toBeDisabled()
    expect(screen.getByRole('button', { name: /^稍后$/ })).toBeDisabled()
    expect(screen.getByRole('button', { name: '正在下载并验证…' })).toBeDisabled()
    await user.keyboard('{Escape}')
    const backdrop = screen.getByRole('dialog').parentElement
    await fireEvent.mouseDown(backdrop!)

    expect(view.emitted('dismiss')).toBeUndefined()
    expect(view.emitted('install')).toBeUndefined()
  })
})
