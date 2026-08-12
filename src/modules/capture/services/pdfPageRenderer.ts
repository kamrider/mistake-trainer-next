import workerUrl from 'pdfjs-dist/legacy/build/pdf.worker.min.mjs?url'

type PdfJsModule = typeof import('pdfjs-dist/legacy/build/pdf.mjs')

let pdfJsModule: Promise<PdfJsModule> | undefined

function loadPdfJs() {
  pdfJsModule ??= import('pdfjs-dist/legacy/build/pdf.mjs').then((module) => {
    module.GlobalWorkerOptions.workerSrc = workerUrl
    return module
  })
  return pdfJsModule
}

const DEFAULT_MAX_PDF_BYTES = 100 * 1024 * 1024
const DEFAULT_MAX_PAGES = 150
const MAX_RENDER_DIMENSION = 4096

export type PdfRenderErrorCode = 'too_large' | 'too_many_pages' | 'password_required' | 'invalid' | 'empty' | 'canceled' | 'render_failed'

export class PdfRenderError extends Error {
  constructor(public readonly code: PdfRenderErrorCode, cause?: unknown) {
    super(code, cause === undefined ? undefined : { cause })
    this.name = 'PdfRenderError'
  }
}

export interface RenderedPdfPage {
  pageNumber: number
  pageCount: number
  file: File
}

export interface PdfPageRenderOptions {
  maxBytes?: number
  maxPages?: number
  pageLimit?: number
  signal?: AbortSignal
}

export function isPdfFile(file: File) {
  const mime = (file.type ?? '').trim().toLowerCase()
  return mime === 'application/pdf'
    || /\.pdf$/i.test(file.name ?? '')
}

export function pdfPageFileName(sourceName: string, pageNumber: number) {
  const stem = sourceName.replace(/\.pdf$/i, '').trim() || '试卷'
  return `${stem}-p${String(pageNumber).padStart(3, '0')}.png`
}

function assertNotCanceled(signal?: AbortSignal) {
  if (signal?.aborted) throw new PdfRenderError('canceled')
}

function canvasPng(canvas: HTMLCanvasElement, fileName: string): Promise<File> {
  return new Promise((resolve, reject) => {
    canvas.toBlob((blob) => {
      if (!blob) {
        reject(new PdfRenderError('render_failed'))
        return
      }
      resolve(new File([blob], fileName, { type: 'image/png' }))
    }, 'image/png')
  })
}

export async function* renderPdfPages(
  file: File,
  options: PdfPageRenderOptions = {},
): AsyncGenerator<RenderedPdfPage> {
  const maxBytes = options.maxBytes ?? DEFAULT_MAX_PDF_BYTES
  const maxPages = options.maxPages ?? DEFAULT_MAX_PAGES
  if (file.size > maxBytes) throw new PdfRenderError('too_large')
  assertNotCanceled(options.signal)

  let loadingTask: ReturnType<PdfJsModule['getDocument']> | undefined
  try {
    const data = new Uint8Array(await file.arrayBuffer())
    assertNotCanceled(options.signal)
    const { getDocument } = await loadPdfJs()
    assertNotCanceled(options.signal)
    loadingTask = getDocument({
      data,
      isEvalSupported: false,
      useWorkerFetch: false,
    })
    const cancelLoading = () => {
      void loadingTask?.destroy()
    }
    options.signal?.addEventListener('abort', cancelLoading, { once: true })
    const pdfDocument = await (async () => {
      try {
        return await loadingTask.promise
      }
      finally {
        options.signal?.removeEventListener('abort', cancelLoading)
      }
    })()
    if (pdfDocument.numPages < 1) throw new PdfRenderError('empty')
    if (pdfDocument.numPages > maxPages) throw new PdfRenderError('too_many_pages')
    const renderCount = Math.min(pdfDocument.numPages, Math.max(0, options.pageLimit ?? pdfDocument.numPages))

    for (let pageNumber = 1; pageNumber <= renderCount; pageNumber += 1) {
      assertNotCanceled(options.signal)
      const page = await pdfDocument.getPage(pageNumber)
      const baseViewport = page.getViewport({ scale: 1 })
      const scale = Math.min(
        2,
        MAX_RENDER_DIMENSION / Math.max(baseViewport.width, baseViewport.height),
      )
      const viewport = page.getViewport({ scale })
      const canvas = globalThis.document.createElement('canvas')
      try {
        canvas.width = Math.max(1, Math.ceil(viewport.width))
        canvas.height = Math.max(1, Math.ceil(viewport.height))
        const context = canvas.getContext('2d', { alpha: false })
        if (!context) throw new PdfRenderError('render_failed')
        context.save()
        context.fillStyle = '#ffffff'
        context.fillRect(0, 0, canvas.width, canvas.height)
        context.restore()
        const renderTask = page.render({ canvas, canvasContext: context, viewport })
        const cancel = () => renderTask.cancel()
        options.signal?.addEventListener('abort', cancel, { once: true })
        try {
          await renderTask.promise
        }
        finally {
          options.signal?.removeEventListener('abort', cancel)
        }
        assertNotCanceled(options.signal)
        const renderedFile = await canvasPng(canvas, pdfPageFileName(file.name, pageNumber))
        assertNotCanceled(options.signal)
        yield {
          pageNumber,
          pageCount: pdfDocument.numPages,
          file: renderedFile,
        }
      }
      finally {
        page.cleanup()
        canvas.width = 1
        canvas.height = 1
      }
    }
  }
  catch (error) {
    if (error instanceof PdfRenderError) throw error
    if (options.signal?.aborted) throw new PdfRenderError('canceled')
    const name = error instanceof Error ? error.name : ''
    if (name === 'PasswordException') throw new PdfRenderError('password_required')
    if (name === 'InvalidPDFException' || name === 'MissingPDFException') {
      throw new PdfRenderError('invalid')
    }
    throw new PdfRenderError('render_failed', error)
  }
  finally {
    await loadingTask?.destroy()
  }
}
