import { ref } from 'vue'
import { describe, expect, it, vi } from 'vitest'
import type { CaptureBatchDetail, CaptureImportReport } from '../../../shared/api/bindings'
import { failure, success, type AppResult } from '../../../shared/api/app-result'
import type { CaptureFileImportController, CaptureFileImportOutcome } from './useCaptureFileImport'
import { useCaptureImportWorkflow } from './useCaptureImportWorkflow'

const detail = {
  batch: {
    id: 'batch-1', subject: '数学', state: 'collecting', itemCount: 0,
    draftCount: 0, readyCount: 0, updatedAtUtcMs: 1, revision: 7,
  },
  items: [], drafts: [], unassignedItemIds: [], pairSuggestions: [],
} satisfies CaptureBatchDetail
const pickerReport = { importedItems: [], importedCount: 0 } satisfies CaptureImportReport

function deferred<T>() {
  let resolve!: (value: T) => void
  let reject!: (reason?: unknown) => void
  const promise = new Promise<T>((finish, fail) => { resolve = finish; reject = fail })
  return { promise, resolve, reject }
}

function harness() {
  let current: CaptureBatchDetail | undefined = detail
  let blocked = false
  const fileImporter = {
    progress: ref(undefined),
    importFiles: vi.fn().mockResolvedValue({
      batchId: 'batch-1', failedNames: [], unsupportedNames: [], attemptedCount: 1, skippedCount: 0,
    }),
    clearProgress: vi.fn(),
    dispose: vi.fn(),
  } as CaptureFileImportController
  const select = vi.fn().mockResolvedValue(success(pickerReport))
  const onBusyChange = vi.fn((value: boolean) => { blocked = value })
  const onError = vi.fn()
  const loadBatches = vi.fn().mockResolvedValue(undefined)
  const loadDetail = vi.fn().mockResolvedValue(undefined)
  const controller = useCaptureImportWorkflow({
    desktopAvailable: true,
    activeDetail: () => current,
    isBlocked: () => blocked,
    onBusyChange,
    onError,
    loadBatches,
    loadDetail,
    select,
    fileImporter,
  })
  return {
    controller, fileImporter, select, onBusyChange, onError, loadBatches, loadDetail,
    setCurrent: (value?: CaptureBatchDetail) => { current = value },
    setBlocked: (value: boolean) => { blocked = value },
  }
}

function pasteEvent(
  target: EventTarget,
  items: Array<{ kind: string, type: string, getAsFile: () => File | null }>,
) {
  return {
    target,
    clipboardData: { items },
    preventDefault: vi.fn(),
  } as unknown as ClipboardEvent
}

