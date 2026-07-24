import { describe, expect, it, vi } from 'vitest'
import { success } from '../shared/api/app-result'
import { createSyncController, syncStatusCopy } from './sync-controller'

const report = {
  pushedOperationCount: 1,
  uploadedAssetCount: 0,
  pulledChangeCount: 2,
  downloadedAssetCount: 0,
  finalCursor: 3,
}

describe('sync controller', () => {
  it('maps every runtime phase to truthful local-first copy', () => {
    expect(syncStatusCopy('syncing')).toEqual({
      label: '正在安全同步',
      tone: 'active',
    })
    expect(syncStatusCopy('deferred_capture')).toEqual({
      label: '手机采集中 · 稍后同步',
      tone: 'waiting',
    })
    expect(syncStatusCopy('retry_waiting')).toEqual({
      label: '本地已保存 · 等待重试',
      tone: 'warning',
    })
  })

  it('coalesces concurrent triggers into one command', async () => {
    let finish!: () => void
    const invoke = vi.fn(() => new Promise<ReturnType<typeof success<typeof report>>>(resolve => {
      finish = () => resolve(success(report))
    }))
    const controller = createSyncController(invoke)

    const first = controller.run('online')
    const second = controller.run('visible')

    expect(invoke).toHaveBeenCalledOnce()
    expect(invoke).toHaveBeenCalledWith('online')
    finish()
    expect(await first).toEqual(await second)
  })

  it('allows a later trigger after the previous job settles', async () => {
    const invoke = vi.fn().mockResolvedValue(success(report))
    const controller = createSyncController(invoke)

    await controller.run('startup')
    await controller.run('manual')

    expect(invoke).toHaveBeenNthCalledWith(1, 'startup')
    expect(invoke).toHaveBeenNthCalledWith(2, 'manual')
  })
})
