import { describe, expect, it, vi } from 'vitest'
import type {
  CaptureItemSummary,
} from '../../../shared/api/bindings'
import {
  failure,
  success,
  type AppResult,
} from '../../../shared/api/app-result'
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

function createHarness() {
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
  it('uses two workers while preserving source sequence', async () => {
    const first = deferred<AppResult<CaptureItemSummary>>()
    const second = deferred<AppResult<CaptureItemSummary>>()
    const third = deferred<AppResult<CaptureItemSummary>>()
    const harness = createHarness()
    harness.importBytes
      .mockReturnValueOnce(first.promise)
      .mockReturnValueOnce(second.promise)
      .mockReturnValueOnce(third.promise)

    const importing = harness.controller.importFiles(files('a.png', 'b.png', 'c.png'))
    await vi.waitFor(() => expect(harness.importBytes).toHaveBeenCalledTimes(2))
    first.resolve(success(importedItem))
    await vi.waitFor(() => expect(harness.importBytes).toHaveBeenCalledTimes(3))
    second.resolve(success(importedItem))
    third.resolve(success(importedItem))
    await importing

    expect(harness.importBytes.mock.calls.map(call => call[0].sourceSequence))
      .toEqual([4, 5, 6])
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
    expect(harness.importBytes.mock.calls[0]![0].sourceSequence).toBe(149)
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
    const pdf = {
      name: 'notes.pdf',
      type: 'application/pdf',
      arrayBuffer: vi.fn().mockResolvedValue(new Uint8Array([4]).buffer),
    } as unknown as File
    const harness = createHarness()

    const result = await harness.controller.importFiles([png, jpeg, webp, pdf])

    expect(harness.importBytes).toHaveBeenCalledTimes(3)
    expect(pdf.arrayBuffer).not.toHaveBeenCalled()
    expect(result).toMatchObject({
      batchId: 'batch-1',
      failedNames: [],
      unsupportedNames: ['notes.pdf'],
      attemptedCount: 3,
      skippedCount: 0,
    })
  })

  it('applies capacity after removing unsupported files', async () => {
    const unsupported = {
      name: 'notes.pdf',
      type: 'application/pdf',
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
      unsupportedNames: ['notes.pdf'],
      attemptedCount: 1,
      skippedCount: 1,
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
    await vi.waitFor(() => expect(harness.importBytes).toHaveBeenCalledTimes(2))
    bad.resolve(failure('invalid', '损坏', false, 'diag-1'))
    await vi.waitFor(() => {
      expect(harness.controller.progress.value).toEqual({
        completed: 1,
        total: 2,
        failed: 1,
      })
    })
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
