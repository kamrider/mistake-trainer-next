import { render, screen, waitFor } from '@testing-library/vue'
import userEvent from '@testing-library/user-event'
import { createMemoryHistory } from 'vue-router'
import { beforeEach, describe, expect, it, vi } from 'vitest'
import App from './App.vue'
import { createAppRouter } from './router'

const commandMocks = vi.hoisted(() => ({
  systemStatus: vi.fn(),
  profileList: vi.fn(),
  profileCreate: vi.fn(),
  profileRename: vi.fn(),
  profileSelect: vi.fn(),
  backupRestoreStatus: vi.fn(),
}))

vi.mock('@tauri-apps/api/core', () => ({ isTauri: () => true }))
vi.mock('../shared/api/bindings', () => ({ commands: commandMocks }))

const daily = { id: 'daily', name: '日常学习', createdAtUtcMs: 1, updatedAtUtcMs: 1, revision: 1 }
const contest = { id: 'contest', name: '竞赛强化', createdAtUtcMs: 2, updatedAtUtcMs: 2, revision: 1 }

describe('App profile orchestration', () => {
  beforeEach(() => {
    vi.clearAllMocks()
    commandMocks.systemStatus.mockResolvedValue({
      ok: true,
      data: { appVersion: 'test', storage: 'ready', sync: 'offline' },
    })
    commandMocks.profileList.mockResolvedValue({
      ok: true,
      data: { activeProfileId: daily.id, profiles: [daily, contest] },
    })
    commandMocks.backupRestoreStatus.mockResolvedValue({ ok: true, data: null })
    commandMocks.profileSelect.mockResolvedValue({
      ok: true,
      data: { activeProfileId: contest.id, profiles: [daily, contest] },
    })
  })

  it('switches the persisted profile, returns to the dashboard, and updates the shell', async () => {
    const user = userEvent.setup()
    const router = createAppRouter(createMemoryHistory())
    await router.push('/library')
    await router.isReady()
    render(App, { global: { plugins: [router], stubs: { transition: false } } })

    const trigger = await screen.findByRole('button', { name: /当前学习档案：日常学习/ })
    await user.click(trigger)
    await user.click(screen.getByRole('button', { name: '切换到档案：竞赛强化' }))

    await waitFor(() => {
      expect(commandMocks.profileSelect).toHaveBeenCalledWith('contest')
      expect(router.currentRoute.value.name).toBe('dashboard')
      expect(screen.getByRole('button', { name: /当前学习档案：竞赛强化/ })).toBeVisible()
    })
  })

  it('shows and dismisses the one-time result after a successful restore restart', async () => {
    commandMocks.backupRestoreStatus.mockResolvedValue({
      ok: true,
      data: { status: 'succeeded', label: '周日完整备份', finishedAtUtcMs: 10 },
    })
    const router = createAppRouter(createMemoryHistory())
    await router.push('/')
    await router.isReady()
    render(App, { global: { plugins: [router], stubs: { transition: false } } })

    expect(await screen.findByText('资料库恢复成功')).toBeInTheDocument()
    expect(screen.getByText(/周日完整备份/)).toBeInTheDocument()
    await userEvent.click(screen.getByRole('button', { name: '关闭恢复结果通知' }))
    await waitFor(() => expect(screen.queryByText('资料库恢复成功')).not.toBeInTheDocument())
  })
})
