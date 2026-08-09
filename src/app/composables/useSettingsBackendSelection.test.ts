import { describe, expect, it, vi } from 'vitest'
import type { AppResult } from '../../shared/api/app-result'
import { failure, success } from '../../shared/api/app-result'
import type { CloudBackendStatus } from '../../shared/api/bindings'
import { useSettingsBackendSelection } from './useSettingsBackendSelection'

const localStatus: CloudBackendStatus = {
  kind: 'local-only',
  configured: true,
  ready: true,
  syncEnabled: false,
}
const supabaseStatus: CloudBackendStatus = {
  kind: 'supabase',
  configured: true,
  ready: true,
  syncEnabled: true,
}

function deferred<T>() {
  let resolve!: (value: T) => void
  const promise = new Promise<T>((finish) => { resolve = finish })
  return { promise, resolve }
}

function harness() {
  const operations = {
    load: vi.fn().mockResolvedValue(success(localStatus)),
    select: vi.fn().mockResolvedValue(success(supabaseStatus)),
    label: vi.fn((kind: string) => kind === 'supabase' ? 'Supabase（海外/开发）' : '仅本地（推荐）'),
  }
  return {
    operations,
    controller: useSettingsBackendSelection(operations),
  }
}

describe('useSettingsBackendSelection', () => {
  it('keeps a completed selection authoritative over an older status read', async () => {
    const oldRead = deferred<AppResult<CloudBackendStatus>>()
    const h = harness()
    h.operations.load.mockReturnValueOnce(oldRead.promise)

    const loading = h.controller.loadStatus()
    await vi.waitFor(() => expect(h.operations.load).toHaveBeenCalledOnce())
    await expect(h.controller.choose('supabase')).resolves.toBe(true)
    expect(h.controller.status.value).toEqual(success(supabaseStatus))

    oldRead.resolve(success(localStatus))
    await expect(loading).resolves.toBe(false)
    expect(h.controller.status.value).toEqual(success(supabaseStatus))
  })

  it('applies only the newest concurrent status read', async () => {
    const oldRead = deferred<AppResult<CloudBackendStatus>>()
    const newRead = deferred<AppResult<CloudBackendStatus>>()
    const h = harness()
    h.operations.load
      .mockReturnValueOnce(oldRead.promise)
      .mockReturnValueOnce(newRead.promise)

    const first = h.controller.loadStatus()
    const second = h.controller.loadStatus()
    newRead.resolve(success(supabaseStatus))
    await expect(second).resolves.toBe(true)
    oldRead.resolve(success(localStatus))
    await expect(first).resolves.toBe(false)

    expect(h.controller.status.value).toEqual(success(supabaseStatus))
  })

  it('admits one selection and refuses status reads while it is pending', async () => {
    const selection = deferred<AppResult<CloudBackendStatus>>()
    const h = harness()
    h.operations.select.mockReturnValueOnce(selection.promise)

    const first = h.controller.choose('supabase')
    await vi.waitFor(() => expect(h.operations.select).toHaveBeenCalledOnce())
    expect(h.controller.busy.value).toBe(true)
    await expect(h.controller.choose('local-only')).resolves.toBe(false)
    await expect(h.controller.loadStatus()).resolves.toBe(false)
    expect(h.operations.load).not.toHaveBeenCalled()

    selection.resolve(success(supabaseStatus))
    await expect(first).resolves.toBe(true)
    expect(h.controller.busy.value).toBe(false)
  })

  it('does not send the current or unavailable backend', async () => {
    const h = harness()
    await h.controller.loadStatus()

    await expect(h.controller.choose('local-only')).resolves.toBe(false)
    await expect(h.controller.choose('tencent')).resolves.toBe(false)

    expect(h.operations.select).not.toHaveBeenCalled()
  })

  it('keeps prior status and exposes exact command failure copy', async () => {
    const h = harness()
    await h.controller.loadStatus()
    h.operations.select.mockResolvedValueOnce(failure(
      'SYNC_BACKEND_NOT_CONFIGURED',
      '该同步服务尚未配置，已保持本地模式',
      false,
      'sync-backend-selection',
    ))

    await expect(h.controller.choose('supabase')).resolves.toBe(false)

    expect(h.controller.status.value).toEqual(success(localStatus))
    expect(h.controller.message.value).toBe('该同步服务尚未配置，已保持本地模式')
  })

  it('uses safe fallback copy when a selection adapter throws', async () => {
    const h = harness()
    h.operations.select.mockRejectedValueOnce(new Error('bridge unavailable'))

    await expect(h.controller.choose('supabase')).resolves.toBe(false)

    expect(h.controller.message.value).toBe('同步后端设置暂时不可用，本地数据不会受到影响。')
    expect(h.controller.busy.value).toBe(false)
  })
})
