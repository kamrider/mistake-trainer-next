import { isTauri } from '@tauri-apps/api/core'
import { commands, type AppResult as GeneratedAppResult, type SystemStatus } from './bindings'
import { success, type AppResult } from './app-result'
import { normalizeAppResult } from './normalize-result'

type RuntimeDetector = () => boolean
type StatusCommand = () => Promise<GeneratedAppResult<SystemStatus>>

export async function loadSystemStatus(
  runtimeDetector: RuntimeDetector = isTauri,
  statusCommand: StatusCommand = commands.systemStatus,
): Promise<AppResult<SystemStatus>> {
  if (!runtimeDetector()) {
    return success({ appVersion: 'web-preview', storage: 'preview', sync: 'offline' })
  }

  return normalizeAppResult(await statusCommand())
}

export function systemStatusLabel(result: AppResult<SystemStatus> | undefined): string {
  if (result === undefined) return '正在检查资料库'
  if (!result.ok) return '状态检查失败'
  if (result.data.storage === 'locked') return '资料库已锁定'
  if (result.data.storage === 'preview') return '浏览器设计预览'
  if (result.data.sync === 'syncing') return '正在同步'
  if (result.data.sync === 'offline') return '本地已保存 · 离线'
  return '本地与云端已同步'
}
