import { render, screen } from '@testing-library/vue'
import userEvent from '@testing-library/user-event'
import { describe, expect, it } from 'vitest'
import ProfileSwitcher from './ProfileSwitcher.vue'

const profiles = [
  { id: 'one', name: '日常学习', createdAtUtcMs: 1, updatedAtUtcMs: 1, revision: 1 },
  { id: 'two', name: '竞赛强化', createdAtUtcMs: 2, updatedAtUtcMs: 2, revision: 1 },
]

describe('ProfileSwitcher', () => {
  it('switches with a single click and exposes an explicit rename action', async () => {
    const user = userEvent.setup()
    const view = render(ProfileSwitcher, {
      props: { profiles, activeProfileId: 'one', busy: false, errorMessage: '' },
    })

    await user.click(screen.getByRole('button', { name: /当前学习档案：日常学习/ }))
    await user.click(screen.getByRole('button', { name: '切换到档案：竞赛强化' }))
    expect(view.emitted('select')).toEqual([['two']])

    await user.click(screen.getByRole('button', { name: '重命名档案：日常学习' }))
    const input = screen.getByRole('textbox', { name: '重命名档案' })
    await user.clear(input)
    await user.type(input, '校内课程')
    await user.click(screen.getByRole('button', { name: '保存' }))
    expect(view.emitted('rename')).toEqual([['one', '校内课程']])
  })

  it('creates a profile and validates blank names without emitting', async () => {
    const user = userEvent.setup()
    const view = render(ProfileSwitcher, {
      props: { profiles, activeProfileId: 'one', busy: false, errorMessage: '' },
    })

    await user.click(screen.getByRole('button', { name: /当前学习档案：日常学习/ }))
    await user.click(screen.getByRole('button', { name: '新建学习档案' }))
    await user.click(screen.getByRole('button', { name: '保存' }))
    expect(screen.getByRole('alert')).toHaveTextContent('请输入档案名称')
    expect(view.emitted('create')).toBeUndefined()

    await user.type(screen.getByRole('textbox', { name: '新档案名称' }), '错题冲刺')
    await user.click(screen.getByRole('button', { name: '保存' }))
    expect(view.emitted('create')).toEqual([['错题冲刺']])
  })

  it('requires the exact profile name before emitting a deletion', async () => {
    const user = userEvent.setup()
    const view = render(ProfileSwitcher, {
      props: { profiles, activeProfileId: 'one', busy: false, errorMessage: '' },
    })

    await user.click(screen.getByRole('button', { name: /当前学习档案：日常学习/ }))
    await user.click(screen.getByRole('button', { name: '删除档案：竞赛强化' }))

    const confirmation = screen.getByRole('textbox', { name: '输入“竞赛强化”确认删除' })
    const remove = screen.getByRole('button', { name: '永久删除档案' })
    expect(remove).toBeDisabled()

    await user.type(confirmation, '竞赛')
    expect(remove).toBeDisabled()
    expect(view.emitted('delete')).toBeUndefined()

    await user.type(confirmation, '强化')
    expect(remove).toBeEnabled()
    await user.click(remove)
    expect(view.emitted('delete')).toEqual([['two', '竞赛强化']])
  })

  it('does not expose deletion when only one profile remains', async () => {
    const user = userEvent.setup()
    render(ProfileSwitcher, {
      props: { profiles: [profiles[0]!], activeProfileId: 'one', busy: false, errorMessage: '' },
    })

    await user.click(screen.getByRole('button', { name: /当前学习档案：日常学习/ }))
    expect(screen.queryByRole('button', { name: /删除档案/ })).not.toBeInTheDocument()
  })

  it('closes on Escape and restores focus to the trigger', async () => {
    const user = userEvent.setup()
    render(ProfileSwitcher, {
      props: { profiles, activeProfileId: 'one', busy: false, errorMessage: '' },
    })
    const trigger = screen.getByRole('button', { name: /当前学习档案：日常学习/ })
    await user.click(trigger)
    await user.keyboard('{Escape}')
    expect(screen.queryByRole('dialog', { name: '切换学习档案' })).not.toBeInTheDocument()
    expect(trigger).toHaveFocus()
  })

  it('offers an explicit retry after a loading failure', async () => {
    const user = userEvent.setup()
    const view = render(ProfileSwitcher, {
      props: {
        profiles: [],
        activeProfileId: '',
        busy: false,
        errorMessage: '学习档案没有读取成功。',
      },
    })
    await user.click(screen.getByRole('button', { name: /当前学习档案/ }))
    await user.click(screen.getByRole('button', { name: '重新读取档案' }))
    expect(view.emitted('retry')).toEqual([[]])
  })
})
