import { describe, expect, it, vi } from 'vitest'
import { failure, success, type AppResult } from '../../shared/api/app-result'
import type { ProfileOverview } from '../../shared/api/bindings'
import {
  useProfileManagement,
  type ProfileManagementOptions,
} from './useProfileManagement'

const daily = {
  id: 'daily', name: '日常学习', createdAtUtcMs: 1, updatedAtUtcMs: 1, revision: 1,
}
const contest = {
  id: 'contest', name: '竞赛强化', createdAtUtcMs: 2, updatedAtUtcMs: 2, revision: 1,
}
const initial: ProfileOverview = { activeProfileId: daily.id, profiles: [daily] }
const switched: ProfileOverview = { activeProfileId: contest.id, profiles: [daily, contest] }

function deferred<T>() {
  let resolve!: (value: T) => void
  let reject!: (reason?: unknown) => void
  const promise = new Promise<T>((finish, fail) => { resolve = finish; reject = fail })
  return { promise, resolve, reject }
}

function createHarness(
  overrides: Partial<Omit<ProfileManagementOptions, 'operations'>> = {},
) {
  const operations = {
    list: vi.fn(async () => success(initial)),
    create: vi.fn(async () => success(switched)),
    rename: vi.fn(async () => success(switched)),
    remove: vi.fn(async () => success(initial)),
    select: vi.fn(async () => success(switched)),
  }
  const attemptWorkspaceTransition = vi.fn(async () => true)
  const scheduleSync = vi.fn()
  const refreshWorkspace = vi.fn(async () => undefined)
  const controller = useProfileManagement({
    enabled: true,
    operations,
    attemptWorkspaceTransition,
    scheduleSync,
    refreshWorkspace,
    ...overrides,
  })
  return {
    controller,
    operations,
    attemptWorkspaceTransition,
    scheduleSync,
    refreshWorkspace,
  }
}

