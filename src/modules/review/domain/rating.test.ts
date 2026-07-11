import { describe, expect, it } from 'vitest'
import { mapSimpleRating } from './rating'

describe('mapSimpleRating', () => {
  it('maps forgot to FSRS Again', () => {
    expect(mapSimpleRating('forgot')).toBe('again')
  })

  it('maps remembered to FSRS Good', () => {
    expect(mapSimpleRating('remembered')).toBe('good')
  })
})
