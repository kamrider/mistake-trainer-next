import { render, screen, waitFor } from '@testing-library/vue'
import userEvent from '@testing-library/user-event'
import { beforeEach, describe, expect, it, vi } from 'vitest'
import ReportView from './ReportView.vue'

const api = vi.hoisted(() => ({
  reportSummary: vi.fn(), exportList: vi.fn(), exportTrashList: vi.fn(), problemList: vi.fn(), exportCreate: vi.fn(), exportGenerate: vi.fn(), exportDelete: vi.fn(), exportRestore: vi.fn(),
}))

vi.mock('@tauri-apps/api/core', () => ({ isTauri: () => true }))
vi.mock('../../shared/api/bindings', () => ({ commands: api }))

describe('ReportView', () => {
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
    api.problemList.mockResolvedValue({ ok: true, data: [{
      id: 'problem-1', subject: '数学', note: '', status: 'active', questionAssetCount: 1, answerAssetCount: 1, updatedAtUtcMs: 1,
    }] })
    api.exportCreate.mockResolvedValue({ ok: true, data: {
      id: 'snapshot-1', title: '本周复盘', problemCount: 1, layout: 'question_answer_alternating', createdAtUtcMs: 1,
    } })
    api.exportDelete.mockResolvedValue({ ok: true, data: true })
    api.exportGenerate.mockResolvedValue({ ok: true, data: {
      snapshotId: 'snapshot-1', outputName: '本周复盘.docx', problemCount: 1, layout: 'question_answer_alternating',
    } })
    api.exportRestore.mockResolvedValue({ ok: true, data: true })
  })

  it('renders real metrics and creates an export snapshot from active problems', async () => {
    const user = userEvent.setup()
    render(ReportView)

    expect(await screen.findByText('75')).toBeVisible()
    expect(screen.getByText('数学')).toBeVisible()
    const title = screen.getByRole('textbox', { name: '快照名称' })
    await user.clear(title)
    await user.type(title, '本周复盘')
    await user.click(screen.getByRole('button', { name: /保存 1 道活动题配置/ }))

    await waitFor(() => expect(api.exportCreate).toHaveBeenCalledWith({
      title: '本周复盘', problemIds: ['problem-1'], layout: 'question_answer_alternating',
    }))
    expect(await screen.findByText('本周复盘')).toBeVisible()
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
    render(ReportView)

    expect(await screen.findByText('上月复盘')).toBeVisible()
    await user.click(screen.getByRole('button', { name: '恢复导出快照：上月复盘' }))

    await waitFor(() => expect(api.exportRestore).toHaveBeenCalledWith('snapshot-deleted'))
    expect(screen.queryByRole('button', { name: '恢复导出快照：上月复盘' })).not.toBeInTheDocument()
  })

  it('generates a real file from a saved snapshot and reports only its safe file name', async () => {
    const user = userEvent.setup()
    api.exportList.mockResolvedValue({ ok: true, data: [{
      id: 'snapshot-1', title: '本周复盘', problemCount: 1,
      layout: 'question_answer_alternating', createdAtUtcMs: 1,
    }] })
    render(ReportView)

    await user.click(await screen.findByRole('button', { name: '生成导出文件：本周复盘' }))

    await waitFor(() => expect(api.exportGenerate).toHaveBeenCalledWith('snapshot-1'))
    expect(screen.getByRole('status')).toHaveTextContent('已生成 本周复盘.docx，共 1 题。')
  })
})
