import { ref, type Ref } from 'vue'
import type {
  CaptureImportBytesInput,
  CaptureItemSummary,
} from '../../../shared/api/bindings'
import type { AppResult } from '../../../shared/api/app-result'
import {
  PdfRenderError,
  isPdfFile,
  renderPdfPages as renderLocalPdfPages,
  type PdfPageRenderOptions,
  type RenderedPdfPage,
} from '../services/pdfPageRenderer'

export interface CaptureFileImportProgress {
  completed: number
  total: number
  failed: number
  sourceName?: string
  phase?: 'reading_pdf' | 'rendering_pdf' | 'encrypting_images'
  currentPage?: number
  pageCount?: number
  cancelable?: boolean
}

export interface CaptureDocumentImportReport {
  sourceName: string
  pageCount: number
  importedCount: number
  failedCount: number
  skippedCount: number
  canceled: boolean
  errorCode?: string
}

export interface CaptureFileImportOutcome {
  batchId: string
  failedNames: string[]
  unsupportedNames: string[]
  attemptedCount: number
  skippedCount: number
  documentReports?: CaptureDocumentImportReport[]
}

export type CaptureImportBinaryInput = Omit<CaptureImportBytesInput, 'bytes'> & {
  bytes: Uint8Array
}

const supportedImageMimeTypes = new Set([
  'image/jpeg',
  'image/png',
  'image/webp',
])
const genericFileMimeTypes = new Set(['', 'application/octet-stream'])
const supportedImageExtensionPattern = /\.(?:jpe?g|png|webp)$/i

function isSupportedImageFile(file: File) {
  const mimeType = (file.type ?? '').trim().toLowerCase()
  if (supportedImageMimeTypes.has(mimeType)) return true
  return genericFileMimeTypes.has(mimeType)
    && supportedImageExtensionPattern.test(file.name ?? '')
}

interface CaptureFileImportOptions {
  activeBatchId: () => string | undefined
  currentItemCount: () => number
  isBlocked: () => boolean
  onBusyChange: (busy: boolean) => void
  importBytes: (
    input: CaptureImportBinaryInput,
  ) => Promise<AppResult<CaptureItemSummary>>
  createUploadId?: () => string
  maxBatchItems?: number
  renderPdfPages?: (
    file: File,
    options: PdfPageRenderOptions,
  ) => AsyncGenerator<RenderedPdfPage>
}

export interface CaptureFileImportController {
  progress: Ref<CaptureFileImportProgress | undefined>
  importFiles: (files: File[]) => Promise<CaptureFileImportOutcome | undefined>
  clearProgress: () => void
  cancelImport: () => void
  dispose: () => void
}

