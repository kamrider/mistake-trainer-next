import type { CaptureBatchDetail, CaptureImportReport } from '../../../shared/api/bindings'
import type { AppResult } from '../../../shared/api/app-result'
import type { CaptureFileImportController, CaptureFileImportOutcome } from './useCaptureFileImport'

interface CaptureImportWorkflowOptions {
  desktopAvailable: boolean
  activeDetail: () => CaptureBatchDetail | undefined
  isBlocked: () => boolean
  onBusyChange: (busy: boolean) => void
  onError: (message: string) => void
  loadBatches: () => Promise<void>
  loadDetail: (batchId: string) => Promise<void>
  select: (batchId: string) => Promise<AppResult<CaptureImportReport>>
  fileImporter: CaptureFileImportController
}

function buildImportNotice(outcome: CaptureFileImportOutcome) {
  const notices: string[] = []
  if (outcome.unsupportedNames.length) {
    const preview = outcome.unsupportedNames.slice(0, 3).join('、')
    const suffix = outcome.unsupportedNames.length > 3
      ? ` 等 ${outcome.unsupportedNames.length} 个文件`
      : ''
    notices.push(`${preview}${suffix} 不是支持的图片格式；仅支持 PNG、JPEG 和 WebP。`)
  }
  if (outcome.skippedCount) {
    notices.push(`本批最多保存 150 张，已跳过最后 ${outcome.skippedCount} 张图片。`)
  }
  if (outcome.failedNames.length) {
    const preview = outcome.failedNames.slice(0, 3).join('、')
    const suffix = outcome.failedNames.length > 3 ? ` 等 ${outcome.failedNames.length} 张` : ''
    notices.push(`${preview}${suffix} 未能加入采集箱，其余图片已继续导入。`)
  }
  return notices.join(' ')
}

export function useCaptureImportWorkflow(options: CaptureImportWorkflowOptions) {
  const isActive = (batchId: string) => options.activeDetail()?.batch.id === batchId

  async function importSelect() {
    const batchId = options.activeDetail()?.batch.id
    if (!options.desktopAvailable || !batchId || options.isBlocked()) return
    options.onBusyChange(true)
    options.onError('')
    try {
      const result = await options.select(batchId)
      if (isActive(batchId)) {
        if (!result.ok) options.onError(result.error.userMessage)
        await options.loadDetail(batchId)
      }
      else if (result.ok) await options.loadBatches()
    }
    catch {
      if (isActive(batchId)) options.onError('图片选择没有完成，请稍后重试。')
    }
    finally {
      options.onBusyChange(false)
    }
  }

  async function importFiles(files: File[]) {
    const batchId = options.activeDetail()?.batch.id
    if (!options.desktopAvailable || !batchId || options.isBlocked() || !files.length) return
    options.onError('')
    let refreshing = false
    try {
      const outcome = await options.fileImporter.importFiles(files)
      if (!outcome) return
      if (isActive(outcome.batchId)) {
        const notice = buildImportNotice(outcome)
        if (notice) options.onError(notice)
      }
      if (outcome.attemptedCount === 0) return
      refreshing = true
      options.onBusyChange(true)
      if (isActive(outcome.batchId)) {
        await options.loadDetail(outcome.batchId)
      }
      else {
        await options.loadBatches()
      }
    }
    catch {
      refreshing = true
      options.onBusyChange(true)
      if (isActive(batchId)) {
        options.onError('拖入或粘贴的图片没有全部保存；已成功的图片仍在批次中。')
        await options.loadDetail(batchId)
      }
      else {
        await options.loadBatches()
      }
    }
    finally {
      if (refreshing) options.onBusyChange(false)
      options.fileImporter.clearProgress()
    }
  }

  async function importFromPaste(event: ClipboardEvent) {
    const current = options.activeDetail()
    if (!current || current.batch.state === 'completed') return
    if (event.target instanceof HTMLInputElement || event.target instanceof HTMLTextAreaElement) return
    const files = [...(event.clipboardData?.items ?? [])]
      .filter(item => item.kind === 'file')
      .map(item => item.getAsFile())
      .filter((file): file is File => Boolean(file))
    if (!files.length) return
    event.preventDefault()
    await importFiles(files)
  }

  return {
    progress: options.fileImporter.progress,
    importSelect,
    importFiles,
    importFromPaste,
    clearProgress: options.fileImporter.clearProgress,
    dispose: options.fileImporter.dispose,
  }
}
