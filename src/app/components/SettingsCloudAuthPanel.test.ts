import { render, screen } from '@testing-library/vue'
import userEvent from '@testing-library/user-event'
import { describe, expect, it } from 'vitest'
import type { CloudAuthState } from '../../shared/api/bindings'
import SettingsCloudAuthPanel from './SettingsCloudAuthPanel.vue'

const signedOut: CloudAuthState = {
  configured: true,
  status: { kind: 'signed_out', emailHint: null },
}

const baseProps = {
  auth: signedOut,
  email: '',
  password: '',
  mode: 'signIn' as const,
  authBusy: false,
  authMessage: '',
  syncBusy: false,
  syncMessage: '',
  statusLabel: '未登录',
}

describe('SettingsCloudAuthPanel', () => {
  it('emits credential and authentication intentions without exposing the password', async () => {
    const view = render(SettingsCloudAuthPanel, { props: baseProps })

    const email = screen.getByRole('textbox', { name: '邮箱' })
    const password = screen.getByLabelText('密码')
    expect(password).toHaveAttribute('type', 'password')

    await userEvent.type(email, 'user@example.com')
    await userEvent.type(password, 'secret-123')
    await userEvent.click(screen.getByRole('button', { name: '登录并连接' }))
    await userEvent.click(screen.getByRole('button', { name: '还没有账户？注册' }))

    expect(view.emitted().updateEmail?.at(-1)).toEqual(['user@example.com'])
    expect(view.emitted().updatePassword?.at(-1)).toEqual(['secret-123'])
    expect(view.emitted().submit).toHaveLength(1)
    expect(view.emitted().updateMode).toEqual([['signUp']])
    expect(document.body).not.toHaveTextContent('secret-123')
  })

  it('emits sync and sign-out intentions for a connected account', async () => {
    const view = render(SettingsCloudAuthPanel, {
      props: {
        ...baseProps,
        auth: {
          configured: true,
          status: { kind: 'connected', emailHint: 'u***@example.com' },
        },
        statusLabel: '已连接',
      },
    })

    expect(screen.getByRole('status')).toHaveTextContent('已连接')
    expect(screen.getByRole('status')).toHaveTextContent('u***@example.com')

    await userEvent.click(screen.getByRole('button', { name: '立即同步' }))
    await userEvent.click(screen.getByRole('button', { name: '退出云端并锁定' }))

    expect(view.emitted().sync).toHaveLength(1)
    expect(view.emitted().signOut).toHaveLength(1)
  })

  it('preserves local functionality when cloud configuration is unavailable', () => {
    render(SettingsCloudAuthPanel, {
      props: {
        ...baseProps,
        auth: {
          configured: false,
          status: { kind: 'unconfigured', emailHint: null },
        },
        statusLabel: '未配置云端',
      },
    })

    expect(screen.getByText('国内网络提示')).toBeVisible()
    expect(screen.getByText(/仍可正常使用全部本地功能/)).toBeVisible()
    expect(screen.queryByRole('textbox', { name: '邮箱' })).not.toBeInTheDocument()
  })

  it('announces pending authentication and sync messages', () => {
    render(SettingsCloudAuthPanel, {
      props: {
        ...baseProps,
        auth: {
          configured: true,
          status: { kind: 'connected', emailHint: null },
        },
        authBusy: true,
        syncBusy: true,
        authMessage: '云端账户状态已更新。',
        syncMessage: '同步请求会在网络恢复后重试。',
        statusLabel: '已连接',
      },
    })

    expect(screen.getByRole('button', { name: '同步中…' })).toBeDisabled()
    expect(screen.getByRole('button', { name: '退出云端并锁定' })).toBeDisabled()
    expect(screen.getAllByRole('status').map(node => node.textContent)).toEqual(expect.arrayContaining([
      expect.stringContaining('云端账户状态已更新。'),
      expect.stringContaining('同步请求会在网络恢复后重试。'),
    ]))
  })
})