describe('useCaptureImportWorkflow', () => {
  it('runs the system picker with busy ownership and refreshes the active batch', async () => {
    const current = harness()
    await current.controller.importSelect()
    expect(current.select).toHaveBeenCalledWith('batch-1')
    expect(current.onBusyChange.mock.calls).toEqual([[true], [false]])
    expect(current.onError).toHaveBeenNthCalledWith(1, '')
    expect(current.loadDetail).toHaveBeenCalledWith('batch-1')
    expect(current.loadBatches).not.toHaveBeenCalled()
  })

  it('does not reopen a batch when the picker finishes after leaving', async () => {
    const gate = deferred<AppResult<CaptureImportReport>>()
    const current = harness()
    current.select.mockReturnValue(gate.promise)
    const selecting = current.controller.importSelect()
    await vi.waitFor(() => expect(current.select).toHaveBeenCalledOnce())
    current.setCurrent(undefined)
    gate.resolve(success(pickerReport))
    await selecting
    expect(current.loadDetail).not.toHaveBeenCalled()
    expect(current.loadBatches).toHaveBeenCalledOnce()
    expect(current.onError).toHaveBeenCalledTimes(1)
    expect(current.onError).toHaveBeenLastCalledWith('')
  })

  it('forwards picker errors and uses stable fallback copy only while active', async () => {
    const rejected = harness()
    rejected.select.mockResolvedValue(
      failure('capture_import_cancelled', '没有选择图片', true, 'diag-picker'),
    )
    await rejected.controller.importSelect()
    expect(rejected.onError).toHaveBeenLastCalledWith('没有选择图片')
    expect(rejected.loadDetail).toHaveBeenCalledWith('batch-1')

    const thrown = harness()
    thrown.select.mockRejectedValue(new Error('offline'))
    await thrown.controller.importSelect()
    expect(thrown.onError).toHaveBeenLastCalledWith('图片选择没有完成，请稍后重试。')
    expect(thrown.loadDetail).not.toHaveBeenCalled()

    const late = harness()
    const gate = deferred<AppResult<CaptureImportReport>>()
    late.select.mockReturnValue(gate.promise)
    const selecting = late.controller.importSelect()
    await vi.waitFor(() => expect(late.select).toHaveBeenCalledOnce())
    late.setCurrent(undefined)
    gate.reject(new Error('late offline'))
    await selecting
    expect(late.onError).toHaveBeenCalledTimes(1)
    expect(late.onError).toHaveBeenLastCalledWith('')
  })

  it('formats partial-import notices and refreshes the active detail', async () => {
    const current = harness()
    current.fileImporter.importFiles = vi.fn().mockResolvedValue({
      batchId: 'batch-1',
      failedNames: ['坏图1.png', '坏图2.png', '坏图3.png', '坏图4.png'],
      unsupportedNames: ['说明.pdf'],
      attemptedCount: 4,
      skippedCount: 2,
    } satisfies CaptureFileImportOutcome)
    await current.controller.importFiles([new File(['x'], '题目.png')])
    expect(current.onError).toHaveBeenLastCalledWith(
      '说明.pdf 不是支持的图片格式；仅支持 PNG、JPEG 和 WebP。 本批最多保存 150 张，已跳过最后 2 张图片。 坏图1.png、坏图2.png、坏图3.png 等 4 张 未能加入采集箱，其余图片已继续导入。',
    )
    expect(current.loadDetail).toHaveBeenCalledWith('batch-1')
    expect(current.loadBatches).not.toHaveBeenCalled()
    expect(current.onBusyChange.mock.calls).toEqual([[true], [false]])
    expect(current.fileImporter.clearProgress).toHaveBeenCalledOnce()
  })

  it('refreshes only the list and suppresses detail notices after leaving', async () => {
    const gate = deferred<CaptureFileImportOutcome | undefined>()
    const current = harness()
    current.fileImporter.importFiles = vi.fn().mockReturnValue(gate.promise)
    const importing = current.controller.importFiles([new File(['x'], '题目.png')])
    await vi.waitFor(() => expect(current.fileImporter.importFiles).toHaveBeenCalledOnce())
    current.setCurrent(undefined)
    gate.resolve({
      batchId: 'batch-1', failedNames: ['坏图.png'], unsupportedNames: [],
      attemptedCount: 1, skippedCount: 1,
    })
    await importing
    expect(current.loadDetail).not.toHaveBeenCalled()
    expect(current.loadBatches).toHaveBeenCalledOnce()
    expect(current.onError).toHaveBeenCalledTimes(1)
    expect(current.onError).toHaveBeenLastCalledWith('')
    expect(current.fileImporter.clearProgress).toHaveBeenCalledOnce()
  })

  it('reports unsupported-only drops without refreshing persisted batch data', async () => {
    const current = harness()
    current.fileImporter.importFiles = vi.fn().mockResolvedValue({
      batchId: 'batch-1',
      failedNames: [],
      unsupportedNames: ['讲义.pdf'],
      attemptedCount: 0,
      skippedCount: 0,
    } satisfies CaptureFileImportOutcome)

    await current.controller.importFiles([new File(['pdf'], '讲义.pdf', { type: 'application/pdf' })])

    expect(current.onError).toHaveBeenLastCalledWith(
      '讲义.pdf 不是支持的图片格式；仅支持 PNG、JPEG 和 WebP。',
    )
    expect(current.loadDetail).not.toHaveBeenCalled()
    expect(current.loadBatches).not.toHaveBeenCalled()
    expect(current.onBusyChange).not.toHaveBeenCalled()
    expect(current.fileImporter.clearProgress).toHaveBeenCalledOnce()
  })

  it('reconciles partial failures and always clears progress', async () => {
    const current = harness()
    current.fileImporter.importFiles = vi.fn().mockRejectedValue(new Error('unexpected'))
    await current.controller.importFiles([new File(['x'], '题目.png')])
    expect(current.onError).toHaveBeenLastCalledWith('拖入或粘贴的图片没有全部保存；已成功的图片仍在批次中。')
    expect(current.loadDetail).toHaveBeenCalledWith('batch-1')
    expect(current.fileImporter.clearProgress).toHaveBeenCalledOnce()

    const empty = harness()
    empty.fileImporter.importFiles = vi.fn().mockResolvedValue(undefined)
    await empty.controller.importFiles([new File(['x'], '题目.png')])
    expect(empty.loadDetail).not.toHaveBeenCalled()
    expect(empty.loadBatches).not.toHaveBeenCalled()
    expect(empty.fileImporter.clearProgress).toHaveBeenCalledOnce()

    const gate = deferred<CaptureFileImportOutcome | undefined>()
    const late = harness()
    late.fileImporter.importFiles = vi.fn().mockReturnValue(gate.promise)
    const importing = late.controller.importFiles([new File(['x'], '题目.png')])
    await vi.waitFor(() => expect(late.fileImporter.importFiles).toHaveBeenCalledOnce())
    late.setCurrent(undefined)
    gate.reject(new Error('late unexpected'))
    await importing
    expect(late.loadDetail).not.toHaveBeenCalled()
    expect(late.loadBatches).toHaveBeenCalledOnce()
    expect(late.onError).toHaveBeenCalledTimes(1)
    expect(late.onError).toHaveBeenLastCalledWith('')
  })

  it('forwards clipboard files for centralized validation and protects text controls and completed batches', async () => {
    const image = new File(['image'], 'paste.PNG', { type: '' })
    const pdf = new File(['pdf'], 'notes.pdf', { type: 'application/pdf' })
    const current = harness()
    const event = pasteEvent(document.createElement('div'), [
      { kind: 'file', type: '', getAsFile: () => image },
      { kind: 'file', type: 'application/pdf', getAsFile: () => pdf },
      { kind: 'string', type: 'text/plain', getAsFile: () => null },
    ])
    await current.controller.importFromPaste(event)
    expect(event.preventDefault).toHaveBeenCalledOnce()
    expect(current.fileImporter.importFiles).toHaveBeenCalledWith([image, pdf])

    const input = harness()
    const inputEvent = pasteEvent(document.createElement('input'), [
      { kind: 'file', type: 'image/png', getAsFile: () => image },
    ])
    await input.controller.importFromPaste(inputEvent)
    expect(inputEvent.preventDefault).not.toHaveBeenCalled()
    expect(input.fileImporter.importFiles).not.toHaveBeenCalled()

    const completed = harness()
    completed.setCurrent({ ...detail, batch: { ...detail.batch, state: 'completed' } })
    const completedEvent = pasteEvent(document.createElement('div'), [
      { kind: 'file', type: 'image/png', getAsFile: () => image },
    ])
    await completed.controller.importFromPaste(completedEvent)
    expect(completedEvent.preventDefault).not.toHaveBeenCalled()
    expect(completed.fileImporter.importFiles).not.toHaveBeenCalled()
  })

  it('delegates progress cleanup/disposal and ignores blocked, batchless, or empty calls', async () => {
    const current = harness()
    expect(current.controller.progress).toBe(current.fileImporter.progress)
    current.controller.clearProgress()
    current.controller.dispose()
    expect(current.fileImporter.clearProgress).toHaveBeenCalledOnce()
    expect(current.fileImporter.dispose).toHaveBeenCalledOnce()

    const blocked = harness()
    blocked.setBlocked(true)
    await blocked.controller.importSelect()
    await blocked.controller.importFiles([new File(['x'], '题目.png')])
    expect(blocked.select).not.toHaveBeenCalled()
    expect(blocked.fileImporter.importFiles).not.toHaveBeenCalled()

    const batchless = harness()
    batchless.setCurrent(undefined)
    await batchless.controller.importSelect()
    await batchless.controller.importFiles([])
    expect(batchless.select).not.toHaveBeenCalled()
    expect(batchless.fileImporter.importFiles).not.toHaveBeenCalled()
  })
})
