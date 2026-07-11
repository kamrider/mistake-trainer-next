import type { AppResult as GeneratedAppResult } from './bindings'
import type { AppResult } from './app-result'

export function normalizeAppResult<T>(generated: GeneratedAppResult<T>): AppResult<T> {
  if ('error' in generated && generated.error !== undefined) {
    return { ok: false, error: generated.error }
  }

  return { ok: true, data: generated.data }
}
