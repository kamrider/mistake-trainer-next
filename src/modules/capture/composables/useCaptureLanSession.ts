import { ref, type Ref } from 'vue'
import type {
  CaptureLanAddress,
  CaptureLanPreflight,
  CaptureLanSession,
  CaptureLanStartInput,
} from '../../../shared/api/bindings'
import type { AppResult } from '../../../shared/api/app-result'

export interface CaptureLanOperations {
  addresses: () => Promise<AppResult<CaptureLanAddress[]>>
  preflight: () => Promise<AppResult<CaptureLanPreflight>>
  repair: () => Promise<AppResult<CaptureLanPreflight>>
  status: () => Promise<AppResult<CaptureLanSession | null>>
  start: (input: CaptureLanStartInput) => Promise<AppResult<CaptureLanSession>>
  stop: () => Promise<AppResult<boolean>>
}

interface CaptureLanSessionOptions {
  desktopAvailable: boolean
  activeBatchId: () => string | undefined
  isBlocked: () => boolean
  onBusyChange: (busy: boolean) => void
  onError: (message: string) => void
  operations: CaptureLanOperations
}

interface CaptureLanStatusRequest {
  epoch: number
  promise: Promise<AppResult<CaptureLanSession | null>>
}

export interface CaptureLanSessionController {
  addresses: Ref<CaptureLanAddress[]>
  preflight: Ref<CaptureLanPreflight | undefined>
  preflightBusy: Ref<boolean>
  session: Ref<CaptureLanSession | undefined>
  loadAddresses: () => Promise<CaptureLanAddress[]>
  loadPreflight: () => Promise<CaptureLanPreflight | undefined>
  loadStatus: () => Promise<void>
  start: (selectedAddress: string | null) => Promise<void>
  stop: (silent?: boolean) => Promise<void>
  startPolling: (intervalMs?: number) => void
  dispose: () => void
}

