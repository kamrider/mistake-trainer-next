import { render, screen, waitFor } from '@testing-library/vue'
import userEvent from '@testing-library/user-event'
import { createMemoryHistory, type Router } from 'vue-router'
import { inject, onBeforeUnmount } from 'vue'
import { beforeEach, describe, expect, it, vi } from 'vitest'
import App from './App.vue'
import { libraryAccessControllerKey } from './library-access-controller'
import { createAppRouter } from './router'
import {
  workspaceTransitionGuardKey,
  type WorkspaceTransitionAttempt,
} from './workspace-transition-guard'

const commandMocks = vi.hoisted(() => ({
  libraryAccessStatus: vi.fn(),
  libraryUnlock: vi.fn(),
  systemStatus: vi.fn(),
  profileList: vi.fn(),
  profileCreate: vi.fn(),
  profileDelete: vi.fn(),
  profileRename: vi.fn(),
  profileSelect: vi.fn(),
  compatibilityStatus: vi.fn(),
  backupRestoreStatus: vi.fn(),
  authRestore: vi.fn(),
  syncNow: vi.fn(),
}))

vi.mock('@tauri-apps/api/core', () => ({ isTauri: () => true }))
vi.mock('../shared/api/bindings', () => ({ commands: commandMocks }))

const daily = { id: 'daily', name: '日常学习', createdAtUtcMs: 1, updatedAtUtcMs: 1, revision: 1 }
const contest = { id: 'contest', name: '竞赛强化', createdAtUtcMs: 2, updatedAtUtcMs: 2, revision: 1 }

function deferred<T>() {
  let resolve!: (value: T) => void
  const promise = new Promise<T>((finish) => { resolve = finish })
  return { promise, resolve }
}

function addWorkspaceGuardProbe(router: Router, attempt: WorkspaceTransitionAttempt) {
  const guardProbe = {
    setup() {
      const guard = inject(workspaceTransitionGuardKey, undefined)
      const unregister = guard?.register(attempt)
      onBeforeUnmount(() => unregister?.())
    },
    template: '<p>受保护工作区</p>',
  }
  router.addRoute({
    path: '/workspace-guard-probe',
    name: 'workspace-guard-probe',
    component: guardProbe,
    meta: { shellPage: 'library' },
  })
}

