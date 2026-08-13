import { describe, expect, it, vi } from 'vitest'
import { failure, success, type AppResult } from '../../shared/api/app-result'
import type { DiagnosticExportReceipt } from '../../shared/api/bindings'
import { useSettingsDiagnosticsExport } from './useSettingsDiagnosticsExport'

const receipt: DiagnosticExportReceipt = {
  reportId: '019f4b87-4cab-7b83-a4a0-46acac7d1362',
  fileLabel: 'Mistake-Trainer-Diagnostics-safe.json',
  generatedAtUtcMs: 1_700_000_000_000,
  warningCount: 0,
}

function deferred<T>() {
  let resolve!: (value: T) => void
  const promise = new Promise<T>((finish) => { resolve = finish })
  return { promise, resolve }
}

function harness(available = true) {
  const exportReport = vi.fn(async () => success<DiagnosticExportReceipt | null>(receipt))
  const restoreFocus = vi.fn(async () => undefined)
  return {
    exportReport,
    restoreFocus,
    controller: useSettingsDiagnosticsExport({ available, exportReport, restoreFocus }),
  }
}

describe('useSettingsDiagnosticsExport', () => {
  it('applies only the accepted receipt and restores focus', async () => {
    const current = harness()
    await expect(current.controller.exportDiagnostics()).resolves.toBe(true)
    expect(current.controller.receipt.value).toEqual(receipt)
    expect(current.controller.busy.value).toBe(false)
    expect(current.controller.message.value).toBe('')
    expect(current.restoreFocus).toHaveBeenCalledOnce()
  })

  it('treats native folder cancellation as neutral', async () => {
    const current = harness()
    current.exportReport.mockResolvedValueOnce(success(null))
    await expect(current.controller.exportDiagnostics()).resolves.toBe(false)
    expect(current.controller.receipt.value).toBeUndefined()
    expect(current.controller.message.value).toBe('')
    expect(current.restoreFocus).toHaveBeenCalledOnce()
  })

  it('coalesces duplicate clicks and clears a stale receipt before retrying', async () => {
    const current = harness()
    await current.controller.exportDiagnostics()
    const pending = deferred<AppResult<DiagnosticExportReceipt | null>>()
    current.exportReport.mockReturnValueOnce(pending.promise)

    const first = current.controller.exportDiagnostics()
    const second = current.controller.exportDiagnostics()
    expect(second).toBe(first)
    expect(current.controller.receipt.value).toBeUndefined()
    expect(current.controller.busy.value).toBe(true)
    expect(current.exportReport).toHaveBeenCalledTimes(2)

    pending.resolve(success(null))
    await first
    expect(current.controller.busy.value).toBe(false)
  })

  it('keeps backend and transport failures local and retryable', async () => {
    const rejected = harness()
    rejected.exportReport.mockResolvedValueOnce(failure(
      'diagnostics_export_failed',
      '诊断报告无法保存，请更换位置后重试。',
      true,
      'private-diagnostic',
    ))
    await expect(rejected.controller.exportDiagnostics()).resolves.toBe(false)
    expect(rejected.controller.message.value).toBe('诊断报告无法保存，请更换位置后重试。')
    expect(rejected.controller.receipt.value).toBeUndefined()

    const thrown = harness()
    thrown.exportReport.mockRejectedValueOnce(new Error('private native error'))
    await expect(thrown.controller.exportDiagnostics()).resolves.toBe(false)
    expect(thrown.controller.message.value).toBe(
      '诊断报告没有生成，现有资料不会受到影响；请检查磁盘空间和保存位置后重试。',
    )
    expect(thrown.controller.message.value).not.toContain('private')
  })

  it('does no native work outside the desktop runtime', async () => {
    const current = harness(false)
    await expect(current.controller.exportDiagnostics()).resolves.toBe(false)
    expect(current.exportReport).not.toHaveBeenCalled()
    expect(current.restoreFocus).not.toHaveBeenCalled()
  })
})
