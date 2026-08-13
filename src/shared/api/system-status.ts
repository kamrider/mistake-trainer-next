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
