import { render, screen, waitFor } from '@testing-library/vue'
import userEvent from '@testing-library/user-event'
import { beforeEach, describe, expect, it, vi } from 'vitest'
import { syncControllerKey } from '@/shared/contracts/sync-controller'
import LegacyImportPanel from './LegacyImportPanel.vue'

const api = vi.hoisted(() => ({
  legacyScan: vi.fn(), legacyImport: vi.fn(), legacyImportList: vi.fn(), legacyRollback: vi.fn(),
}))
const eventState = vi.hoisted(() => ({ listener: undefined as ((event: { payload: unknown }) => void) | undefined, unlisten: vi.fn() }))
vi.mock('../../../shared/api/bindings', () => ({ commands: api }))
vi.mock('@tauri-apps/api/event', () => ({
  listen: vi.fn(async (_name: string, listener: (event: { payload: unknown }) => void) => {
    eventState.listener = listener
    return eventState.unlisten
  }),
}))

const candidate = {
  candidateId: 'candidate-safe', problemCount: 2, expiresAtUtcMs: Date.now() + 1_800_000,
  report: {
    members: 1, metadataRecords: 4, existingAssets: 4, trainingRecords: 3,
    frozenRecords: 1, duplicateAssets: 0, truncated: false, issues: [],
  },
}
const completedImport = {
  importId: 'import-new', memberCount: 1, problemCount: 2, assetCount: 4,
  reviewCount: 3, status: 'completed' as const, createdAtUtcMs: Date.now(), rolledBackAtUtcMs: null,
}

function deferred<T>() {
  let resolve!: (value: T) => void
  const promise = new Promise<T>((finish) => { resolve = finish })
  return { promise, resolve }
}

const syncController = {
  run: vi.fn(),
  scheduleMutation: vi.fn(),
  dispose: vi.fn(),
}
const renderPanel = () => render(LegacyImportPanel, {
  global: { provide: { [syncControllerKey as symbol]: syncController } },
})

