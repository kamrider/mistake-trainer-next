import { describe, expect, it } from 'vitest'
import { acquireDialogDocumentBoundary } from './dialog-document-boundary'

describe('acquireDialogDocumentBoundary', () => {
  it('keeps nested background and scroll ownership until the final idempotent release', () => {
    const previousOverflow = document.body.style.overflow
    document.body.style.overflow = 'auto'
    const page = document.createElement('main')
    const background = document.createElement('button')
    const preInert = document.createElement('aside')
    preInert.setAttribute('inert', '')
    const outerModal = document.createElement('section')
    const outerContent = document.createElement('button')
    const nestedModal = document.createElement('section')
    outerModal.append(outerContent, nestedModal)
    page.append(background, preInert, outerModal)
    document.body.append(page)

    try {
      const releaseOuter = acquireDialogDocumentBoundary(outerModal)
      expect(background).toHaveAttribute('inert')
      expect(preInert).toHaveAttribute('inert')
      expect(outerModal).not.toHaveAttribute('inert')
      expect(document.body.style.overflow).toBe('hidden')

      const releaseNested = acquireDialogDocumentBoundary(nestedModal)
      expect(outerContent).toHaveAttribute('inert')
      expect(nestedModal).not.toHaveAttribute('inert')

      releaseOuter()
      releaseOuter()
      expect(background).toHaveAttribute('inert')
      expect(document.body.style.overflow).toBe('hidden')

      releaseNested()
      releaseNested()
      expect(document.body.style.overflow).toBe('auto')
      expect(background).not.toHaveAttribute('inert')
      expect(outerContent).not.toHaveAttribute('inert')
      expect(preInert).toHaveAttribute('inert')
    }
    finally {
      page.remove()
      document.body.style.overflow = previousOverflow
    }
  })
})
