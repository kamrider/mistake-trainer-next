interface CaptureRefreshSchedulerOptions {
  activeBatchId: () => string | undefined
  refreshDetail: (batchId: string) => Promise<void>
  refreshList: () => Promise<void>
  refreshLanStatus: () => Promise<void>
  delayMs?: number
}

export function useCaptureRefreshScheduler(options: CaptureRefreshSchedulerOptions) {
  const delayMs = Number.isFinite(options.delayMs)
    ? Math.max(0, Math.floor(options.delayMs ?? 120))
    : 120
  const pendingBatchIds = new Set<string>()
  let timer: ReturnType<typeof setTimeout> | undefined
  let disposed = false

  async function flush() {
    if (timer) clearTimeout(timer)
    timer = undefined
    if (disposed || !pendingBatchIds.size) return
    const changedBatchIds = [...pendingBatchIds]
    pendingBatchIds.clear()
    const activeBatchId = options.activeBatchId()
    const tasks: Promise<void>[] = [options.refreshLanStatus()]
    if (activeBatchId && changedBatchIds.includes(activeBatchId)) {
      tasks.push(options.refreshDetail(activeBatchId))
    }
    if (!activeBatchId || changedBatchIds.some(batchId => batchId !== activeBatchId)) {
      tasks.push(options.refreshList())
    }
    await Promise.allSettled(tasks)
  }

  function schedule(batchId: string) {
    if (disposed || !batchId) return
    pendingBatchIds.add(batchId)
    if (timer) clearTimeout(timer)
    timer = setTimeout(() => { void flush() }, delayMs)
  }

  function dispose() {
    if (disposed) return
    disposed = true
    pendingBatchIds.clear()
    if (timer) clearTimeout(timer)
    timer = undefined
  }

  return { schedule, flush, dispose }
}
