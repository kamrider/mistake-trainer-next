import { render, screen } from '@testing-library/vue'
import userEvent from '@testing-library/user-event'
import { describe, expect, it } from 'vitest'
import AppShell from './AppShell.vue'

describe('AppShell', () => {
  it('announces the selected learner and emits navigation intent', async () => {
    const user = userEvent.setup()
    const view = render(AppShell, {
      props: {
        profiles: [{ id: 'tree', name: '小树', createdAtUtcMs: 1, updatedAtUtcMs: 1, revision: 1 }],
        activeProfileId: 'tree',
        profileBusy: false,
        profileError: '',
        activePage: 'dashboard',
        systemStatus: '资料库已锁定',
      },
      slots: { default: '<p>内容</p>' },
    })

    expect(screen.getByLabelText(/当前学习档案：小树/)).toBeVisible()
    expect(screen.getByText('资料库已锁定')).toBeVisible()
    const navigation = screen.getByRole('navigation', { name: '主导航' })
    expect(navigation).toHaveStyle('--active-index: 0')
    expect(view.container.querySelector('.nav-indicator')).toHaveAttribute('aria-hidden', 'true')
    await user.click(screen.getByRole('button', { name: '题库' }))

    expect(view.emitted('navigate')).toEqual([['library']])
    await view.rerender({ activePage: 'library' })
    expect(navigation).toHaveStyle('--active-index: 2')
  })
})
