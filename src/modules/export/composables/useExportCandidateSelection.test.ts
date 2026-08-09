import { describe, expect, it, vi } from 'vitest'
import type { ExportCandidate } from '../../../shared/api/bindings'
import { useExportCandidateSelection } from './useExportCandidateSelection'

function deferred<T>() {
  let resolve!: (value: T) => void
  const promise = new Promise<T>((finish) => { resolve = finish })
  return { promise, resolve }
}

const math: ExportCandidate = {
  id: 'math', subject: '数学', note: '圆锥曲线', questionAssetCount: 1,
  answerAssetCount: 1, dueAtUtcMs: null, reviewCount: 1,
}
const physics: ExportCandidate = {
  id: 'physics', subject: '物理', note: '受力分析', questionAssetCount: 1,
  answerAssetCount: 1, dueAtUtcMs: null, reviewCount: 2,
}
const chemistry: ExportCandidate = {
  id: 'chemistry', subject: '化学', note: '反应路线', questionAssetCount: 1,
  answerAssetCount: 0, dueAtUtcMs: null, reviewCount: 0,
}

describe('useExportCandidateSelection', () => {
  it('selects every candidate after the first authoritative load', async () => {
    const load = vi.fn().mockResolvedValue({ ok: true, data: [math, physics] })
    const selection = useExportCandidateSelection({ load })

    await expect(selection.loadCandidates()).resolves.toBe(true)

    expect(selection.candidates.value).toEqual([math, physics])
    expect(selection.selectedIds.value).toEqual(['math', 'physics'])
  })

  it('keeps a same-source selection visible while refreshing and after failure', async () => {
    const refresh = deferred<{
      ok: false
      error: { code: string, userMessage: string, retryable: boolean, diagnosticId: string }
    }>()
    const load = vi.fn()
      .mockResolvedValueOnce({ ok: true, data: [math, physics] })
      .mockReturnValueOnce(refresh.promise)
    const selection = useExportCandidateSelection({ load })
    await selection.loadCandidates()
    selection.toggle('physics')

    const pending = selection.loadCandidates()

    expect(selection.loading.value).toBe(true)
    expect(selection.candidates.value).toEqual([math, physics])
    expect(selection.selectedIds.value).toEqual(['math'])
    refresh.resolve({
      ok: false,
      error: {
        code: 'export_candidates_failed',
        userMessage: '候选题读取失败。',
        retryable: true,
        diagnosticId: 'diag-selection',
      },
    })
    await expect(pending).resolves.toBe(false)

    expect(selection.candidates.value).toEqual([math, physics])
    expect(selection.selectedIds.value).toEqual(['math'])
    expect(selection.error.value).toBe('候选题读取失败。')
  })

  it('reconciles a same-source success without selecting new or deselected candidates', async () => {
    const load = vi.fn()
      .mockResolvedValueOnce({ ok: true, data: [math, physics] })
      .mockResolvedValueOnce({ ok: true, data: [physics, chemistry, math] })
    const selection = useExportCandidateSelection({ load })
    await selection.loadCandidates()
    selection.toggle('physics')

    await expect(selection.loadCandidates()).resolves.toBe(true)

    expect(selection.candidates.value).toEqual([physics, chemistry, math])
    expect(selection.selectedIds.value).toEqual(['math'])
  })

  it('clears the previous source while replacing it and selects the new result', async () => {
    const replacement = deferred<{ ok: true, data: ExportCandidate[] }>()
    const load = vi.fn()
      .mockResolvedValueOnce({ ok: true, data: [math] })
      .mockReturnValueOnce(replacement.promise)
    const selection = useExportCandidateSelection({ load })
    await selection.loadCandidates()

    const pending = selection.changeSource('all_active')

    expect(selection.source.value).toBe('all_active')
    expect(selection.candidates.value).toEqual([])
    expect(selection.selectedIds.value).toEqual([])
    replacement.resolve({ ok: true, data: [chemistry, physics] })
    await expect(pending).resolves.toBe(true)
    expect(selection.selectedIds.value).toEqual(['chemistry', 'physics'])
  })

  it('admits only one load in the same in-flight window', async () => {
    const request = deferred<{ ok: true, data: ExportCandidate[] }>()
    const load = vi.fn().mockReturnValue(request.promise)
    const selection = useExportCandidateSelection({ load })

    const first = selection.loadCandidates()
    const second = selection.loadCandidates()

    await expect(second).resolves.toBe(false)
    expect(load).toHaveBeenCalledOnce()
    request.resolve({ ok: true, data: [math] })
    await expect(first).resolves.toBe(true)
  })
})
