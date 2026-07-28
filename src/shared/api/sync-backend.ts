import { isTauri } from '@tauri-apps/api/core'
import {
  commands,
  type AppResult as GeneratedAppResult,
  type CloudBackendKind,
  type CloudBackendStatus,
} from './bindings'
import { failure, success, type AppResult } from './app-result'
import { normalizeAppResult } from './normalize-result'

type RuntimeDetector = () => boolean
type StatusCommand = () => Promise<GeneratedAppResult<CloudBackendStatus>>
type SetCommand = (request: { kind: CloudBackendKind }) => Promise<GeneratedAppResult<CloudBackendStatus>>

/**
 * Reads backend state without coupling feature code to a cloud vendor.
 * Browser previews intentionally return a deterministic local-only state and
 * never invoke Tauri or attempt a network request.
 */
export async function loadSyncBackendStatus(
  runtimeDetector: RuntimeDetector = isTauri,
  statusCommand: StatusCommand = commands.syncBackendStatus,
): Promise<AppResult<CloudBackendStatus>> {
  if (!runtimeDetector()) {
    return success({ kind: 'local-only', configured: true, ready: true, syncEnabled: false })
  }

  try {
    return normalizeAppResult(await statusCommand())
  } catch {
    return failure(
      'SYNC_STATUS_UNAVAILABLE',
      '暂时无法读取同步设置，本地数据仍可正常使用',
      true,
      'sync-status-unavailable',
    )
  }
}

/**
 * Selects a backend through the typed Rust command. Remote providers return a
 * typed, fail-closed error until their adapter and credentials are present.
 */
export async function setSyncBackend(
  kind: CloudBackendKind,
  runtimeDetector: RuntimeDetector = isTauri,
  setCommand: SetCommand = commands.syncBackendSet,
): Promise<AppResult<CloudBackendStatus>> {
  if (!runtimeDetector()) {
    return failure(
      'TAURI_REQUIRED',
      '请在桌面应用中修改同步设置',
      false,
      'sync-backend-browser-preview',
    )
  }

  try {
    return normalizeAppResult(await setCommand({ kind }))
  } catch {
    return failure(
      'SYNC_BACKEND_UNAVAILABLE',
      '同步设置暂时不可用，本地模式未受影响',
      true,
      'sync-backend-unavailable',
    )
  }
}

export function backendKindLabel(kind: CloudBackendKind): string {
  switch (kind) {
    case 'supabase': return 'Supabase（海外/开发）'
    case 'tencent': return '腾讯云（国内预留）'
    default: return '仅本地（推荐）'
  }
}

export function backendStatusLabel(status: CloudBackendStatus): string {
  if (status.kind === 'local-only') return '本地优先 · 不需要网络'
  if (!status.configured) return `${backendKindLabel(status.kind)} · 尚未配置`
  if (!status.ready) return `${backendKindLabel(status.kind)} · 等待适配器`
  return `${backendKindLabel(status.kind)} · 已连接`
}
