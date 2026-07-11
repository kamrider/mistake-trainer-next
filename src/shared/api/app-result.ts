export interface AppError {
  code: string
  userMessage: string
  retryable: boolean
  diagnosticId: string
}

export type AppResult<T> =
  | { ok: true; data: T }
  | { ok: false; error: AppError }

export function success<T>(data: T): AppResult<T> {
  return { ok: true, data }
}

export function failure(
  code: string,
  userMessage: string,
  retryable: boolean,
  diagnosticId: string,
): AppResult<never> {
  return {
    ok: false,
    error: { code, userMessage, retryable, diagnosticId },
  }
}
