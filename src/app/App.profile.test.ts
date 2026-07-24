import { render, screen, waitFor } from '@testing-library/vue'
import userEvent from '@testing-library/user-event'
import { createMemoryHistory } from 'vue-router'
import { defineComponent, inject } from 'vue'
import { beforeEach, describe, expect, it, vi } from 'vitest'
import App from './App.vue'
import { libraryAccessControllerKey } from './library-access-controller'
import { createAppRouter } from './router'

const commandMocks = vi.hoisted(() => ({
  libraryAccessStatus: vi.fn(),
  libraryUnlock: vi.fn(),
  systemStatus: vi.fn(),
  profileList: vi.fn(),
  profileCreate: vi.fn(),
  profileDelete: vi.fn(),
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
    commandMocks.libraryAccessStatus.mockResolvedValue({
      ok: true,
      data: { locked: false, trustedWindowsAccount: true },
    })
    commandMocks.libraryUnlock.mockResolvedValue({
      ok: true,
      data: { locked: false, trustedWindowsAccount: true },
    })
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
    commandMocks.profileDelete.mockResolvedValue({
      ok: true,
      data: { activeProfileId: daily.id, profiles: [daily] },
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

  it('keeps every workspace command behind the persistent library lock', async () => {
    commandMocks.libraryAccessStatus.mockResolvedValue({
      ok: true,
      data: { locked: true, trustedWindowsAccount: true },
    })
    const user = userEvent.setup()
    const router = createAppRouter(createMemoryHistory())
    await router.push('/')
    await router.isReady()
    render(App, { global: { plugins: [router], stubs: { transition: false } } })

    expect(await screen.findByRole('heading', { name: '本地资料库已锁定' })).toBeVisible()
    expect(screen.queryByRole('button', { name: '训练台' })).not.toBeInTheDocument()
    expect(commandMocks.systemStatus).not.toHaveBeenCalled()
    expect(commandMocks.profileList).not.toHaveBeenCalled()
    expect(commandMocks.backupRestoreStatus).not.toHaveBeenCalled()

    await user.click(screen.getByRole('button', { name: '使用当前 Windows 账户解锁' }))

    expect(commandMocks.libraryUnlock).toHaveBeenCalledOnce()
    expect(await screen.findByRole('heading', { name: '正在安全重启' })).toBeVisible()
    expect(commandMocks.profileList).not.toHaveBeenCalled()
  })

  it('keeps the unlocked workspace usable when the supplementary status read fails', async () => {
    commandMocks.systemStatus.mockRejectedValueOnce(new Error('status unavailable'))
    const router = createAppRouter(createMemoryHistory())
    await router.push('/')
    await router.isReady()
    render(App, { global: { plugins: [router], stubs: { transition: false } } })

    expect(await screen.findByRole('button', { name: '训练台' })).toBeVisible()
    expect(await screen.findByText('状态检查失败')).toBeVisible()
    await waitFor(() => expect(commandMocks.profileList).toHaveBeenCalledOnce())
    expect(screen.queryByRole('heading', { name: '暂时无法确认资料库状态' })).not.toBeInTheDocument()
  })

  it('fails closed when the configured storage is disconnected and never offers credential unlock', async () => {
    commandMocks.libraryAccessStatus.mockResolvedValue({
      ok: false,
      error: {
        code: 'LIBRARY_STORAGE_UNAVAILABLE',
        userMessage: '配置的资料库位置当前不可用，未打开或创建任何资料，请重新连接磁盘后重试。',
        retryable: true,
        diagnosticId: 'storage-disconnected',
      },
    })
    const router = createAppRouter(createMemoryHistory())
    await router.push('/')
    await router.isReady()
    render(App, { global: { plugins: [router], stubs: { transition: false } } })

    expect(await screen.findByRole('heading', { name: '请重新连接资料库位置' })).toBeVisible()
    expect(screen.getByRole('button', { name: '已连接，重新检查' })).toBeVisible()
    expect(screen.queryByRole('button', { name: '重新解锁' })).not.toBeInTheDocument()
    expect(screen.queryByRole('button', { name: '训练台' })).not.toBeInTheDocument()
    expect(commandMocks.systemStatus).not.toHaveBeenCalled()
    expect(commandMocks.profileList).not.toHaveBeenCalled()
    expect(commandMocks.backupRestoreStatus).not.toHaveBeenCalled()
  })

  it('unmounts the entire application shell as soon as a lock begins restarting', async () => {
    const lockProbe = defineComponent({
      setup() {
        const controller = inject(libraryAccessControllerKey)
        return { restart: () => controller?.enterRestarting() }
      },
      template: '<button type="button" @click="restart">模拟锁定成功</button>',
    })
    const user = userEvent.setup()
    const router = createAppRouter(createMemoryHistory())
    router.addRoute({
      path: '/lock-probe',
      name: 'lock-probe',
      component: lockProbe,
      meta: { shellPage: 'settings' },
    })
    await router.push('/lock-probe')
    await router.isReady()
    render(App, { global: { plugins: [router], stubs: { transition: false } } })

    await user.click(await screen.findByRole('button', { name: '模拟锁定成功' }))

    expect(await screen.findByRole('heading', { name: '正在安全重启' })).toBeVisible()
    expect(screen.queryByRole('button', { name: '训练台' })).not.toBeInTheDocument()
    expect(screen.queryByRole('button', { name: '模拟锁定成功' })).not.toBeInTheDocument()
  })

  it('deletes a confirmed profile and refreshes the active workspace', async () => {
    const user = userEvent.setup()
    const router = createAppRouter(createMemoryHistory())
    await router.push('/library')
    await router.isReady()
    render(App, { global: { plugins: [router], stubs: { transition: false } } })

    await user.click(await screen.findByRole('button', { name: /当前学习档案：日常学习/ }))
    await user.click(screen.getByRole('button', { name: '删除档案：竞赛强化' }))
    await user.type(await screen.findByRole('textbox', { name: '输入“竞赛强化”确认删除' }), '竞赛强化')
    await user.click(screen.getByRole('button', { name: '永久删除档案' }))

    await waitFor(() => {
      expect(commandMocks.profileDelete).toHaveBeenCalledWith({
        profileId: 'contest',
        confirmationName: '竞赛强化',
      })
      expect(router.currentRoute.value.name).toBe('dashboard')
      expect(screen.getByRole('button', { name: /当前学习档案：日常学习/ })).toBeVisible()
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
