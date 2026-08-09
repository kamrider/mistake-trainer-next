import { ref, type Ref } from 'vue'
import type {
  CaptureImportBytesInput,
  CaptureItemSummary,
} from '../../../shared/api/bindings'
import type { AppResult } from '../../../shared/api/app-result'

export interface CaptureFileImportProgress {
  completed: number
  total: number
  failed: number
}

export interface CaptureFileImportOutcome {
  batchId: string
  failedNames: string[]
  unsupportedNames: string[]
  attemptedCount: number
  skippedCount: number
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
    input: CaptureImportBytesInput,
  ) => Promise<AppResult<CaptureItemSummary>>
  createUploadId?: () => string
  maxBatchItems?: number
  concurrency?: number
}

export interface CaptureFileImportController {
  progress: Ref<CaptureFileImportProgress | undefined>
  importFiles: (files: File[]) => Promise<CaptureFileImportOutcome | undefined>
  clearProgress: () => void
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
  const requestedConcurrency = options.concurrency ?? 2
  const concurrency = Number.isFinite(requestedConcurrency)
    ? Math.max(1, Math.floor(requestedConcurrency))
    : 2
  const createUploadId = options.createUploadId ?? (() => crypto.randomUUID())
  let disposed = false
  let running = false
  let progressEpoch = 0

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
      if (isSupportedImageFile(file)) supportedFiles.push(file)
      else unsupportedNames.push(file.name || 'clipboard-image')
    }

    const currentItemCount = Math.max(0, options.currentItemCount())
    const remainingCapacity = Math.max(0, maxBatchItems - currentItemCount)
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

    const importOne = async (file: File, sourceSequence: number) => {
      const sourceName = file.name || 'clipboard-image'
      try {
        const bytes = [...new Uint8Array(await file.arrayBuffer())]
        const result = await options.importBytes({
          batchId,
          clientUploadId: createUploadId(),
          sourceName,
          sourceSequence,
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
      const workerCount = Math.min(concurrency, filesToImport.length)
      await Promise.all(Array.from({ length: workerCount }, async () => {
        while (nextFileIndex < filesToImport.length) {
          const index = nextFileIndex
          nextFileIndex += 1
          await importOne(filesToImport[index]!, currentItemCount + index)
        }
      }))
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
    disposed = true
    clearProgress()
  }

  return {
    progress,
    importFiles,
    clearProgress,
    dispose,
  }
}
