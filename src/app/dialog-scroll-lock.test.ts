import { describe, expect, it } from 'vitest'
import { acquireDialogScrollLock } from './dialog-scroll-lock'

describe('acquireDialogScrollLock', () => {
  it('keeps nested locks active until the final idempotent release', () => {
    const previousOverflow = document.body.style.overflow
    document.body.style.overflow = 'auto'

    try {
      const releaseFirst = acquireDialogScrollLock(document)
      const releaseSecond = acquireDialogScrollLock(document)
      expect(document.body.style.overflow).toBe('hidden')

      releaseFirst()
      expect(document.body.style.overflow).toBe('hidden')

      releaseSecond()
      releaseSecond()
      expect(document.body.style.overflow).toBe('auto')

      document.body.style.overflow = 'clip'
      const releaseFresh = acquireDialogScrollLock(document)
      expect(document.body.style.overflow).toBe('hidden')
      releaseFresh()
      expect(document.body.style.overflow).toBe('clip')
    }
    finally {
      document.body.style.overflow = previousOverflow
    }
  })
})