const profileCommandProbe = {
  emits: ['profile-create', 'profile-rename', 'profile-delete'],
  template: `
    <div>
      <button type="button" @click="$emit('profile-create', '错题冲刺')">模拟新建档案</button>
      <button type="button" @click="$emit('profile-rename', 'contest', '竞赛提高')">模拟重命名非当前档案</button>
      <button type="button" @click="$emit('profile-delete', 'daily', '日常学习')">模拟删除当前档案</button>
      <button type="button" @click="$emit('profile-delete', 'contest', '竞赛强化')">模拟删除非当前档案</button>
      <slot />
    </div>
  `,
}

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
    commandMocks.compatibilityStatus.mockResolvedValue({
      status: 'ok',
      data: {
        ok: true,
        data: {
          supportLevel: 'supported',
          supported: true,
          osName: 'Windows 11 Pro',
          displayVersion: '24H2',
          buildNumber: 26100,
          updateBuildRevision: 1000,
          processArchitecture: 'x86_64',
          nativeArchitecture: 'x86_64',
          webview2Version: '138.0.3351.83',
          minimumWindowsBuild: 17763,
          summary: '当前设备处于完整支持范围。',
        },
      },
    })
    commandMocks.backupRestoreStatus.mockResolvedValue({ ok: true, data: null })
    commandMocks.authRestore.mockResolvedValue({
      status: 'ok',
      data: {
        ok: true,
        data: {
          configured: false,
          status: { kind: 'unconfigured', emailHint: null },
        },
      },
    })
    commandMocks.syncNow.mockResolvedValue({
      status: 'ok',
      data: {
        ok: true,
        data: {
          pushedOperationCount: 0,
          uploadedAssetCount: 0,
          pulledChangeCount: 0,
          downloadedAssetCount: 0,
          finalCursor: 0,
        },
      },
    })
    commandMocks.profileSelect.mockResolvedValue({
      ok: true,
      data: { activeProfileId: contest.id, profiles: [daily, contest] },
    })
    commandMocks.profileCreate.mockResolvedValue({
      ok: true,
      data: { activeProfileId: contest.id, profiles: [daily, contest] },
    })
    commandMocks.profileRename.mockResolvedValue({
      ok: true,
      data: {
        activeProfileId: daily.id,
        profiles: [daily, { ...contest, name: '竞赛提高', revision: 2 }],
      },
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

  it('keeps the current profile and route when the active workspace cancels a switch', async () => {
    const user = userEvent.setup()
    const attempt = vi.fn()
      .mockResolvedValueOnce(false)
      .mockResolvedValueOnce(true)
    const router = createAppRouter(createMemoryHistory())
    addWorkspaceGuardProbe(router, attempt)
    await router.push('/workspace-guard-probe')
    await router.isReady()
    render(App, { global: { plugins: [router], stubs: { transition: false } } })

    await user.click(await screen.findByRole('button', { name: /当前学习档案：日常学习/ }))
    const selectContest = screen.getByRole('button', { name: '切换到档案：竞赛强化' })
    await user.click(selectContest)

    await waitFor(() => expect(attempt).toHaveBeenCalledOnce())
    expect(commandMocks.profileSelect).not.toHaveBeenCalled()
    expect(router.currentRoute.value.name).toBe('workspace-guard-probe')
    expect(screen.getByRole('button', { name: /当前学习档案：日常学习/ })).toBeVisible()

    await user.click(selectContest)
    await waitFor(() => {
      expect(commandMocks.profileSelect).toHaveBeenCalledOnce()
      expect(router.currentRoute.value.name).toBe('dashboard')
      expect(screen.getByRole('button', { name: /当前学习档案：竞赛强化/ })).toBeVisible()
    })
  })

  it('coalesces repeated profile switches behind one workspace decision and one mutation', async () => {
    const user = userEvent.setup()
    let finishAttempt!: (allowed: boolean) => void
    const attempt = vi.fn(() => new Promise<boolean>((resolve) => {
      finishAttempt = resolve
    }))
    const router = createAppRouter(createMemoryHistory())
    addWorkspaceGuardProbe(router, attempt)
    await router.push('/workspace-guard-probe')
    await router.isReady()
    render(App, { global: { plugins: [router], stubs: { transition: false } } })

    await user.click(await screen.findByRole('button', { name: /当前学习档案：日常学习/ }))
    const selectContest = screen.getByRole('button', { name: '切换到档案：竞赛强化' })
    await user.click(selectContest)
    await user.click(selectContest)

    expect(attempt).toHaveBeenCalledOnce()
    expect(commandMocks.profileSelect).not.toHaveBeenCalled()
    finishAttempt(true)

    await waitFor(() => {
      expect(commandMocks.profileSelect).toHaveBeenCalledOnce()
      expect(router.currentRoute.value.name).toBe('dashboard')
    })
  })

  it('guards profile creation and deletion of the current profile', async () => {
    const user = userEvent.setup()
    const attempt = vi.fn().mockResolvedValue(false)
    const router = createAppRouter(createMemoryHistory())
    addWorkspaceGuardProbe(router, attempt)
    await router.push('/workspace-guard-probe')
    await router.isReady()
    render(App, {
      global: {
        plugins: [router],
        stubs: { AppShell: profileCommandProbe, transition: false },
      },
    })

    await screen.findByText('受保护工作区')
    await waitFor(() => expect(commandMocks.profileList).toHaveBeenCalledOnce())
    await user.click(screen.getByRole('button', { name: '模拟新建档案' }))

    await waitFor(() => expect(attempt).toHaveBeenCalledOnce())
    expect(commandMocks.profileCreate).not.toHaveBeenCalled()
    await user.click(screen.getByRole('button', { name: '模拟删除当前档案' }))

    await waitFor(() => expect(attempt).toHaveBeenCalledTimes(2))
    expect(commandMocks.profileDelete).not.toHaveBeenCalled()
    expect(router.currentRoute.value.name).toBe('workspace-guard-probe')
  })

  it('does not guard rename or deletion of a noncurrent profile', async () => {
    const user = userEvent.setup()
    const attempt = vi.fn().mockResolvedValue(false)
    const router = createAppRouter(createMemoryHistory())
    addWorkspaceGuardProbe(router, attempt)
    await router.push('/workspace-guard-probe')
    await router.isReady()
    render(App, {
      global: {
        plugins: [router],
        stubs: { AppShell: profileCommandProbe, transition: false },
      },
    })

    await screen.findByText('受保护工作区')
    await waitFor(() => expect(commandMocks.profileList).toHaveBeenCalledOnce())
    await user.click(screen.getByRole('button', { name: '模拟重命名非当前档案' }))

    await waitFor(() => expect(commandMocks.profileRename).toHaveBeenCalledOnce())
    await user.click(screen.getByRole('button', { name: '模拟删除非当前档案' }))

    await waitFor(() => expect(commandMocks.profileDelete).toHaveBeenCalledOnce())
    expect(attempt).not.toHaveBeenCalled()
    expect(router.currentRoute.value.name).toBe('workspace-guard-probe')
  })

  it('does not let a sync refresh race an in-flight profile switch', async () => {
    const user = userEvent.setup()
    let finishSelection!: () => void
    commandMocks.profileSelect.mockReturnValueOnce(new Promise(resolve => {
      finishSelection = () => resolve({
        ok: true,
        data: { activeProfileId: contest.id, profiles: [daily, contest] },
      })
    }))
    const router = createAppRouter(createMemoryHistory())
    await router.push('/library')
    await router.isReady()
    render(App, { global: { plugins: [router], stubs: { transition: false } } })

    const trigger = await screen.findByRole('button', { name: /当前学习档案：日常学习/ })
    await user.click(trigger)
    await user.click(screen.getByRole('button', { name: '切换到档案：竞赛强化' }))
    await waitFor(() => expect(commandMocks.profileSelect).toHaveBeenCalledWith('contest'))
    expect(trigger).toBeDisabled()

    commandMocks.profileList.mockResolvedValueOnce({
      ok: true,
      data: { activeProfileId: contest.id, profiles: [daily, contest] },
    })
    commandMocks.authRestore.mockResolvedValue({
      status: 'ok',
      data: {
        ok: true,
        data: {
          configured: true,
          status: { kind: 'connected', emailHint: 's***@example.test' },
        },
      },
    })
    window.dispatchEvent(new Event('online'))
    await waitFor(() => expect(commandMocks.syncNow).toHaveBeenCalledOnce())
    await Promise.resolve()
    await Promise.resolve()

    expect(commandMocks.profileList).toHaveBeenCalledOnce()
    expect(trigger).toBeDisabled()

    finishSelection()
    await waitFor(() => {
      expect(commandMocks.profileList).toHaveBeenCalledTimes(2)
      expect(router.currentRoute.value.name).toBe('dashboard')
      expect(screen.getByRole('button', { name: /当前学习档案：竞赛强化/ })).toBeVisible()
    })
  })

  it('restores a connected session and starts one background sync after unlock', async () => {
    commandMocks.authRestore.mockResolvedValue({
      status: 'ok',
      data: {
        ok: true,
        data: {
          configured: true,
          status: { kind: 'connected', emailHint: 's***@example.test' },
        },
      },
    })
    const router = createAppRouter(createMemoryHistory())
    await router.push('/')
    await router.isReady()
    render(App, { global: { plugins: [router], stubs: { transition: false } } })

    await waitFor(() => {
      expect(commandMocks.authRestore).toHaveBeenCalledOnce()
      expect(commandMocks.syncNow).toHaveBeenCalledOnce()
      expect(screen.getByText('本地与云端已同步')).toBeVisible()
    })

    document.dispatchEvent(new Event('visibilitychange'))
    await Promise.resolve()

    expect(commandMocks.authRestore).toHaveBeenCalledOnce()
    expect(commandMocks.syncNow).toHaveBeenCalledOnce()
  })

  it('coalesces same-turn access retries into one workspace startup transaction', async () => {
    const retryAccess = deferred<Awaited<ReturnType<typeof commandMocks.libraryAccessStatus>>>()
    commandMocks.libraryAccessStatus
      .mockResolvedValueOnce({
        ok: false,
        error: {
          code: 'LIBRARY_ACCESS_UNAVAILABLE',
          userMessage: '暂时无法确认资料库状态。',
          retryable: true,
          diagnosticId: 'access-retry',
        },
      })
      .mockReturnValue(retryAccess.promise)
    commandMocks.authRestore.mockResolvedValue({
      status: 'ok',
      data: {
        ok: true,
        data: {
          configured: true,
          status: { kind: 'connected', emailHint: 's***@example.test' },
        },
      },
    })
    const router = createAppRouter(createMemoryHistory())
    await router.push('/')
    await router.isReady()
    render(App, { global: { plugins: [router], stubs: { transition: false } } })
    const retry = await screen.findByRole('button', { name: '重新检查' })

    retry.click()
    retry.click()

    expect(commandMocks.libraryAccessStatus).toHaveBeenCalledTimes(2)
    retryAccess.resolve({
      ok: true,
      data: { locked: false, trustedWindowsAccount: true },
    })
    expect(await screen.findByRole('button', { name: '训练台' })).toBeVisible()
    await waitFor(() => {
      expect(commandMocks.systemStatus).toHaveBeenCalledOnce()
      expect(commandMocks.profileList).toHaveBeenCalledTimes(2)
      expect(commandMocks.backupRestoreStatus).toHaveBeenCalledOnce()
      expect(commandMocks.authRestore).toHaveBeenCalledOnce()
      expect(commandMocks.syncNow).toHaveBeenCalledOnce()
    })
  })

  it('keeps the local workspace usable when session restore reports offline', async () => {
    commandMocks.authRestore.mockResolvedValue({
      status: 'ok',
      data: {
        ok: true,
        data: {
          configured: true,
          status: { kind: 'offline', emailHint: 's***@example.test' },
        },
      },
    })
    const router = createAppRouter(createMemoryHistory())
    await router.push('/')
    await router.isReady()
    render(App, { global: { plugins: [router], stubs: { transition: false } } })

    expect(await screen.findByText('本地已保存 · 当前离线')).toBeVisible()
    expect(commandMocks.syncNow).not.toHaveBeenCalled()
    expect(screen.getByRole('button', { name: '训练台' })).toBeVisible()
  })

  it('defers background sync while phone capture is active', async () => {
    commandMocks.authRestore.mockResolvedValue({
      status: 'ok',
      data: {
        ok: true,
        data: {
          configured: true,
          status: { kind: 'connected', emailHint: 's***@example.test' },
        },
      },
    })
    commandMocks.syncNow.mockResolvedValue({
      status: 'ok',
      data: {
        ok: false,
        error: {
          code: 'SYNC_CAPTURE_ACTIVE',
          userMessage: '手机采集正在进行，当前上传不会被打断。',
          retryable: true,
          diagnosticId: 'capture-active',
        },
      },
    })
    const router = createAppRouter(createMemoryHistory())
    await router.push('/')
    await router.isReady()
    render(App, { global: { plugins: [router], stubs: { transition: false } } })

    expect(await screen.findByText('手机采集中 · 稍后同步')).toBeVisible()
    expect(commandMocks.syncNow).toHaveBeenCalledOnce()
    expect(screen.getByRole('button', { name: '训练台' })).toBeVisible()
  })

  it('coalesces automatic restore during startup sync and releases for a later trigger', async () => {
    commandMocks.authRestore.mockResolvedValue({
      status: 'ok',
      data: {
        ok: true,
        data: {
          configured: true,
          status: { kind: 'connected', emailHint: 's***@example.test' },
        },
      },
    })
    let finishSync!: () => void
    commandMocks.syncNow.mockReturnValue(new Promise(resolve => {
      finishSync = () => resolve({
        status: 'ok',
        data: {
          ok: true,
          data: {
            pushedOperationCount: 0,
            uploadedAssetCount: 0,
            pulledChangeCount: 0,
            downloadedAssetCount: 0,
            finalCursor: 0,
          },
        },
      })
    }))
    const router = createAppRouter(createMemoryHistory())
    await router.push('/')
    await router.isReady()
    render(App, { global: { plugins: [router], stubs: { transition: false } } })

    await waitFor(() => expect(commandMocks.syncNow).toHaveBeenCalledOnce())
    window.dispatchEvent(new Event('online'))
    await Promise.resolve()
    expect(commandMocks.authRestore).toHaveBeenCalledOnce()
    expect(commandMocks.syncNow).toHaveBeenCalledOnce()

    finishSync()
    expect(await screen.findByText('本地与云端已同步')).toBeVisible()
    await waitFor(() => expect(commandMocks.profileList).toHaveBeenCalledTimes(2))
    await waitFor(() => expect(
      screen.getByRole('button', { name: /当前学习档案：日常学习/ }),
    ).toBeEnabled())
    await new Promise(resolve => window.setTimeout(resolve, 0))

    window.dispatchEvent(new Event('online'))
    await waitFor(() => {
      expect(commandMocks.authRestore).toHaveBeenCalledTimes(2)
      expect(commandMocks.syncNow).toHaveBeenCalledTimes(2)
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
    const lockProbe = {
      setup() {
        const controller = inject(libraryAccessControllerKey)
        return { restart: () => controller?.enterRestarting() }
      },
      template: '<button type="button" @click="restart">模拟锁定成功</button>',
    }
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

  it('deletes a confirmed noncurrent profile without replacing the active workspace', async () => {
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
      expect(router.currentRoute.value.name).toBe('library')
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

  it('stacks simultaneous global notices and dismisses each notice independently', async () => {
    const user = userEvent.setup()
    commandMocks.compatibilityStatus.mockResolvedValue({
      status: 'ok',
      data: {
        ok: true,
        data: {
          supportLevel: 'unsupported',
          supported: false,
          osName: 'Windows 10',
          displayVersion: '22H2',
          buildNumber: 19045,
          updateBuildRevision: 0,
          processArchitecture: 'x86_64',
          nativeArchitecture: 'x86_64',
          webview2Version: '138.0.3351.83',
          minimumWindowsBuild: 17763,
          summary: '当前系统已超出完整支持范围。',
        },
      },
    })
    commandMocks.backupRestoreStatus.mockResolvedValue({
      ok: true,
      data: { status: 'succeeded', label: '周日完整备份', finishedAtUtcMs: 10 },
    })
    const router = createAppRouter(createMemoryHistory())
    await router.push('/')
    await router.isReady()
    render(App, { global: { plugins: [router], stubs: { transition: false } } })

    const restoreNotice = (await screen.findByText('资料库恢复成功')).closest('aside')
    const compatibilityNotice = (await screen.findByText('当前 Windows 环境不在支持范围')).closest('aside')

    expect(restoreNotice?.parentElement).toBe(compatibilityNotice?.parentElement)
    expect(restoreNotice?.parentElement).toHaveClass('global-notice-stack')

    await user.click(screen.getByRole('button', { name: '关闭恢复结果通知' }))
    await waitFor(() => expect(screen.queryByText('资料库恢复成功')).not.toBeInTheDocument())
    expect(screen.getByText('当前 Windows 环境不在支持范围')).toBeVisible()

    await user.click(screen.getByRole('button', { name: '关闭 Windows 兼容性通知' }))
    await waitFor(() => expect(screen.queryByText('当前 Windows 环境不在支持范围')).not.toBeInTheDocument())
  })
})
