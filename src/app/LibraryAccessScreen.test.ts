import { render, screen } from '@testing-library/vue'
import userEvent from '@testing-library/user-event'
import { describe, expect, it } from 'vitest'
import LibraryAccessScreen from './LibraryAccessScreen.vue'

describe('LibraryAccessScreen', () => {
  it('explains the trusted Windows unlock and emits one explicit action', async () => {
    const user = userEvent.setup()
    const view = render(LibraryAccessScreen, {
      props: { phase: 'locked' },
    })

    expect(screen.getByRole('heading', { name: '本地资料库已锁定' })).toBeVisible()
    expect(screen.getByText(/无需额外密码/)).toBeVisible()
    await user.click(screen.getByRole('button', { name: '使用当前 Windows 账户解锁' }))

    expect(view.emitted('unlock')).toHaveLength(1)
  })

  it('offers both a status retry and a safe unlock recovery after an access error', async () => {
    const user = userEvent.setup()
    const view = render(LibraryAccessScreen, {
      props: { phase: 'error', message: '凭据暂时不可用。' },
    })

    expect(screen.getByRole('alert')).toHaveTextContent('凭据暂时不可用。')
    await user.click(screen.getByRole('button', { name: '重新检查' }))
    await user.click(screen.getByRole('button', { name: '重新解锁' }))

    expect(view.emitted('retry')).toHaveLength(1)
    expect(view.emitted('unlock')).toHaveLength(1)
  })

  it('treats a disconnected configured storage location as a reconnect problem', async () => {
    const user = userEvent.setup()
    const view = render(LibraryAccessScreen, {
      props: {
        phase: 'error',
        reason: 'storage',
        message: '配置的资料库位置当前不可用，未打开或创建任何资料，请重新连接磁盘后重试。',
      },
    })

    expect(screen.getByRole('heading', { name: '请重新连接资料库位置' })).toBeVisible()
    expect(screen.getByRole('alert')).toHaveTextContent('未打开或创建任何资料')
    expect(screen.getByText(/不会在默认位置创建一个空资料库/)).toBeVisible()
    expect(screen.queryByRole('button', { name: '重新解锁' })).not.toBeInTheDocument()
    await user.click(screen.getByRole('button', { name: '已连接，重新检查' }))

    expect(view.emitted('retry')).toHaveLength(1)
    expect(view.emitted('unlock')).toBeUndefined()
  })
})
