import { render, screen, waitFor } from '@testing-library/vue'
import userEvent from '@testing-library/user-event'
import { createMemoryHistory } from 'vue-router'
import { beforeEach, describe, expect, it, vi } from 'vitest'
import { createAppRouter } from '../router'
import { syncControllerKey } from '../sync-controller'
import LibraryView from './LibraryView.vue'

const api = vi.hoisted(() => ({
  libraryContext: vi.fn(),
  problemList: vi.fn(),
  problemDetail: vi.fn(),
  problemChangeStatus: vi.fn(),
  problemUpdate: vi.fn(),
  reviewManualStart: vi.fn(),
  reviewExamStart: vi.fn(),
}))

vi.mock('@tauri-apps/api/core', () => ({ isTauri: () => true }))
vi.mock('../../shared/api/bindings', () => ({ commands: api }))

const problems = [
  {
    id: 'problem-1', subject: '数学', note: '先看定义域。', status: 'active',
    questionAssetCount: 1, answerAssetCount: 1, questionPreviewDataUrl: null, updatedAtUtcMs: 1,
  },
  {
    id: 'problem-2', subject: '物理', note: '画出受力图。', status: 'active',
    questionAssetCount: 1, answerAssetCount: 1, questionPreviewDataUrl: null, updatedAtUtcMs: 2,
  },
]
const syncController = {
  run: vi.fn(),
  scheduleMutation: vi.fn(),
  dispose: vi.fn(),
}

async function renderView() {
  const router = createAppRouter(createMemoryHistory())
  await router.push('/library')
  await router.isReady()
  render(LibraryView, {
    global: {
      plugins: [router],
      provide: { [syncControllerKey as symbol]: syncController },
    },
  })
  await screen.findByText('先看定义域。')
  await userEvent.click(screen.getByRole('button', { name: '批量管理' }))
  return router
}

describe('LibraryView manual review deck', () => {
  beforeEach(() => {
    vi.clearAllMocks()
    api.libraryContext.mockResolvedValue({ ok: true, data: { profileName: '本机学习档案' } })
    api.problemList.mockResolvedValue({ ok: true, data: problems })
    api.reviewManualStart.mockResolvedValue({
      ok: true,
      data: {
        sessionId: 'manual-1', mode: 'manual', resumed: false,
        completedCount: 0, totalCount: 2, items: [],
      },
    })
    api.reviewExamStart.mockResolvedValue({
      ok: true,
      data: {
        sessionId: 'exam-1', mode: 'exam', resumed: false,
        completedCount: 0, totalCount: 2, examPhase: 'answering',
        examQuestionIndex: 0, examCorrectCount: 0, examWrongCount: 0, items: [],
      },
    })
    api.problemChangeStatus.mockResolvedValue({ ok: true, data: problems })
  })

  it('persists an ordered exam before routing and leaves ids out of the URL', async () => {
    const user = userEvent.setup()
    const router = await renderView()
    await user.click(screen.getByRole('checkbox', { name: '选择 物理 错题' }))
    await user.click(screen.getByRole('checkbox', { name: '选择 数学 错题' }))
    await user.click(screen.getByRole('button', { name: '模拟考试 2 道题' }))

    await waitFor(() => expect(api.reviewExamStart).toHaveBeenCalledWith({
      problemIds: ['problem-2', 'problem-1'],
    }))
    expect(api.reviewManualStart).not.toHaveBeenCalled()
    await waitFor(() => expect(router.currentRoute.value.name).toBe('review'))
    expect(router.currentRoute.value.query).toEqual({})
  })

  it('keeps the selected exam deck when persistence fails', async () => {
    const user = userEvent.setup()
    api.reviewExamStart.mockResolvedValue({
      ok: false,
      error: {
        code: 'review_exam_selection_invalid', userMessage: '所选题目已经变化，请重新选择。',
        retryable: false, diagnosticId: 'diag-exam',
      },
    })
    await renderView()
    await user.click(screen.getByRole('checkbox', { name: '选择 数学 错题' }))
    await user.click(screen.getByRole('button', { name: '模拟考试 1 道题' }))

    expect(await screen.findByRole('alert')).toHaveTextContent('所选题目已经变化，请重新选择。')
    expect(screen.getByRole('checkbox', { name: '选择 数学 错题' })).toBeChecked()
    expect(screen.getByRole('button', { name: '模拟考试 1 道题' })).toBeEnabled()
  })

  it('persists click order before routing without leaking ids into the URL', async () => {
    const user = userEvent.setup()
    const router = await renderView()
    await user.click(screen.getByRole('checkbox', { name: '选择 物理 错题' }))
    await user.click(screen.getByRole('checkbox', { name: '选择 数学 错题' }))
    await user.click(screen.getByRole('button', { name: '开始训练 2 道题' }))

    await waitFor(() => expect(api.reviewManualStart).toHaveBeenCalledWith({
      problemIds: ['problem-2', 'problem-1'],
    }))
    await waitFor(() => expect(router.currentRoute.value.name).toBe('review'))
    expect(router.currentRoute.value.query).toEqual({})
  })

  it('keeps the selection actionable when session creation fails', async () => {
    const user = userEvent.setup()
    api.reviewManualStart.mockResolvedValue({
      ok: false,
      error: {
        code: 'review_manual_selection_invalid', userMessage: '所选题目已经变化。',
        retryable: false, diagnosticId: 'diag-manual',
      },
    })
    await renderView()
    await user.click(screen.getByRole('checkbox', { name: '选择 数学 错题' }))
    await user.click(screen.getByRole('button', { name: '开始训练 1 道题' }))

    expect(await screen.findByRole('alert')).toHaveTextContent('所选题目已经变化。')
    expect(screen.getByRole('checkbox', { name: '选择 数学 错题' })).toBeChecked()
    expect(screen.getByRole('button', { name: '开始训练 1 道题' })).toBeEnabled()
  })

  it('schedules sync after a batch status change but not after a failed change', async () => {
    const user = userEvent.setup()
    await renderView()
    await user.click(screen.getByRole('checkbox', { name: '选择 数学 错题' }))
    await user.click(screen.getByRole('button', { name: '移入回收站' }))

    await waitFor(() => expect(syncController.scheduleMutation).toHaveBeenCalledOnce())

    api.problemChangeStatus.mockResolvedValueOnce({
      ok: false,
      error: {
        code: 'problem_update_failed',
        userMessage: '题目状态没有改变。',
        retryable: true,
        diagnosticId: 'problem-status',
      },
    })
    await user.click(screen.getByRole('checkbox', { name: '选择 数学 错题' }))
    await user.click(screen.getByRole('button', { name: '移入回收站' }))
    await screen.findByRole('alert')

    expect(syncController.scheduleMutation).toHaveBeenCalledOnce()
  })
})
