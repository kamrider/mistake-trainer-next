import { render, screen, waitFor } from '@testing-library/vue'
import userEvent from '@testing-library/user-event'
import { createMemoryHistory, RouterView } from 'vue-router'
import { beforeEach, describe, expect, it, vi } from 'vitest'
import { createAppRouter } from '../router'
import { syncControllerKey } from '../sync-controller'
import {
  createWorkspaceTransitionGuard,
  workspaceTransitionGuardKey,
  type WorkspaceTransitionGuard,
} from '../workspace-transition-guard'
import ReviewView from './ReviewView.vue'

const api = vi.hoisted(() => ({
  reviewQueue: vi.fn(),
  reviewCurrentProblem: vi.fn(),
  reviewSubmit: vi.fn(),
  reviewFocusSelect: vi.fn(),
  reviewFocusSkip: vi.fn(),
  reviewExamNavigate: vi.fn(),
  reviewExamBeginGrading: vi.fn(),
}))

vi.mock('@tauri-apps/api/core', () => ({ isTauri: () => true }))
vi.mock('../../shared/api/bindings', () => ({ commands: api }))

const queueOverview = {
  sessionId: 'session-1',
  mode: 'due',
  resumed: true,
  completedCount: 1,
  totalCount: 2,
  items: [{ problemId: 'problem-2', dueAtUtcMs: null, reviewCount: 0 }],
  examPhase: null,
  examQuestionIndex: 0,
  examCorrectCount: 0,
  examWrongCount: 0,
  focus: null,
}

const problemDetail = {
  id: 'problem-2',
  subject: '数学',
  note: '先独立写出关键步骤。',
  status: 'active',
  timeLimitSeconds: 60,
  updatedAtUtcMs: 1,
  assets: [],
}
const syncController = {
  run: vi.fn(),
  scheduleMutation: vi.fn(),
  dispose: vi.fn(),
}

async function renderView(workspaceTransitionGuard?: WorkspaceTransitionGuard) {
  const router = createAppRouter(createMemoryHistory())
  await router.push('/review')
  await router.isReady()
  const view = render(ReviewView, {
    global: {
      plugins: [router],
      provide: {
        [syncControllerKey as symbol]: syncController,
        ...(workspaceTransitionGuard
          ? { [workspaceTransitionGuardKey as symbol]: workspaceTransitionGuard }
          : {}),
      },
    },
  })
  return { router, view }
}

async function renderRoutedView() {
  const router = createAppRouter(createMemoryHistory())
  await router.push('/review')
  await router.isReady()
  render(RouterView, {
    global: {
      plugins: [router],
      provide: { [syncControllerKey as symbol]: syncController },
    },
  })
  return router
}

