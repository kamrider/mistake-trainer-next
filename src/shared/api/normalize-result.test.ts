import { describe, expect, it } from 'vitest'
import { normalizeAppResult } from './normalize-result'

describe('normalizeAppResult', () => {
  it('restores a literal success discriminator from generated bindings', () => {
    const result = normalizeAppResult({ ok: true, data: { appVersion: '0.1.0' } })

    expect(result).toEqual({ ok: true, data: { appVersion: '0.1.0' } })
  })

  it('restores a literal failure discriminator and safe error', () => {
    const error = {
      code: 'DATABASE_LOCKED',
      userMessage: '本地资料库已锁定',
      retryable: false,
      diagnosticId: 'diag-1',
    }
    const result = normalizeAppResult({ ok: false, error })

    expect(result).toEqual({ ok: false, error })
  })
})