describe('useProfileManagement', () => {
  it('projects real desktop state and one stable browser-preview profile', () => {
    const desktop = createHarness()
    expect(desktop.controller.shellProfiles.value).toEqual([])
    expect(desktop.controller.shellActiveProfileId.value).toBe('')

    const preview = createHarness({ enabled: false })
    expect(preview.controller.shellProfiles.value).toEqual([expect.objectContaining({
      id: 'preview-profile',
      name: '本机学习档案',
    })])
    expect(preview.controller.shellActiveProfileId.value).toBe('preview-profile')
  })

  it('keeps loading single-flight and rejects a concurrent profile action', async () => {
    const current = createHarness()
    const gate = deferred<AppResult<ProfileOverview>>()
    current.operations.list.mockReturnValueOnce(gate.promise)

    const loading = current.controller.loadProfiles()
    expect(current.controller.busy.value).toBe(true)
    expect(await current.controller.loadProfiles()).toBe(false)
    expect(await current.controller.renameProfile('daily', '每日复盘')).toBe(false)

    expect(current.operations.list).toHaveBeenCalledOnce()
    expect(current.operations.rename).not.toHaveBeenCalled()
    gate.resolve(success(initial))
    expect(await loading).toBe(true)
    expect(current.controller.profiles.value).toEqual([daily])
    expect(current.controller.activeProfileId.value).toBe(daily.id)
    expect(current.controller.busy.value).toBe(false)
  })

  it('owns create and rename transition, refresh, and sync policies', async () => {
    const created = createHarness()
    expect(await created.controller.createProfile('错题冲刺')).toBe(true)
    expect(created.attemptWorkspaceTransition).toHaveBeenCalledOnce()
    expect(created.operations.create).toHaveBeenCalledWith('错题冲刺')
    expect(created.refreshWorkspace).toHaveBeenCalledOnce()
    expect(created.scheduleSync).toHaveBeenCalledOnce()

    const renamed = createHarness()
    expect(await renamed.controller.renameProfile('contest', '竞赛提高')).toBe(true)
    expect(renamed.operations.rename).toHaveBeenCalledWith('contest', '竞赛提高')
    expect(renamed.attemptWorkspaceTransition).not.toHaveBeenCalled()
    expect(renamed.refreshWorkspace).not.toHaveBeenCalled()
    expect(renamed.scheduleSync).toHaveBeenCalledOnce()
  })

  it('guards and refreshes only deletion of the active profile', async () => {
    const active = createHarness()
    await active.controller.loadProfiles()
    expect(await active.controller.deleteProfile('daily', '日常学习')).toBe(true)
    expect(active.operations.remove).toHaveBeenCalledWith('daily', '日常学习')
    expect(active.attemptWorkspaceTransition).toHaveBeenCalledOnce()
    expect(active.refreshWorkspace).toHaveBeenCalledOnce()
    expect(active.scheduleSync).toHaveBeenCalledOnce()

    const inactive = createHarness()
    await inactive.controller.loadProfiles()
    expect(await inactive.controller.deleteProfile('contest', '竞赛强化')).toBe(true)
    expect(inactive.attemptWorkspaceTransition).not.toHaveBeenCalled()
    expect(inactive.refreshWorkspace).not.toHaveBeenCalled()
    expect(inactive.scheduleSync).toHaveBeenCalledOnce()
  })

  it('skips the current profile and guards a real selection without scheduling mutation sync', async () => {
    const current = createHarness()
    await current.controller.loadProfiles()

    expect(await current.controller.selectProfile('daily')).toBe(false)
    expect(current.operations.select).not.toHaveBeenCalled()
    expect(current.attemptWorkspaceTransition).not.toHaveBeenCalled()

    expect(await current.controller.selectProfile('contest')).toBe(true)
    expect(current.operations.select).toHaveBeenCalledWith('contest')
    expect(current.attemptWorkspaceTransition).toHaveBeenCalledOnce()
    expect(current.refreshWorkspace).toHaveBeenCalledOnce()
    expect(current.scheduleSync).not.toHaveBeenCalled()
  })

  it('cancels guarded work before invoking a native operation', async () => {
    const current = createHarness({
      attemptWorkspaceTransition: vi.fn(async () => false),
    })

    expect(await current.controller.createProfile('错题冲刺')).toBe(false)
    expect(current.operations.create).not.toHaveBeenCalled()
    expect(current.refreshWorkspace).not.toHaveBeenCalled()
    expect(current.scheduleSync).not.toHaveBeenCalled()
  })

  it('queues one silent profile refresh behind an in-flight mutation', async () => {
    const current = createHarness()
    await current.controller.loadProfiles()
    current.operations.list.mockClear()
    const gate = deferred<AppResult<ProfileOverview>>()
    current.operations.select.mockReturnValueOnce(gate.promise)

    const mutation = current.controller.selectProfile('contest')
    await Promise.resolve()
    expect(current.controller.busy.value).toBe(true)
    expect(await current.controller.loadProfiles()).toBe(false)
    expect(await current.controller.renameProfile('daily', '每日复盘')).toBe(false)
    expect(current.operations.list).not.toHaveBeenCalled()
    expect(current.operations.rename).not.toHaveBeenCalled()

    gate.resolve(success(switched))
    expect(await mutation).toBe(true)
    expect(current.operations.list).toHaveBeenCalledOnce()
    expect(current.controller.errorMessage.value).toBe('')
    expect(current.controller.busy.value).toBe(false)
  })

  it('reports load application and transport failures without replacing the overview', async () => {
    const rejected = createHarness()
    rejected.operations.list.mockResolvedValueOnce(failure(
      'profile_list_failed', '学习档案读取失败。', true, 'diag-list',
    ))
    expect(await rejected.controller.loadProfiles()).toBe(false)
    expect(rejected.controller.errorMessage.value).toBe('学习档案读取失败。')
    expect(rejected.controller.profiles.value).toEqual([])

    const thrown = createHarness()
    thrown.operations.list.mockRejectedValueOnce(new Error('database unavailable'))
    expect(await thrown.controller.loadProfiles()).toBe(false)
    expect(thrown.controller.errorMessage.value).toBe('学习档案没有读取成功，请重新打开应用后重试。')
    expect(thrown.controller.busy.value).toBe(false)
  })

  it('reports mutation failures without running durable-success side effects', async () => {
    const rejected = createHarness()
    rejected.operations.rename.mockResolvedValueOnce(failure(
      'profile_failed', '档案没有重命名。', true, 'diag-mutation',
    ))
    expect(await rejected.controller.renameProfile('daily', '每日复盘')).toBe(false)
    expect(rejected.controller.errorMessage.value).toBe('档案没有重命名。')
    expect(rejected.scheduleSync).not.toHaveBeenCalled()

    const thrown = createHarness()
    thrown.operations.rename.mockRejectedValueOnce(new Error('database unavailable'))
    expect(await thrown.controller.renameProfile('daily', '每日复盘')).toBe(false)
    expect(thrown.controller.errorMessage.value).toBe('学习档案没有完成这次操作，请稍后重试。')
    expect(thrown.controller.busy.value).toBe(false)
  })

  it('preserves durable mutation success when optional side effects fail', async () => {
    const current = createHarness({
      scheduleSync: vi.fn(() => { throw new Error('scheduler unavailable') }),
      refreshWorkspace: vi.fn(async () => { throw new Error('navigation cancelled') }),
    })

    expect(await current.controller.createProfile('错题冲刺')).toBe(true)
    expect(current.controller.profiles.value).toEqual([daily, contest])
    expect(current.controller.activeProfileId.value).toBe(contest.id)
    expect(current.controller.errorMessage.value).toBe('')
    expect(current.controller.busy.value).toBe(false)
  })

  it('skips all disabled profile work', async () => {
    const disabled = createHarness({ enabled: false })

    expect(await disabled.controller.loadProfiles()).toBe(false)
    expect(await disabled.controller.createProfile('错题冲刺')).toBe(false)
    expect(await disabled.controller.renameProfile('daily', '每日复盘')).toBe(false)
    expect(await disabled.controller.deleteProfile('daily', '日常学习')).toBe(false)
    expect(await disabled.controller.selectProfile('contest')).toBe(false)
    expect(disabled.operations.list).not.toHaveBeenCalled()
    expect(disabled.operations.create).not.toHaveBeenCalled()
    expect(disabled.operations.rename).not.toHaveBeenCalled()
    expect(disabled.operations.remove).not.toHaveBeenCalled()
    expect(disabled.operations.select).not.toHaveBeenCalled()
    expect(disabled.attemptWorkspaceTransition).not.toHaveBeenCalled()
  })
})
