import { afterEach, describe, expect, it, vi } from 'vitest'
import { useCaptureRefreshScheduler } from './useCaptureRefreshScheduler'

function harness() {
  let activeBatchId: string | undefined = 'batch-1'
  const refreshDetail = vi.fn().mockResolvedValue(undefined)
  const refreshList = vi.fn().mockResolvedValue(undefined)
  const refreshLanStatus = vi.fn().mockResolvedValue(undefined)
  const scheduler = useCaptureRefreshScheduler({
    activeBatchId: () => activeBatchId,
    refreshDetail,
    refreshList,
    refreshLanStatus,
    delayMs: 120,
  })
  return {
    scheduler, refreshDetail, refreshList, refreshLanStatus,
    setActiveBatchId: (value?: string) => { activeBatchId = value },
  }
}

afterEach(() => {
  vi.useRealTimers()
})

describe('useCaptureRefreshScheduler', () => {
  it('debounces repeated active-batch changes into one detail and LAN refresh', async () => {
    vi.useFakeTimers()
    const current = harness()
    current.scheduler.schedule('batch-1')
    current.scheduler.schedule('batch-1')
    await vi.advanceTimersByTimeAsync(119)
    expect(current.refreshDetail).not.toHaveBeenCalled()
    await vi.advanceTimersByTimeAsync(1)
    expect(current.refreshDetail).toHaveBeenCalledOnce()
    expect(current.refreshDetail).toHaveBeenCalledWith('batch-1')
    expect(current.refreshList).not.toHaveBeenCalled()
    expect(current.refreshLanStatus).toHaveBeenCalledOnce()
  })

  it('refreshes only the list and LAN status for inactive changes', async () => {
    vi.useFakeTimers()
    const current = harness()
    current.scheduler.schedule('batch-2')
    await vi.runAllTimersAsync()
    expect(current.refreshDetail).not.toHaveBeenCalled()
    expect(current.refreshList).toHaveBeenCalledOnce()
    expect(current.refreshLanStatus).toHaveBeenCalledOnce()
  })

  it('preserves mixed burst identities and refreshes detail and list once each', async () => {
    vi.useFakeTimers()
    const current = harness()
    current.scheduler.schedule('batch-1')
    current.scheduler.schedule('batch-2')
    current.scheduler.schedule('batch-3')
    await vi.runAllTimersAsync()
    expect(current.refreshDetail).toHaveBeenCalledOnce()
    expect(current.refreshDetail).toHaveBeenCalledWith('batch-1')
    expect(current.refreshList).toHaveBeenCalledOnce()
    expect(current.refreshLanStatus).toHaveBeenCalledOnce()
  })

  it('chooses refresh targets from the active batch at flush time', async () => {
    vi.useFakeTimers()
    const current = harness()
    current.scheduler.schedule('batch-1')
    current.setActiveBatchId('batch-2')
    await vi.runAllTimersAsync()
    expect(current.refreshDetail).not.toHaveBeenCalled()
    expect(current.refreshList).toHaveBeenCalledOnce()
  })

  it('flushes explicitly and settles callback failures', async () => {
    vi.useFakeTimers()
    const current = harness()
    current.refreshDetail.mockRejectedValue(new Error('detail offline'))
    current.refreshLanStatus.mockRejectedValue(new Error('lan offline'))
    current.scheduler.schedule('batch-1')
    await expect(current.scheduler.flush()).resolves.toBeUndefined()
    expect(current.refreshDetail).toHaveBeenCalledOnce()
    expect(current.refreshLanStatus).toHaveBeenCalledOnce()
    await vi.runAllTimersAsync()
    expect(current.refreshDetail).toHaveBeenCalledOnce()
  })

  it('disposes pending work and ignores future schedules', async () => {
    vi.useFakeTimers()
    const current = harness()
    current.scheduler.schedule('batch-1')
    current.scheduler.dispose()
    current.scheduler.schedule('batch-2')
    await vi.runAllTimersAsync()
    await current.scheduler.flush()
    expect(current.refreshDetail).not.toHaveBeenCalled()
    expect(current.refreshList).not.toHaveBeenCalled()
    expect(current.refreshLanStatus).not.toHaveBeenCalled()
  })
})
