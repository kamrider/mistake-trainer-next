import { afterEach, describe, expect, it, vi } from 'vitest'
import { failure, success } from '../shared/api/app-result'
import { createSyncController, syncStatusCopy } from './sync-controller'

const report = {
  pushedOperationCount: 1,
  uploadedAssetCount: 0,
  pulledChangeCount: 2,
  downloadedAssetCount: 0,
  finalCursor: 3,
}

describe('sync controller', () => {
  afterEach(() => {
    vi.useRealTimers()
  })

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

  it('debounces a burst of local mutations into one fresh sync', async () => {
    vi.useFakeTimers()
    const invoke = vi.fn().mockResolvedValue(success(report))
    const controller = createSyncController(invoke, { mutationDebounceMs: 800 })

    controller.scheduleMutation()
    controller.scheduleMutation()
    controller.scheduleMutation()
    await vi.advanceTimersByTimeAsync(799)
    expect(invoke).not.toHaveBeenCalled()

    await vi.advanceTimersByTimeAsync(1)
    expect(invoke).toHaveBeenCalledOnce()
    expect(invoke).toHaveBeenCalledWith('mutation')
  })

  it('runs a fresh pass when a mutation arrives during an older direct sync', async () => {
    vi.useFakeTimers()
    let finishFirst!: () => void
    const invoke = vi.fn()
      .mockReturnValueOnce(new Promise<ReturnType<typeof success<typeof report>>>(resolve => {
        finishFirst = () => resolve(success(report))
      }))
      .mockResolvedValue(success(report))
    const controller = createSyncController(invoke, { mutationDebounceMs: 20 })

    const direct = controller.run('manual')
    controller.scheduleMutation()
    await vi.advanceTimersByTimeAsync(20)
    expect(invoke).toHaveBeenCalledOnce()

    finishFirst()
    await direct
    await vi.advanceTimersByTimeAsync(0)

    expect(invoke).toHaveBeenCalledTimes(2)
    expect(invoke).toHaveBeenNthCalledWith(2, 'mutation')
  })

  it('keeps a mutation that arrives during the mutation pass for one later pass', async () => {
    vi.useFakeTimers()
    let finishFirst!: () => void
    const invoke = vi.fn()
      .mockReturnValueOnce(new Promise<ReturnType<typeof success<typeof report>>>(resolve => {
        finishFirst = () => resolve(success(report))
      }))
      .mockResolvedValue(success(report))
    const controller = createSyncController(invoke, { mutationDebounceMs: 20 })

    controller.scheduleMutation()
    await vi.advanceTimersByTimeAsync(20)
    expect(invoke).toHaveBeenCalledOnce()

    controller.scheduleMutation()
    await vi.advanceTimersByTimeAsync(20)
    finishFirst()
    await vi.advanceTimersByTimeAsync(20)

    expect(invoke).toHaveBeenCalledTimes(2)
    expect(invoke).toHaveBeenNthCalledWith(2, 'mutation')
  })

  it('stays quiet when mutation sync is ineligible or disposed', async () => {
    vi.useFakeTimers()
    let eligible = false
    const invoke = vi.fn().mockResolvedValue(success(report))
    const controller = createSyncController(invoke, {
      mutationDebounceMs: 10,
      canScheduleMutation: () => eligible,
    })

    controller.scheduleMutation()
    await vi.advanceTimersByTimeAsync(10)
    expect(invoke).not.toHaveBeenCalled()

    eligible = true
    controller.scheduleMutation()
    controller.dispose()
    await vi.advanceTimersByTimeAsync(10)
    expect(invoke).not.toHaveBeenCalled()
  })

  it('does not automatically loop after a failed mutation sync', async () => {
    vi.useFakeTimers()
    const invoke = vi.fn().mockResolvedValue(failure(
      'cloud_network',
      '网络暂时不可用。',
      true,
      'network',
    ))
    const controller = createSyncController(invoke, { mutationDebounceMs: 10 })

    controller.scheduleMutation()
    await vi.advanceTimersByTimeAsync(100)

    expect(invoke).toHaveBeenCalledOnce()
  })
})
