import { describe, expect, it, vi } from 'vitest'
import type { ProfileOverview } from '../../shared/api/bindings'
import { failure, success, type AppResult } from '../../shared/api/app-result'
import { useProfileManagement } from './useProfileManagement'

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

function createHarness(enabled = true) {
  const listProfiles = vi.fn(async () => success(initial))
  const scheduleSync = vi.fn()
  const refreshWorkspace = vi.fn(async () => undefined)
  const controller = useProfileManagement({
    enabled,
    listProfiles,
    scheduleSync,
    refreshWorkspace,
  })
  return { controller, listProfiles, scheduleSync, refreshWorkspace }
}

describe('useProfileManagement', () => {
  it('keeps loading single-flight and rejects a concurrent mutation', async () => {
    const current = createHarness()
    const gate = deferred<AppResult<ProfileOverview>>()
    const mutationOperation = vi.fn(async () => success(switched))
    current.listProfiles.mockReturnValueOnce(gate.promise)

    const loading = current.controller.loadProfiles()
    expect(current.controller.busy.value).toBe(true)
    expect(await current.controller.loadProfiles()).toBe(false)
    const mutationAccepted = await current.controller.mutateProfile(
      mutationOperation,
      { refreshWorkspace: true, scheduleSync: true },
    )

    expect(mutationAccepted).toBe(false)
    expect(current.listProfiles).toHaveBeenCalledOnce()
    expect(mutationOperation).not.toHaveBeenCalled()
    gate.resolve(success(initial))
    expect(await loading).toBe(true)
    expect(current.controller.profiles.value).toEqual([daily])
    expect(current.controller.activeProfileId.value).toBe(daily.id)
    expect(current.controller.busy.value).toBe(false)
  })

  it('keeps a mutation single-flight against refreshes and other mutations', async () => {
    const current = createHarness()
    const gate = deferred<AppResult<ProfileOverview>>()
    const operation = vi.fn(() => gate.promise)
    const competing = vi.fn(async () => success(initial))
    current.listProfiles.mockResolvedValueOnce(success(switched))

    const mutation = current.controller.mutateProfile(
      operation,
      { refreshWorkspace: true, scheduleSync: true },
    )
    expect(current.controller.busy.value).toBe(true)

    expect(await current.controller.loadProfiles()).toBe(false)
    expect(await current.controller.mutateProfile(
      competing,
      { refreshWorkspace: false, scheduleSync: false },
    )).toBe(false)
    expect(current.listProfiles).not.toHaveBeenCalled()
    expect(competing).not.toHaveBeenCalled()

    gate.resolve(success(switched))
    expect(await mutation).toBe(true)
    expect(current.controller.profiles.value).toEqual([daily, contest])
    expect(current.controller.activeProfileId.value).toBe(contest.id)
    expect(current.scheduleSync).toHaveBeenCalledOnce()
    expect(current.refreshWorkspace).toHaveBeenCalledOnce()
    expect(current.listProfiles).toHaveBeenCalledOnce()
    expect(current.controller.busy.value).toBe(false)
  })

  it('runs a queued refresh silently without revoking mutation feedback', async () => {
    const durable = createHarness()
    const durableGate = deferred<AppResult<ProfileOverview>>()
    durable.listProfiles.mockResolvedValueOnce(failure(
      'profile_list_failed', '学习档案读取失败。', true, 'diag-queued-list',
    ))
    const durableMutation = durable.controller.mutateProfile(
      () => durableGate.promise,
      { refreshWorkspace: false, scheduleSync: false },
    )
    expect(await durable.controller.loadProfiles()).toBe(false)
    durableGate.resolve(success(switched))

    expect(await durableMutation).toBe(true)
    expect(durable.controller.profiles.value).toEqual([daily, contest])
    expect(durable.controller.errorMessage.value).toBe('')
    expect(durable.listProfiles).toHaveBeenCalledOnce()

    const rejected = createHarness()
    const rejectedGate = deferred<AppResult<ProfileOverview>>()
    rejected.listProfiles.mockRejectedValueOnce(new Error('refresh unavailable'))
    const rejectedMutation = rejected.controller.mutateProfile(
      () => rejectedGate.promise,
      { refreshWorkspace: false, scheduleSync: false },
    )
    expect(await rejected.controller.loadProfiles()).toBe(false)
    rejectedGate.resolve(failure(
      'profile_select_failed', '档案没有切换。', true, 'diag-select',
    ))

    expect(await rejectedMutation).toBe(false)
    expect(rejected.controller.errorMessage.value).toBe('档案没有切换。')
    expect(rejected.listProfiles).toHaveBeenCalledOnce()
  })

  it('reports list application and transport failures without replacing the overview', async () => {
    const rejected = createHarness()
    rejected.listProfiles.mockResolvedValueOnce(failure(
      'profile_list_failed', '学习档案读取失败。', true, 'diag-list',
    ))
    expect(await rejected.controller.loadProfiles()).toBe(false)
    expect(rejected.controller.errorMessage.value).toBe('学习档案读取失败。')
    expect(rejected.controller.profiles.value).toEqual([])

    const thrown = createHarness()
    thrown.listProfiles.mockRejectedValueOnce(new Error('database unavailable'))
    expect(await thrown.controller.loadProfiles()).toBe(false)
    expect(thrown.controller.errorMessage.value).toBe('学习档案没有读取成功，请重新打开应用后重试。')
    expect(thrown.controller.busy.value).toBe(false)
  })

  it('reports mutation application and transport failures without running side effects', async () => {
    const rejected = createHarness()
    expect(await rejected.controller.mutateProfile(
      async () => failure('profile_failed', '档案没有切换。', true, 'diag-mutation'),
      { refreshWorkspace: true, scheduleSync: true },
    )).toBe(false)
    expect(rejected.controller.errorMessage.value).toBe('档案没有切换。')
    expect(rejected.scheduleSync).not.toHaveBeenCalled()
    expect(rejected.refreshWorkspace).not.toHaveBeenCalled()

    const thrown = createHarness()
    expect(await thrown.controller.mutateProfile(
      async () => { throw new Error('database unavailable') },
      { refreshWorkspace: true, scheduleSync: true },
    )).toBe(false)
    expect(thrown.controller.errorMessage.value).toBe('学习档案没有完成这次操作，请稍后重试。')
    expect(thrown.controller.busy.value).toBe(false)
  })

  it('preserves durable mutation success when optional side effects fail', async () => {
    const current = createHarness()
    current.scheduleSync.mockImplementationOnce(() => { throw new Error('scheduler unavailable') })
    current.refreshWorkspace.mockRejectedValueOnce(new Error('navigation cancelled'))

    expect(await current.controller.mutateProfile(
      async () => success(switched),
      { refreshWorkspace: true, scheduleSync: true },
    )).toBe(true)

    expect(current.controller.profiles.value).toEqual([daily, contest])
    expect(current.controller.activeProfileId.value).toBe(contest.id)
    expect(current.controller.errorMessage.value).toBe('')
    expect(current.scheduleSync).toHaveBeenCalledOnce()
    expect(current.refreshWorkspace).toHaveBeenCalledOnce()
    expect(current.controller.busy.value).toBe(false)
  })

  it('skips disabled work and respects mutation side-effect policy', async () => {
    const disabled = createHarness(false)
    const operation = vi.fn(async () => success(switched))
    expect(await disabled.controller.loadProfiles()).toBe(false)
    expect(await disabled.controller.mutateProfile(
      operation,
      { refreshWorkspace: true, scheduleSync: true },
    )).toBe(false)
    expect(disabled.listProfiles).not.toHaveBeenCalled()
    expect(operation).not.toHaveBeenCalled()

    const current = createHarness()
    expect(await current.controller.mutateProfile(
      async () => success(switched),
      { refreshWorkspace: false, scheduleSync: false },
    )).toBe(true)
    expect(current.scheduleSync).not.toHaveBeenCalled()
    expect(current.refreshWorkspace).not.toHaveBeenCalled()
  })
})
