import { describe, expect, it, vi } from 'vitest'
import type {
  CaptureItemSummary,
} from '../../../shared/api/bindings'
import {
  failure,
  success,
  type AppResult,
} from '../../../shared/api/app-result'
import {
  PdfRenderError,
  type PdfPageRenderOptions,
  type RenderedPdfPage,
} from '../services/pdfPageRenderer'
import { useCaptureFileImport } from './useCaptureFileImport'

function deferred<T>() {
  let resolve!: (value: T) => void
  const promise = new Promise<T>((finish) => {
    resolve = finish
  })
  return { promise, resolve }
}

function files(...names: string[]): File[] {
  return names.map((name, index) => ({
    name,
    arrayBuffer: vi.fn().mockResolvedValue(new Uint8Array([index + 1]).buffer),
  }) as unknown as File)
}

const importedItem = {} as CaptureItemSummary

function createHarness(renderPdfPages?: (
  file: File,
  options: PdfPageRenderOptions,
) => AsyncGenerator<RenderedPdfPage>) {
  let activeBatchId: string | undefined = 'batch-1'
  let currentItemCount = 4
  let blocked = false
  let uploadId = 0
  const importBytes = vi.fn().mockResolvedValue(success(importedItem))
  const onBusyChange = vi.fn((value: boolean) => {
    blocked = value
  })
  const controller = useCaptureFileImport({
    activeBatchId: () => activeBatchId,
    currentItemCount: () => currentItemCount,
    isBlocked: () => blocked,
    onBusyChange,
    importBytes,
    createUploadId: () => `upload-${++uploadId}`,
    ...(renderPdfPages ? { renderPdfPages } : {}),
  })
  return {
    controller,
    importBytes,
    onBusyChange,
    setActiveBatchId: (value: string | undefined) => { activeBatchId = value },
    setCurrentItemCount: (value: number) => { currentItemCount = value },
    setBlocked: (value: boolean) => { blocked = value },
  }
}

