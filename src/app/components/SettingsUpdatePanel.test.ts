import { render, screen } from '@testing-library/vue'
import userEvent from '@testing-library/user-event'
import { describe, expect, it } from 'vitest'
import SettingsUpdatePanel from './SettingsUpdatePanel.vue'

const baseProps = {
  status: {
    enabled: true,
    currentVersion: '0.1.0',
  },
  report: undefined,
  checking: false,
  installing: false,
  message: '',
  publicationLabel: '',
}

describe('SettingsUpdatePanel', () => {
  it('keeps update-disabled builds offline', () => {
    render(SettingsUpdatePanel, {
      props: {
        ...baseProps,
        status: {
          enabled: false,
          currentVersion: '0.1.0',
        },
      },
    })

    expect(screen.getByText('当前安装包未接入自动更新')).toBeVisible()
    expect(screen.queryByRole('button', { name: '检查更新' })).not.toBeInTheDocument()
  })

  it('emits explicit check and exact-version install intentions', async () => {
    const report = {
      available: true,
      currentVersion: '0.1.0',
      version: '0.2.0',
      publishedAt: '2026-07-28T00:00:00Z',
      endpoint: 'https://private.example/latest.json',
      signature: 'must-not-render',
    }
    const view = render(SettingsUpdatePanel, {
      props: {
        ...baseProps,
        report,
        message: '发现已签名版本 0.2.0。',
        publicationLabel: '2026年7月28日 00:00',
      },
    })

    await userEvent.click(screen.getByRole('button', { name: '检查更新' }))
    await userEvent.click(screen.getByRole('button', { name: '安装 0.2.0' }))

    expect(view.emitted().check).toHaveLength(1)
    expect(view.emitted().install).toHaveLength(1)
    expect(screen.getByRole('status', { name: '应用更新状态' })).not.toHaveTextContent(
      /private\.example|must-not-render/,
    )
  })

  it('disables every update action while a check is running', () => {
    render(SettingsUpdatePanel, {
      props: {
        ...baseProps,
        checking: true,
        report: {
          available: true,
          currentVersion: '0.1.0',
          version: '0.2.0',
          publishedAt: null,
        },
      },
    })

    expect(screen.getByRole('button', { name: '正在检查…' })).toBeDisabled()
    expect(screen.getByRole('button', { name: '安装 0.2.0' })).toBeDisabled()
  })
})
