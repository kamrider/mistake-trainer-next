import { describe, expect, it, vi } from 'vitest'

const pdfJs = vi.hoisted(() => ({
  getDocument: vi.fn(),
}))

vi.mock('pdfjs-dist/legacy/build/pdf.mjs', () => ({
  GlobalWorkerOptions: { workerSrc: '' },
  getDocument: pdfJs.getDocument,
}))
import { isPdfFile, pdfPageFileName, renderPdfPages } from './pdfPageRenderer'

describe('pdfPageRenderer', () => {
  it('recognizes PDF MIME and safe fallback extensions', () => {
    expect(isPdfFile(new File([], '试卷.PDF', { type: '' }))).toBe(true)
    expect(isPdfFile(new File([], '试卷.pdf', { type: 'application/x-pdf' }))).toBe(true)
    expect(isPdfFile(new File([], '试卷.bin', { type: 'application/pdf' }))).toBe(true)
    expect(isPdfFile(new File([], 'photo.png', { type: 'image/png' }))).toBe(false)
    expect(pdfPageFileName('数学周测.pdf', 8)).toBe('数学周测-p008.png')
  })

  it('rejects an oversized file before reading or parsing it', async () => {
    const file = new File(['pdf'], 'large.pdf', { type: 'application/pdf' })
    const pages = renderPdfPages(file, { maxBytes: 1 })
    await expect(pages.next()).rejects.toHaveProperty('code', 'too_large')
  })

  it('honors cancellation before reading bytes', async () => {
    const controller = new AbortController()
    controller.abort()
    const pages = renderPdfPages(new File(['pdf'], 'exam.pdf', { type: 'application/pdf' }), {
      signal: controller.signal,
    })
    await expect(pages.next()).rejects.toHaveProperty('code', 'canceled')
  })

  it('destroys the loading task when canceled during PDF parsing', async () => {
    let rejectParsing!: (reason?: unknown) => void
    const parsing = new Promise<never>((_resolve, reject) => { rejectParsing = reject })
    const destroy = vi.fn().mockImplementation(async () => {
      rejectParsing(new Error('loading destroyed'))
    })
    pdfJs.getDocument.mockReturnValueOnce({ promise: parsing, destroy })
    const controller = new AbortController()
    const pages = renderPdfPages(new File(['pdf'], 'exam.pdf', { type: 'application/pdf' }), {
      signal: controller.signal,
    })

    const nextPage = pages.next()
    await vi.waitFor(() => expect(pdfJs.getDocument).toHaveBeenCalledOnce())
    controller.abort()

    await expect(nextPage).rejects.toHaveProperty('code', 'canceled')
    expect(destroy).toHaveBeenCalled()
  })
})
