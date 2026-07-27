import { render, screen, waitFor } from '@testing-library/vue'
import userEvent from '@testing-library/user-event'
import { beforeEach, describe, expect, it, vi } from 'vitest'
import { syncControllerKey } from '../sync-controller'
import ReportView from './ReportView.vue'

const api = vi.hoisted(() => ({
  reportSummary: vi.fn(), exportList: vi.fn(), exportTrashList: vi.fn(), exportCandidates: vi.fn(), exportCreate: vi.fn(), exportGenerate: vi.fn(), exportDelete: vi.fn(), exportRestore: vi.fn(),
}))

vi.mock('@tauri-apps/api/core', () => ({ isTauri: () => true }))
vi.mock('../../shared/api/bindings', () => ({ commands: api }))

describe('ReportView', () => {
  const syncController = {
    run: vi.fn(),
    scheduleMutation: vi.fn(),
    dispose: vi.fn(),
  }
  const renderView = () => render(ReportView, {
    global: { provide: { [syncControllerKey as symbol]: syncController } },
  })
  const mathCandidate = {
    id: 'problem-1', subject: '数学', note: '圆锥曲线', questionAssetCount: 1,
    answerAssetCount: 1, dueAtUtcMs: null, reviewCount: 0,
  }

  beforeEach(() => {
    vi.clearAllMocks()
    api.reportSummary.mockResolvedValue({ ok: true, data: {
      activeProblemCount: 1, dueProblemCount: 1, reviewCount: 4, rememberedRate: 0.75,
      totalDurationMs: 120_000, currentStreakDays: 2,
      dailyActivity: [{ dayStartUtcMs: 1_700_000_000_000, reviewCount: 4, durationMs: 120_000 }],
      subjectActivity: [{ subject: '数学', problemCount: 1, reviewCount: 4 }],
    } })
    api.exportList.mockResolvedValue({ ok: true, data: [] })
    api.exportTrashList.mockResolvedValue({ ok: true, data: [] })
    api.exportCandidates.mockResolvedValue({ ok: true, data: [mathCandidate] })
    api.exportCreate.mockResolvedValue({ ok: true, data: {
      id: 'snapshot-1', title: '本周复盘', problemCount: 1, layout: 'question_answer_alternating', createdAtUtcMs: 1,
    } })
    api.exportDelete.mockResolvedValue({ ok: true, data: true })
    api.exportGenerate.mockResolvedValue({ ok: true, data: {
      snapshotId: 'snapshot-1', outputName: '本周复盘.docx', problemCount: 1, layout: 'question_answer_alternating',
    } })
    api.exportRestore.mockResolvedValue({ ok: true, data: true })
  })

  it('renders real metrics and creates an export snapshot from due candidates', async () => {
    const user = userEvent.setup()
    renderView()

    expect(await screen.findByText('75')).toBeVisible()
    expect(screen.getAllByText('数学').length).toBeGreaterThanOrEqual(1)
    const title = screen.getByRole('textbox', { name: '快照名称' })
    await user.clear(title)
    await user.type(title, '本周复盘')
    await user.click(screen.getByRole('button', { name: /保存 1 道题的导出快照/ }))

    await waitFor(() => expect(api.exportCreate).toHaveBeenCalledWith({
      title: '本周复盘', problemIds: ['problem-1'], layout: 'question_answer_alternating',
    }))
    expect(await screen.findByText('本周复盘')).toBeVisible()
    expect(syncController.scheduleMutation).toHaveBeenCalledOnce()
  })

  it('switches candidate source and saves only the explicitly selected problems', async () => {
    const user = userEvent.setup()
    const physicsCandidate = {
      id: 'problem-2', subject: '物理', note: '受力分析', questionAssetCount: 1,
      answerAssetCount: 1, dueAtUtcMs: 1_700_000_000_000, reviewCount: 2,
    }
    api.exportCandidates
      .mockResolvedValueOnce({ ok: true, data: [mathCandidate] })
      .mockResolvedValueOnce({ ok: true, data: [physicsCandidate, mathCandidate] })
    renderView()

    expect(await screen.findByRole('checkbox', { name: '选择数学：圆锥曲线' })).toBeChecked()
    await user.click(screen.getByRole('radio', { name: /最近训练批次/ }))
    await waitFor(() => expect(api.exportCandidates).toHaveBeenLastCalledWith('latest_review_session'))
    const physics = await screen.findByRole('checkbox', { name: '选择物理：受力分析' })
    expect(physics).toBeChecked()
    await user.click(physics)
    await user.click(screen.getByRole('button', { name: /保存 1 道题的导出快照/ }))

    await waitFor(() => expect(api.exportCreate).toHaveBeenCalledWith(expect.objectContaining({
      problemIds: ['problem-1'],
    })))
  })

  it('retries candidate loading without fabricating a selection', async () => {
    const user = userEvent.setup()
    api.exportCandidates
      .mockResolvedValueOnce({ ok: false, error: {
        code: 'export_candidates_failed', userMessage: '候选题读取失败。', retryable: true, diagnosticId: 'diag-1',
      } })
      .mockResolvedValueOnce({ ok: true, data: [mathCandidate] })
    renderView()

    expect(await screen.findByRole('alert')).toHaveTextContent('候选题读取失败。')
    expect(screen.getByRole('button', { name: /保存 0 道题的导出快照/ })).toBeDisabled()
    await user.click(screen.getByRole('button', { name: '重新读取候选题' }))

    expect(await screen.findByRole('checkbox', { name: '选择数学：圆锥曲线' })).toBeChecked()
    expect(api.exportCandidates).toHaveBeenCalledTimes(2)
  })

  it('keeps the explicit selection when snapshot creation fails', async () => {
    const user = userEvent.setup()
    api.exportCreate.mockResolvedValue({ ok: false, error: {
      code: 'export_create_failed', userMessage: '快照没有保存。', retryable: true, diagnosticId: 'diag-2',
    } })
    renderView()

    const candidate = await screen.findByRole('checkbox', { name: '选择数学：圆锥曲线' })
    await user.click(screen.getByRole('button', { name: /保存 1 道题的导出快照/ }))

    expect(await screen.findByRole('alert')).toHaveTextContent('快照没有保存。')
    expect(candidate).toBeChecked()
    expect(screen.getByRole('button', { name: /保存 1 道题的导出快照/ })).toBeEnabled()
    expect(syncController.scheduleMutation).not.toHaveBeenCalled()
  })

  it('loads the persistent recycle area and restores a deleted snapshot', async () => {
    const user = userEvent.setup()
    api.exportTrashList.mockResolvedValue({ ok: true, data: [{
      snapshot: {
        id: 'snapshot-deleted', title: '上月复盘', problemCount: 3,
        layout: 'questions_then_answers', createdAtUtcMs: 1_700_000_000_000,
      },
      deletedAtUtcMs: 1_700_100_000_000,
      purgeAfterUtcMs: 1_702_692_000_000,
    }] })
    renderView()

    expect(await screen.findByText('上月复盘')).toBeVisible()
    await user.click(screen.getByRole('button', { name: '恢复导出快照：上月复盘' }))

    await waitFor(() => expect(api.exportRestore).toHaveBeenCalledWith('snapshot-deleted'))
    expect(screen.queryByRole('button', { name: '恢复导出快照：上月复盘' })).not.toBeInTheDocument()
    expect(syncController.scheduleMutation).toHaveBeenCalledOnce()
  })

  it('generates a real file from a saved snapshot and reports only its safe file name', async () => {
    const user = userEvent.setup()
    api.exportList.mockResolvedValue({ ok: true, data: [{
      id: 'snapshot-1', title: '本周复盘', problemCount: 1,
      layout: 'question_answer_alternating', createdAtUtcMs: 1,
    }] })
    renderView()

    await user.click(await screen.findByRole('button', { name: '生成导出文件：本周复盘' }))

    await waitFor(() => expect(api.exportGenerate).toHaveBeenCalledWith('snapshot-1'))
    expect(screen.getByRole('status')).toHaveTextContent('已生成 本周复盘.docx，共 1 题。')
  })
})
