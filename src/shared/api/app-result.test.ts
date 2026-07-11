import { describe, expect, it } from 'vitest'
import { failure, success } from './app-result'

describe('AppResult', () => {
  it('wraps successful command data', () => {
    expect(success({ id: 'profile-1' })).toEqual({
      ok: true,
      data: { id: 'profile-1' },
    })
  })

  it('returns a stable user-safe error shape', () => {
    expect(failure('PROFILE_NOT_FOUND', '找不到学习档案', false, 'diag-1')).toEqual({
      ok: false,
      error: {
        code: 'PROFILE_NOT_FOUND',
        userMessage: '找不到学习档案',
        retryable: false,
        diagnosticId: 'diag-1',
      },
    })
  })
})