describe('ReviewView', () => {
  beforeEach(() => {
    vi.clearAllMocks()
    api.reviewQueue.mockResolvedValue({ ok: true, data: queueOverview })
    api.reviewCurrentProblem.mockResolvedValue({ ok: true, data: problemDetail })
    api.reviewSubmit.mockResolvedValue({ ok: true, data: {
      eventId: 'event-1', problemId: 'problem-2', rating: 'good', dueAtUtcMs: 2,
      stability: 1, difficulty: 5, algorithmVersion: 'fsrs-6.6.1', parameterVersion: 'default-6.6.1',
      focus: null,
    } })
    api.reviewFocusSelect.mockResolvedValue({ ok: true, data: null })
    api.reviewFocusSkip.mockResolvedValue({ ok: true, data: null })
  })

  it('uses persisted progress and submits a bounded stopped duration exactly once', async () => {
    const user = userEvent.setup()
    await renderView()

    expect(await screen.findByText('2 / 2')).toBeVisible()
    expect(api.reviewQueue).toHaveBeenCalledWith()
    expect(screen.getByText('已恢复上次进度')).toBeVisible()
    await user.click(screen.getByRole('button', { name: '显示答案' }))
    await user.click(screen.getByRole('button', { name: '记住了' }))

    await waitFor(() => expect(api.reviewSubmit).toHaveBeenCalledOnce())
    const [input] = api.reviewSubmit.mock.calls[0]!
    expect(input).toMatchObject({ problemId: 'problem-2', rating: 'good' })
    expect(input.durationMs).toBeGreaterThanOrEqual(0)
    expect(input.durationMs).toBeLessThanOrEqual(86_400_000)
    expect(await screen.findByRole('heading', { name: '把今天该记住的，认真看完了。' })).toBeVisible()
    expect(syncController.scheduleMutation).toHaveBeenCalledOnce()
  })

  it('blocks route changes until an in-flight rating has been persisted', async () => {
    const user = userEvent.setup()
    let resolveSubmit: (value: unknown) => void = () => undefined
    api.reviewSubmit.mockImplementationOnce(() => new Promise(resolve => { resolveSubmit = resolve }))
    const router = await renderRoutedView()

    await user.click(await screen.findByRole('button', { name: '显示答案' }))
    await user.click(screen.getByRole('button', { name: '记住了' }))
    await waitFor(() => expect(api.reviewSubmit).toHaveBeenCalledOnce())
    await router.push({ name: 'dashboard' })

    expect(router.currentRoute.value.name).toBe('review')
    expect(await screen.findByRole('alert')).toHaveTextContent('正在保存训练进度，请稍候再离开。')

    resolveSubmit({ ok: true, data: {
      eventId: 'event-pending', problemId: 'problem-2', rating: 'good', dueAtUtcMs: 2,
      stability: 1, difficulty: 5, algorithmVersion: 'fsrs-6.6.1', parameterVersion: 'default-6.6.1',
      focus: null,
    } })
    expect(await screen.findByRole('heading', { name: '把今天该记住的，认真看完了。' })).toBeVisible()

    await router.push({ name: 'dashboard' })
    expect(router.currentRoute.value.name).toBe('dashboard')
  })

  it('blocks workspace transitions while review progress is being persisted and unregisters on unmount', async () => {
    const user = userEvent.setup()
    let resolveSubmit: (value: unknown) => void = () => undefined
    api.reviewSubmit.mockImplementationOnce(() => new Promise(resolve => { resolveSubmit = resolve }))
    const workspaceTransitionGuard = createWorkspaceTransitionGuard()
    const { view } = await renderView(workspaceTransitionGuard)

    const idleUnload = new Event('beforeunload', { cancelable: true })
    window.dispatchEvent(idleUnload)
    expect(idleUnload.defaultPrevented).toBe(false)

    await user.click(await screen.findByRole('button', { name: '显示答案' }))
    await user.click(screen.getByRole('button', { name: '记住了' }))
    await waitFor(() => expect(api.reviewSubmit).toHaveBeenCalledOnce())

    await expect(workspaceTransitionGuard.attempt()).resolves.toBe(false)
    expect(await screen.findByRole('alert')).toHaveTextContent('正在保存训练进度，请稍候再离开。')
    expect(api.reviewSubmit).toHaveBeenCalledOnce()
    const busyUnload = new Event('beforeunload', { cancelable: true })
    window.dispatchEvent(busyUnload)
    expect(busyUnload.defaultPrevented).toBe(true)

    view.unmount()
    await expect(workspaceTransitionGuard.attempt()).resolves.toBe(true)
    const afterUnmount = new Event('beforeunload', { cancelable: true })
    window.dispatchEvent(afterUnmount)
    expect(afterUnmount.defaultPrevented).toBe(false)
    resolveSubmit({ ok: true, data: {
      eventId: 'event-pending', problemId: 'problem-2', rating: 'good', dueAtUtcMs: 2,
      stability: 1, difficulty: 5, algorithmVersion: 'fsrs-6.6.1', parameterVersion: 'default-6.6.1',
      focus: null,
    } })
  })

  it('retries a failed queue read without showing fake review content', async () => {
    const user = userEvent.setup()
    api.reviewQueue
      .mockResolvedValueOnce({ ok: false, error: { code: 'review_queue_failed', userMessage: '资料库暂时忙碌。', retryable: true, diagnosticId: 'diag-1' } })
      .mockResolvedValueOnce({ ok: true, data: queueOverview })
    await renderView()

    expect(await screen.findByRole('alert')).toHaveTextContent('资料库暂时忙碌。')
    expect(api.reviewCurrentProblem).not.toHaveBeenCalled()
    await user.click(screen.getByRole('button', { name: '重新读取训练' }))
    expect(await screen.findByText('2 / 2')).toBeVisible()
    expect(api.reviewQueue).toHaveBeenCalledTimes(2)
  })

  it('keeps the same card visible when rating persistence fails', async () => {
    const user = userEvent.setup()
    api.reviewSubmit.mockResolvedValue({ ok: false, error: {
      code: 'review_submit_failed', userMessage: '评分没有保存。', retryable: true, diagnosticId: 'diag-2',
    } })
    await renderView()

    await user.click(await screen.findByRole('button', { name: '显示答案' }))
    await user.click(screen.getByRole('button', { name: '记住了' }))
    expect(await screen.findByRole('alert')).toHaveTextContent('评分没有保存。')
    expect(screen.getByText('2 / 2')).toBeVisible()
    expect(screen.getByRole('button', { name: '记住了' })).toBeEnabled()
    expect(syncController.scheduleMutation).not.toHaveBeenCalled()
  })

  it('runs a persisted focus round before reading the next problem', async () => {
    const user = userEvent.setup()
    api.reviewQueue.mockResolvedValue({
      ok: true,
      data: {
        ...queueOverview,
        completedCount: 10,
        totalCount: 11,
        focus: {
          kind: 'break',
          roundIndex: 0,
          numbers: Array.from({ length: 25 }, (_, index) => index + 1),
          nextNumber: 1,
          elapsedMs: 600,
        },
      },
    })
    api.reviewFocusSelect.mockResolvedValue({
      ok: true,
      data: {
        kind: 'break', roundIndex: 0,
        numbers: Array.from({ length: 25 }, (_, index) => index + 1),
        nextNumber: 2, elapsedMs: 900,
      },
    })
    await renderView()

    expect(await screen.findByRole('heading', { name: '让眼睛换一条路' })).toBeVisible()
    expect(api.reviewCurrentProblem).not.toHaveBeenCalled()
    await user.click(screen.getByRole('button', { name: '数字 2' }))
    expect(api.reviewFocusSelect).not.toHaveBeenCalled()
    expect(screen.getByText('请先找到 1')).toBeVisible()
    await user.click(screen.getByRole('button', { name: '数字 1' }))
    await waitFor(() => expect(api.reviewFocusSelect).toHaveBeenCalledOnce())
    expect(api.reviewFocusSelect.mock.calls[0]![0]).toMatchObject({ number: 1 })
    expect(api.reviewFocusSelect.mock.calls[0]![0].elapsedMs).toBeGreaterThanOrEqual(600)
    expect(await screen.findByText('下一位 2')).toBeVisible()

    await user.click(screen.getByRole('button', { name: '跳过，继续训练' }))
    await waitFor(() => expect(api.reviewFocusSkip).toHaveBeenCalledOnce())
    expect(await screen.findByText('11 / 11')).toBeVisible()
    expect(api.reviewCurrentProblem).toHaveBeenCalledOnce()
  })

  it('shows the transactional focus break after rating without reading the next card early', async () => {
    const user = userEvent.setup()
    api.reviewQueue.mockResolvedValue({
      ok: true,
      data: {
        ...queueOverview,
        completedCount: 9,
        totalCount: 11,
        focus: null,
        items: [
          { problemId: 'problem-2', dueAtUtcMs: null, reviewCount: 0 },
          { problemId: 'problem-3', dueAtUtcMs: null, reviewCount: 0 },
        ],
      },
    })
    api.reviewSubmit.mockResolvedValue({ ok: true, data: {
      eventId: 'event-10', problemId: 'problem-2', rating: 'good', dueAtUtcMs: 2,
      stability: 1, difficulty: 5, algorithmVersion: 'fsrs-6.6.1', parameterVersion: 'default-6.6.1',
      focus: {
        kind: 'break', roundIndex: 0,
        numbers: Array.from({ length: 25 }, (_, index) => 25 - index),
        nextNumber: 1, elapsedMs: 0,
      },
    } })
    await renderView()

    await user.click(await screen.findByRole('button', { name: '显示答案' }))
    await user.click(screen.getByRole('button', { name: '记住了' }))

    expect(await screen.findByRole('heading', { name: '让眼睛换一条路' })).toBeVisible()
    expect(api.reviewCurrentProblem).toHaveBeenCalledOnce()
  })

  it('reloads authoritative focus progress after a stale selection', async () => {
    const user = userEvent.setup()
    const initialFocus = {
      kind: 'break', roundIndex: 0,
      numbers: Array.from({ length: 25 }, (_, index) => index + 1),
      nextNumber: 1, elapsedMs: 400,
    }
    api.reviewQueue
      .mockResolvedValueOnce({ ok: true, data: { ...queueOverview, focus: initialFocus } })
      .mockResolvedValueOnce({ ok: true, data: {
        ...queueOverview,
        focus: { ...initialFocus, nextNumber: 2, elapsedMs: 900 },
      } })
    api.reviewFocusSelect.mockResolvedValue({ ok: false, error: {
      code: 'review_focus_state_changed', userMessage: '专注进度已经变化。',
      retryable: false, diagnosticId: 'diag-stale-focus',
    } })
    await renderView()

    await user.click(await screen.findByRole('button', { name: '数字 1' }))

    await waitFor(() => expect(api.reviewQueue).toHaveBeenCalledTimes(2))
    expect(await screen.findByText('下一位 2')).toBeVisible()
    expect(screen.getByRole('alert')).toHaveTextContent('已恢复到最新位置')
    expect(api.reviewCurrentProblem).not.toHaveBeenCalled()
  })

  it('uses manual-deck copy through completion and never reads ids from the route', async () => {
    const user = userEvent.setup()
    api.reviewQueue.mockResolvedValue({
      ok: true,
      data: { ...queueOverview, mode: 'manual', resumed: false, completedCount: 0, totalCount: 1 },
    })
    await renderView()

    expect(await screen.findByText('自选训练')).toBeVisible()
    await user.click(screen.getByRole('button', { name: '显示答案' }))
    await user.click(screen.getByRole('button', { name: '记住了' }))
    expect(await screen.findByRole('heading', { name: '这组自选卡已经练完。' })).toBeVisible()
    expect(api.reviewQueue).toHaveBeenCalledWith()
  })

  it('restores and persists exam navigation before changing the visible card', async () => {
    const user = userEvent.setup()
    const exam = {
      sessionId: 'exam-1', mode: 'exam', resumed: true, completedCount: 0, totalCount: 2,
      examPhase: 'answering', examQuestionIndex: 1, examCorrectCount: 0, examWrongCount: 0,
      items: [
        { problemId: 'exam-a', dueAtUtcMs: null, reviewCount: 0 },
        { problemId: 'exam-b', dueAtUtcMs: null, reviewCount: 0 },
      ],
    }
    api.reviewQueue.mockResolvedValue({ ok: true, data: exam })
    api.reviewCurrentProblem
      .mockResolvedValueOnce({ ok: true, data: { ...problemDetail, id: 'exam-b', note: '第二题' } })
      .mockResolvedValueOnce({ ok: true, data: { ...problemDetail, id: 'exam-a', note: '第一题' } })
    api.reviewExamNavigate.mockResolvedValue({
      ok: true,
      data: { ...exam, resumed: false, examQuestionIndex: 0 },
    })
    await renderView()

    expect(await screen.findByText('第二题')).toBeVisible()
    expect(screen.getByText('2 / 2')).toBeVisible()
    expect(screen.getByText('已恢复上次进度')).toBeVisible()
    await user.click(screen.getByRole('button', { name: '上一题' }))
    await waitFor(() => expect(api.reviewExamNavigate).toHaveBeenCalledWith({ position: 0 }))
    expect(await screen.findByText('第一题')).toBeVisible()
  })

  it('moves from answer-secret exam to grading and reports a persisted score', async () => {
    const user = userEvent.setup()
    const answering = {
      sessionId: 'exam-2', mode: 'exam', resumed: false, completedCount: 0, totalCount: 1,
      examPhase: 'answering', examQuestionIndex: 0, examCorrectCount: 0, examWrongCount: 0,
      items: [{ problemId: 'problem-2', dueAtUtcMs: null, reviewCount: 0 }],
    }
    api.reviewQueue.mockResolvedValue({ ok: true, data: answering })
    api.reviewExamBeginGrading.mockResolvedValue({
      ok: true,
      data: { ...answering, examPhase: 'grading' },
    })
    await renderView()

    await user.click(await screen.findByRole('button', { name: '开始核对答案' }))
    await waitFor(() => expect(api.reviewExamBeginGrading).toHaveBeenCalledOnce())
    expect(screen.getByText('请对照答案图片，确认关键步骤和易错点。')).toBeVisible()
    await user.click(screen.getByRole('button', { name: '答对' }))

    expect(await screen.findByRole('heading', { name: '这场模拟考试已经核对完。' })).toBeVisible()
    expect(screen.getByText(/答对 1 道 · 答错 0 道 · 正确率 100%/)).toBeVisible()
  })
})