describe('LegacyImportPanel', () => {
  beforeEach(() => {
    vi.clearAllMocks()
    eventState.listener = undefined
    api.legacyImportList.mockResolvedValue({ ok: true, data: [] })
  })

  it('keeps a safe candidate through confirmation, progress and successful completion', async () => {
    let finishImport!: (value: unknown) => void
    api.legacyScan.mockResolvedValue({ status: 'ok', data: { ok: true, data: candidate } })
    api.legacyImport.mockReturnValue(new Promise(resolve => { finishImport = resolve }))
    renderPanel()

    await userEvent.click(screen.getByRole('button', { name: '选择旧版目录并扫描' }))
    expect(await screen.findByText('预检完成，尚未写入新题库')).toBeVisible()
    expect(screen.getByText('2', { selector: 'dd' })).toBeVisible()

    const openConfirm = screen.getByRole('button', { name: '查看范围并确认导入' })
    await userEvent.click(openConfirm)
    expect(screen.getByRole('button', { name: '取消，保持现状' })).toHaveFocus()
    await userEvent.keyboard('{Escape}')
    expect(openConfirm).toHaveFocus()

    await userEvent.click(openConfirm)
    await userEvent.click(screen.getByRole('checkbox', { name: /导入只复制数据/ }))
    await userEvent.click(screen.getByRole('button', { name: '确认并开始导入' }))
    await waitFor(() => expect(api.legacyImport).toHaveBeenCalledWith('candidate-safe'))
    eventState.listener?.({ payload: { candidateId: 'candidate-safe', phase: 'writing', completed: 1, total: 2 } })
    expect(await screen.findByText('68%')).toBeVisible()
    expect(screen.getByRole('progressbar')).toHaveAttribute('aria-valuenow', '68')

    finishImport({ status: 'ok', data: { ok: true, data: {
      importId: 'import-one', memberCount: 1, problemCount: 2, assetCount: 4,
      reviewCount: 3, frozenProblemCount: 1, createdAtUtcMs: Date.now(),
    } } })
    expect(await screen.findByText('2 道旧题已经进入新题库')).toBeVisible()
    expect(screen.getByRole('link', { name: /前往题库验收/ })).toHaveAttribute('href', '#/library')
    await waitFor(() => expect(eventState.unlisten).toHaveBeenCalledOnce())
    expect(syncController.scheduleMutation).toHaveBeenCalledOnce()
  })

  it('disables truncated candidates and safely rolls back a completed import', async () => {
    api.legacyScan.mockResolvedValue({ ok: true, data: {
      ...candidate, candidateId: 'candidate-truncated', report: { ...candidate.report, truncated: true },
    } })
    api.legacyImportList
      .mockResolvedValueOnce({ ok: true, data: [{
        importId: 'import-old', memberCount: 1, problemCount: 5, assetCount: 8,
        reviewCount: 4, status: 'completed', createdAtUtcMs: Date.now(), rolledBackAtUtcMs: null,
      }] })
      .mockResolvedValueOnce({
        ok: false,
        error: {
          code: 'legacy_history_refresh_failed', userMessage: '迁移记录没有刷新。',
          retryable: true, diagnosticId: 'diag-refresh',
        },
      })
    api.legacyRollback.mockResolvedValue({ status: 'ok', data: { ok: true, data: {
      importId: 'import-old', removedProblemCount: 5, removedProfileCount: 1,
      removedAssetCount: 7, preservedEntityCount: 1, rolledBackAtUtcMs: Date.now(),
    } } })
    renderPanel()

    await userEvent.click(screen.getByRole('button', { name: '选择旧版目录并扫描' }))
    expect(await screen.findByText(/扫描报告已截断/)).toBeVisible()
    expect(screen.getByRole('button', { name: '查看范围并确认导入' })).toBeDisabled()

    const rollback = await screen.findByRole('button', { name: '撤销这次导入' })
    await userEvent.click(rollback)
    await userEvent.click(screen.getByRole('checkbox', { name: /撤销本次导入/ }))
    await userEvent.click(screen.getByRole('button', { name: '确认撤销这次导入' }))
    expect(api.legacyRollback).toHaveBeenCalledWith('import-old')
    expect(await screen.findByText(/已移除 5 道仍归属于本次导入的题/)).toBeVisible()
    expect(screen.getByText(/保留了 1 项/)).toBeVisible()
    expect(await screen.findByRole('alert')).toHaveTextContent('迁移记录没有刷新。')
    expect(screen.getByRole('status')).toHaveTextContent('当前仍显示上一次成功读取的迁移记录。')
    expect(screen.getByText('5 道题 · 1 个档案')).toBeVisible()
    expect(syncController.scheduleMutation).toHaveBeenCalledOnce()
  })

  it('keeps the post-import history when the mount request resolves late', async () => {
    const initial = deferred<unknown>()
    const refreshed = deferred<unknown>()
    api.legacyImportList.mockReturnValueOnce(initial.promise).mockReturnValueOnce(refreshed.promise)
    api.legacyScan.mockResolvedValue({ status: 'ok', data: { ok: true, data: candidate } })
    api.legacyImport.mockResolvedValue({ status: 'ok', data: { ok: true, data: {
      importId: completedImport.importId,
      memberCount: completedImport.memberCount,
      problemCount: completedImport.problemCount,
      assetCount: completedImport.assetCount,
      reviewCount: completedImport.reviewCount,
      frozenProblemCount: 1,
      createdAtUtcMs: completedImport.createdAtUtcMs,
    } } })
    renderPanel()

    await userEvent.click(screen.getByRole('button', { name: '选择旧版目录并扫描' }))
    await userEvent.click(await screen.findByRole('button', { name: '查看范围并确认导入' }))
    await userEvent.click(screen.getByRole('checkbox', { name: /导入只复制数据/ }))
    await userEvent.click(screen.getByRole('button', { name: '确认并开始导入' }))
    await waitFor(() => expect(api.legacyImportList).toHaveBeenCalledTimes(2))
    await waitFor(() => expect(eventState.unlisten).toHaveBeenCalledOnce())
    expect(screen.getByRole('button', { name: '选择旧版目录并扫描' })).toBeEnabled()

    refreshed.resolve({ ok: true, data: [completedImport] })
    expect(await screen.findByText('2 道题 · 1 个档案')).toBeVisible()
    initial.resolve({ ok: true, data: [] })
    await Promise.resolve()
    await Promise.resolve()

    expect(screen.getByText('2 道题 · 1 个档案')).toBeVisible()
    expect(screen.queryByText('还没有完成过旧版迁移。')).not.toBeInTheDocument()
  })

  it('shows a retryable history error instead of a false empty state', async () => {
    api.legacyImportList
      .mockResolvedValueOnce({
        ok: false,
        error: {
          code: 'legacy_history_failed', userMessage: '迁移记录读取失败。',
          retryable: true, diagnosticId: 'diag-history',
        },
      })
      .mockResolvedValueOnce({ ok: true, data: [] })
    renderPanel()

    expect(await screen.findByRole('alert')).toHaveTextContent('迁移记录读取失败。')
    expect(screen.queryByText('还没有完成过旧版迁移。')).not.toBeInTheDocument()
    await userEvent.click(screen.getByRole('button', { name: '重新读取迁移记录' }))

    expect(await screen.findByText('还没有完成过旧版迁移。')).toBeVisible()
    expect(screen.queryByRole('alert')).not.toBeInTheDocument()
  })
})