export function useCaptureFileImport(
  options: CaptureFileImportOptions,
): CaptureFileImportController {
  const progress = ref<CaptureFileImportProgress>()
  const requestedMaxBatchItems = options.maxBatchItems ?? 150
  const maxBatchItems = Number.isFinite(requestedMaxBatchItems)
    ? Math.max(1, Math.floor(requestedMaxBatchItems))
    : 150
  const createUploadId = options.createUploadId ?? (() => crypto.randomUUID())
  const renderPdfPages = options.renderPdfPages ?? renderLocalPdfPages
  let disposed = false
  let running = false
  let progressEpoch = 0
  let activeAbortController: AbortController | undefined

  function clearProgress() {
    progressEpoch += 1
    progress.value = undefined
  }

  async function importFiles(
    files: File[],
  ): Promise<CaptureFileImportOutcome | undefined> {
    const batchId = options.activeBatchId()
    if (
      disposed
      || running
      || options.isBlocked()
      || !batchId
      || !files.length
    ) {
      return undefined
    }

    const supportedFiles: File[] = []
    const unsupportedNames: string[] = []
    for (const file of files) {
      if (isSupportedImageFile(file) || isPdfFile(file)) supportedFiles.push(file)
      else unsupportedNames.push(file.name || 'clipboard-image')
    }

    const currentItemCount = Math.max(0, options.currentItemCount())
    const remainingCapacity = Math.max(0, maxBatchItems - currentItemCount)
    if (supportedFiles.some(isPdfFile)) {
      const documentReports: CaptureDocumentImportReport[] = []
      const failedNames: string[] = []
      let attemptedCount = 0
      let skippedCount = 0
      let remaining = remainingCapacity
      const currentProgressEpoch = ++progressEpoch
      activeAbortController = new AbortController()
      progress.value = { completed: 0, total: Math.min(remaining, supportedFiles.length), failed: 0 }
      running = true
      options.onBusyChange(true)

      const setProgress = (update: Partial<CaptureFileImportProgress>) => {
        if (disposed || progressEpoch !== currentProgressEpoch) return
        progress.value = { ...(progress.value ?? { completed: 0, total: 0, failed: 0 }), ...update }
      }
      const importOne = async (file: File) => {
        const sourceName = file.name || 'clipboard-image'
        attemptedCount += 1
        let imported = false
        try {
          const bytes = new Uint8Array(await file.arrayBuffer())
          const result = await options.importBytes({
            batchId,
            clientUploadId: createUploadId(),
            sourceName,
            // Let the Rust transaction append after every stored item, including
            // superseded crop sources that are intentionally hidden from detail.
            sourceSequence: null,
            bytes,
          })
          imported = result.ok
          if (!result.ok) failedNames.push(sourceName)
        }
        catch {
          failedNames.push(sourceName)
        }
        finally {
          remaining = Math.max(0, remaining - 1)
          setProgress({
            completed: (progress.value?.completed ?? 0) + 1,
            failed: failedNames.length,
            sourceName,
            phase: 'encrypting_images',
          })
        }
        return imported
      }

      try {
        for (const file of supportedFiles) {
          if (activeAbortController.signal.aborted) break
          if (isSupportedImageFile(file)) {
            if (remaining === 0) {
              skippedCount += 1
              continue
            }
            setProgress({ cancelable: false })
            await importOne(file)
            continue
          }

          const report: CaptureDocumentImportReport = {
            sourceName: file.name || 'document.pdf',
            pageCount: 0,
            importedCount: 0,
            failedCount: 0,
            skippedCount: 0,
            canceled: false,
          }
          documentReports.push(report)
          if (remaining === 0) {
            report.errorCode = 'capacity_full'
            continue
          }
          setProgress({
            sourceName: report.sourceName,
            phase: 'reading_pdf',
            currentPage: 0,
            cancelable: true,
          })
          const pageLimit = remaining
          try {
            for await (const page of renderPdfPages(file, {
              maxBytes: 100 * 1024 * 1024,
              maxPages: 150,
              pageLimit,
              signal: activeAbortController.signal,
            })) {
              report.pageCount = page.pageCount
              report.skippedCount = Math.max(0, page.pageCount - pageLimit)
              setProgress({
                sourceName: report.sourceName,
                phase: 'rendering_pdf',
                currentPage: page.pageNumber,
                pageCount: page.pageCount,
                total: (progress.value?.completed ?? 0) + Math.min(page.pageCount, pageLimit),
              })
              if (await importOne(page.file)) report.importedCount += 1
              else report.failedCount += 1
            }
            skippedCount += report.skippedCount
          }
          catch (error) {
            if (error instanceof PdfRenderError) {
              report.errorCode = error.code
              report.canceled = error.code === 'canceled'
            }
            else report.errorCode = 'render_failed'
          }
          finally {
            setProgress({ cancelable: false })
          }
        }
        if (disposed) return undefined
        return {
          batchId,
          failedNames,
          unsupportedNames,
          attemptedCount,
          skippedCount,
          documentReports,
        }
      }
      finally {
        activeAbortController = undefined
        running = false
        if (!disposed) options.onBusyChange(false)
      }
    }

    const filesToImport = supportedFiles.slice(0, remainingCapacity)
    const skippedCount = supportedFiles.length - filesToImport.length
    const attemptedCount = filesToImport.length
    const failedNames: string[] = []
    if (!filesToImport.length) {
      return { batchId, failedNames, unsupportedNames, attemptedCount, skippedCount }
    }

    const currentProgressEpoch = ++progressEpoch
    progress.value = {
      completed: 0,
      total: filesToImport.length,
      failed: 0,
    }
    running = true
    options.onBusyChange(true)
    let nextFileIndex = 0

    const updateProgress = () => {
      if (disposed || progressEpoch !== currentProgressEpoch) return
      progress.value = {
        completed: (progress.value?.completed ?? 0) + 1,
        total: filesToImport.length,
        failed: failedNames.length,
      }
    }

    const importOne = async (file: File) => {
      const sourceName = file.name || 'clipboard-image'
      try {
        const bytes = new Uint8Array(await file.arrayBuffer())
        const result = await options.importBytes({
          batchId,
          clientUploadId: createUploadId(),
          sourceName,
          // Server-side allocation uses MAX(source_sequence) + 1 and cannot
          // collide with hidden derivation history or gaps from failed imports.
          sourceSequence: null,
          bytes,
        })
        if (!result.ok) failedNames.push(sourceName)
      }
      catch {
        failedNames.push(sourceName)
      }
      finally {
        updateProgress()
      }
    }

    try {
      // Keep command dispatch ordered; Rust assigns the durable sequence inside
      // its serialized database transaction.
      while (nextFileIndex < filesToImport.length) {
        const index = nextFileIndex
        nextFileIndex += 1
        await importOne(filesToImport[index]!)
      }
      if (disposed) return undefined
      return { batchId, failedNames, unsupportedNames, attemptedCount, skippedCount }
    }
    finally {
      running = false
      if (!disposed) options.onBusyChange(false)
    }
  }

  function dispose() {
    if (disposed) return
    activeAbortController?.abort()
    disposed = true
    clearProgress()
  }

  return {
    progress,
    importFiles,
    clearProgress,
    cancelImport: () => activeAbortController?.abort(),
    dispose,
  }
}