describe('useCaptureFileImport', () => {
  it('dispatches imports in order and delegates durable sequence allocation', async () => {
    const first = deferred<AppResult<CaptureItemSummary>>()
    const second = deferred<AppResult<CaptureItemSummary>>()
    const third = deferred<AppResult<CaptureItemSummary>>()
    const harness = createHarness()
    harness.importBytes
      .mockReturnValueOnce(first.promise)
      .mockReturnValueOnce(second.promise)
      .mockReturnValueOnce(third.promise)

    const importing = harness.controller.importFiles(files('a.png', 'b.png', 'c.png'))
    await vi.waitFor(() => expect(harness.importBytes).toHaveBeenCalledOnce())
    first.resolve(success(importedItem))
    await vi.waitFor(() => expect(harness.importBytes).toHaveBeenCalledTimes(2))
    second.resolve(success(importedItem))
    await vi.waitFor(() => expect(harness.importBytes).toHaveBeenCalledTimes(3))
    third.resolve(success(importedItem))
    await importing

    expect(harness.importBytes.mock.calls.map(call => call[0].sourceSequence))
      .toEqual([null, null, null])
    expect(harness.importBytes.mock.calls.map(call => call[0].clientUploadId))
      .toEqual(['upload-1', 'upload-2', 'upload-3'])
  })

  it('uses only the remaining batch capacity', async () => {
    const harness = createHarness()
    harness.setCurrentItemCount(149)

    const result = await harness.controller.importFiles(files('a.png', 'b.png', 'c.png'))

    expect(harness.importBytes).toHaveBeenCalledOnce()
    expect(result).toMatchObject({
      batchId: 'batch-1',
      failedNames: [],
      unsupportedNames: [],
      attemptedCount: 1,
      skippedCount: 2,
    })
    expect(harness.importBytes.mock.calls[0]![0].sourceSequence).toBeNull()
  })

  it('accepts supported image types by MIME or fallback extension before reading bytes', async () => {
    const png = {
      name: 'photo.PNG',
      type: '',
      arrayBuffer: vi.fn().mockResolvedValue(new Uint8Array([1]).buffer),
    } as unknown as File
    const jpeg = {
      name: 'answer.jpeg',
      type: 'application/octet-stream',
      arrayBuffer: vi.fn().mockResolvedValue(new Uint8Array([2]).buffer),
    } as unknown as File
    const webp = {
      name: 'scan.webp',
      type: 'image/webp',
      arrayBuffer: vi.fn().mockResolvedValue(new Uint8Array([3]).buffer),
    } as unknown as File
    const text = {
      name: 'notes.txt',
      type: 'text/plain',
      arrayBuffer: vi.fn().mockResolvedValue(new Uint8Array([4]).buffer),
    } as unknown as File
    const harness = createHarness()

    const result = await harness.controller.importFiles([png, jpeg, webp, text])

    expect(harness.importBytes).toHaveBeenCalledTimes(3)
    expect(text.arrayBuffer).not.toHaveBeenCalled()
    expect(result).toMatchObject({
      batchId: 'batch-1',
      failedNames: [],
      unsupportedNames: ['notes.txt'],
      attemptedCount: 3,
      skippedCount: 0,
    })
  })

  it('applies capacity after removing unsupported files', async () => {
    const unsupported = {
      name: 'notes.txt',
      type: 'text/plain',
      arrayBuffer: vi.fn(),
    } as unknown as File
    const harness = createHarness()
    harness.setCurrentItemCount(149)

    const result = await harness.controller.importFiles([
      unsupported,
      ...files('first.png', 'second.png'),
    ])

    expect(harness.importBytes).toHaveBeenCalledOnce()
    expect(unsupported.arrayBuffer).not.toHaveBeenCalled()
    expect(result).toMatchObject({
      unsupportedNames: ['notes.txt'],
      attemptedCount: 1,
      skippedCount: 1,
    })
  })

  it('imports a PDF as ordered page images alongside ordinary images', async () => {
    async function* renderPages(): AsyncGenerator<RenderedPdfPage> {
      yield {
        pageNumber: 1,
        pageCount: 2,
        file: files('exam-p001.png')[0]!,
      }
      yield {
        pageNumber: 2,
        pageCount: 2,
        file: files('exam-p002.png')[0]!,
      }
    }
    const harness = createHarness(renderPages)
    const pdf = {
      name: 'exam.pdf',
      type: 'application/pdf',
      arrayBuffer: vi.fn(),
    } as unknown as File

    const result = await harness.controller.importFiles([
      files('cover.png')[0]!,
      pdf,
      files('answer.png')[0]!,
    ])

    expect(pdf.arrayBuffer).not.toHaveBeenCalled()
    expect(harness.importBytes.mock.calls.map(call => call[0].sourceName)).toEqual([
      'cover.png',
      'exam-p001.png',
      'exam-p002.png',
      'answer.png',
    ])
    expect(harness.importBytes.mock.calls.map(call => call[0].sourceSequence)).toEqual([
      null,
      null,
      null,
      null,
    ])
    expect(result).toMatchObject({
      attemptedCount: 4,
      skippedCount: 0,
      documentReports: [{
        sourceName: 'exam.pdf',
        pageCount: 2,
        importedCount: 2,
        failedCount: 0,
        skippedCount: 0,
        canceled: false,
      }],
    })
  })

  it('keeps imported PDF pages when the remaining document is canceled', async () => {
    const continueRendering = deferred<void>()
    async function* renderPages(
      _file: File,
      options: PdfPageRenderOptions,
    ): AsyncGenerator<RenderedPdfPage> {
      yield {
        pageNumber: 1,
        pageCount: 3,
        file: files('exam-p001.png')[0]!,
      }
      await continueRendering.promise
      if (options.signal?.aborted) throw new PdfRenderError('canceled')
      yield {
        pageNumber: 2,
        pageCount: 3,
        file: files('exam-p002.png')[0]!,
      }
    }
    const harness = createHarness(renderPages)
    const importing = harness.controller.importFiles([
      new File(['pdf'], 'exam.pdf', { type: 'application/pdf' }),
    ])

    await vi.waitFor(() => expect(harness.importBytes).toHaveBeenCalledOnce())
    harness.controller.cancelImport()
    continueRendering.resolve()
    const result = await importing

    expect(harness.importBytes).toHaveBeenCalledOnce()
    expect(result).toMatchObject({
      attemptedCount: 1,
      documentReports: [{
        sourceName: 'exam.pdf',
        pageCount: 3,
        importedCount: 1,
        canceled: true,
        errorCode: 'canceled',
      }],
    })
  })

  it('reports live completed and failed counts', async () => {
    const bad = deferred<AppResult<CaptureItemSummary>>()
    const good = deferred<AppResult<CaptureItemSummary>>()
    const harness = createHarness()
    harness.importBytes
      .mockReturnValueOnce(bad.promise)
      .mockReturnValueOnce(good.promise)

    const importing = harness.controller.importFiles(files('bad.png', 'good.png'))
    await vi.waitFor(() => expect(harness.importBytes).toHaveBeenCalledOnce())
    bad.resolve(failure('invalid', '损坏', false, 'diag-1'))
    await vi.waitFor(() => {
      expect(harness.controller.progress.value).toEqual({
        completed: 1,
        total: 2,
        failed: 1,
      })
    })
    await vi.waitFor(() => expect(harness.importBytes).toHaveBeenCalledTimes(2))
    good.resolve(success(importedItem))
    const result = await importing

    expect(result?.failedNames).toEqual(['bad.png'])
    expect(harness.controller.progress.value).toEqual({
      completed: 2,
      total: 2,
      failed: 1,
    })
    expect(harness.onBusyChange.mock.calls).toEqual([[true], [false]])
  })

  it('continues after a file read fails', async () => {
    const broken = {
      name: 'broken.png',
      arrayBuffer: vi.fn().mockRejectedValue(new Error('broken')),
    } as unknown as File
    const harness = createHarness()

    const result = await harness.controller.importFiles([broken, ...files('good.png')])

    expect(harness.importBytes).toHaveBeenCalledOnce()
    expect(result?.failedNames).toEqual(['broken.png'])
    expect(harness.controller.progress.value?.completed).toBe(2)
  })

  it('does nothing for blocked, missing-batch, or empty imports', async () => {
    const harness = createHarness()
    harness.setBlocked(true)
    await expect(harness.controller.importFiles(files('blocked.png'))).resolves.toBeUndefined()
    harness.setBlocked(false)
    harness.setActiveBatchId(undefined)
    await expect(harness.controller.importFiles(files('missing.png'))).resolves.toBeUndefined()
    harness.setActiveBatchId('batch-1')
    await expect(harness.controller.importFiles([])).resolves.toBeUndefined()

    expect(harness.importBytes).not.toHaveBeenCalled()
    expect(harness.onBusyChange).not.toHaveBeenCalled()
  })

  it('clears progress explicitly and ignores late callbacks after disposal', async () => {
    const gate = deferred<AppResult<CaptureItemSummary>>()
    const harness = createHarness()
    harness.importBytes.mockReturnValueOnce(gate.promise)

    const importing = harness.controller.importFiles(files('slow.png'))
    await vi.waitFor(() => expect(harness.importBytes).toHaveBeenCalledOnce())
    harness.controller.clearProgress()
    expect(harness.controller.progress.value).toBeUndefined()
    harness.controller.dispose()
    gate.resolve(success(importedItem))
    await expect(importing).resolves.toBeUndefined()

    expect(harness.controller.progress.value).toBeUndefined()
    expect(harness.onBusyChange).toHaveBeenCalledTimes(1)
    await expect(harness.controller.importFiles(files('ignored.png'))).resolves.toBeUndefined()
    expect(harness.importBytes).toHaveBeenCalledOnce()
  })
})