export function useCaptureLanSession(
  options: CaptureLanSessionOptions,
): CaptureLanSessionController {
  const addresses = ref<CaptureLanAddress[]>([])
  const preflight = ref<CaptureLanPreflight>()
  const preflightBusy = ref(false)
  const session = ref<CaptureLanSession>()
  let disposed = false
  let sessionEpoch = 0
  let sessionMutationCount = 0
  let preflightRequest: Promise<AppResult<CaptureLanPreflight>> | undefined
  let statusRequest: CaptureLanStatusRequest | undefined
  let pollTimer: ReturnType<typeof setInterval> | undefined

  function requestPreflight() {
    if (preflightRequest) return preflightRequest
    const request = options.operations.preflight()
    preflightRequest = request
    const release = () => {
      if (preflightRequest === request) preflightRequest = undefined
    }
    void request.then(release, release)
    return request
  }

  function acquireStatusRequest() {
    if (statusRequest) return statusRequest
    const request: CaptureLanStatusRequest = {
      epoch: sessionEpoch,
      promise: options.operations.status(),
    }
    statusRequest = request
    return request
  }

  function beginSessionMutation() {
    sessionEpoch += 1
    sessionMutationCount += 1
    return sessionEpoch
  }

  function endSessionMutation() {
    sessionMutationCount = Math.max(0, sessionMutationCount - 1)
  }

  async function loadAddresses(): Promise<CaptureLanAddress[]> {
    if (!options.desktopAvailable || disposed) return []
    try {
      const result = await options.operations.addresses()
      if (disposed) return []
      if (result.ok) {
        addresses.value = result.data
        return result.data
      }
    }
    catch {
      if (!disposed) addresses.value = []
    }
    return []
  }

  async function loadPreflight(): Promise<CaptureLanPreflight | undefined> {
    if (!options.desktopAvailable || disposed) return preflight.value
    preflightBusy.value = true
    try {
      const result = await requestPreflight()
      if (disposed) return undefined
      if (result.ok) {
        preflight.value = result.data
        return result.data
      }
      preflight.value = undefined
      options.onError(result.error.userMessage)
    }
    catch {
      if (!disposed) {
        preflight.value = undefined
        options.onError('没有读取到 Windows 手机连接权限，请重新检测。')
      }
    }
    finally {
      if (!disposed) preflightBusy.value = false
    }
    return undefined
  }

  async function requestPermission(): Promise<CaptureLanPreflight | undefined> {
    if (!options.desktopAvailable || disposed) return undefined
    preflightBusy.value = true
    try {
      const result = await options.operations.repair()
      if (disposed) return undefined
      if (result.ok) {
        preflight.value = result.data
        return result.data
      }
      options.onError(result.error.userMessage)
    }
    catch {
      if (!disposed) {
        options.onError('Windows 授权没有完成；下次点击“手机扫码”时会再次请求。')
      }
    }
    finally {
      if (!disposed) preflightBusy.value = false
    }
    return undefined
  }

  async function loadStatus() {
    if (
      !options.desktopAvailable
      || disposed
      || sessionMutationCount > 0
    ) {
      return
    }
    const request = acquireStatusRequest()
    try {
      const result = await request.promise
      if (
        disposed
        || request.epoch !== sessionEpoch
      ) {
        return
      }
      if (result.ok) session.value = result.data ?? undefined
    }
    catch {
      if (!disposed && request.epoch === sessionEpoch) {
        session.value = undefined
      }
    }
    finally {
      if (statusRequest === request) statusRequest = undefined
    }
  }

  async function start(selectedAddress: string | null) {
    const requestedBatchId = options.activeBatchId()
    if (
      !options.desktopAvailable
      || disposed
      || !requestedBatchId
      || options.isBlocked()
    ) {
      return
    }

    const operationEpoch = beginSessionMutation()
    options.onBusyChange(true)
    try {
      let currentPreflight = await loadPreflight()
      if (currentPreflight?.needsFirewallRepair) {
        currentPreflight = await requestPermission()
      }
      if (
        disposed
        || operationEpoch !== sessionEpoch
        || !currentPreflight?.canStart
        || options.activeBatchId() !== requestedBatchId
      ) {
        return
      }

      const currentAddresses = await loadAddresses()
      if (disposed || operationEpoch !== sessionEpoch) return
      const currentAddress = selectedAddress
        && currentAddresses.some(address => address.address === selectedAddress)
        ? selectedAddress
        : currentAddresses[0]?.address ?? null
      const result = await options.operations.start({
        batchId: requestedBatchId,
        selectedAddress: currentAddress,
      })
      if (disposed || operationEpoch !== sessionEpoch) return
      if (result.ok) session.value = result.data
      else options.onError(result.error.userMessage)
    }
    catch {
      if (!disposed && operationEpoch === sessionEpoch) {
        options.onError('手机采集服务没有启动成功，请检查 Wi‑Fi 后重试。')
      }
    }
    finally {
      endSessionMutation()
      if (!disposed) options.onBusyChange(false)
    }
  }

  async function stop(silent = false) {
    if (!options.desktopAvailable || disposed) return
    const operationEpoch = beginSessionMutation()
    try {
      const result = await options.operations.stop()
      if (disposed || operationEpoch !== sessionEpoch) return
      if (result.ok) session.value = undefined
      else if (!silent) options.onError(result.error.userMessage)
    }
    catch {
      if (!disposed && operationEpoch === sessionEpoch && !silent) {
        options.onError('手机采集服务没有停止成功；退出应用会强制关闭端口。')
      }
    }
    finally {
      endSessionMutation()
    }
  }

  function startPolling(intervalMs = 5_000) {
    if (!options.desktopAvailable || disposed) return
    if (pollTimer) clearInterval(pollTimer)
    pollTimer = setInterval(() => void loadStatus(), intervalMs)
  }

  function dispose() {
    if (disposed) return
    disposed = true
    sessionEpoch += 1
    statusRequest = undefined
    preflightRequest = undefined
    preflightBusy.value = false
    if (pollTimer) clearInterval(pollTimer)
    pollTimer = undefined
  }

  return {
    addresses,
    preflight,
    preflightBusy,
    session,
    loadAddresses,
    loadPreflight,
    loadStatus,
    start,
    stop,
    startPolling,
    dispose,
  }
}
