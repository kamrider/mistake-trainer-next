import { render, screen, waitFor, within } from '@testing-library/vue'
import userEvent from '@testing-library/user-event'
import { createMemoryHistory, RouterView } from 'vue-router'
import { beforeEach, describe, expect, it, vi } from 'vitest'
import { createAppRouter } from '../router'
import { syncControllerKey } from '../sync-controller'
import {
  createWorkspaceTransitionGuard,
  workspaceTransitionGuardKey,
} from '../workspace-transition-guard'
import LibraryView from './LibraryView.vue'
import type { ProblemSummary } from '../../shared/api/bindings'

const api = vi.hoisted(() => ({
  libraryContext: vi.fn(),
  problemFilterOptions: vi.fn(),
  problemList: vi.fn(),
  problemDetail: vi.fn(),
  problemChangeStatus: vi.fn(),
  problemBulkMetadata: vi.fn(),
  problemUpdate: vi.fn(),
  reviewManualStart: vi.fn(),
  reviewExamStart: vi.fn(),
}))

vi.mock('@tauri-apps/api/core', () => ({ isTauri: () => true }))
vi.mock('../../shared/api/bindings', () => ({ commands: api }))

const problems: ProblemSummary[] = [
  {
    id: 'problem-1', subject: '数学', note: '先看定义域。', status: 'active',
    tags: [],
    questionAssetCount: 1, answerAssetCount: 1, questionPreviewDataUrl: null, updatedAtUtcMs: 1,
  },
  {
    id: 'problem-2', subject: '物理', note: '画出受力图。', status: 'active',
    tags: [],
    questionAssetCount: 1, answerAssetCount: 1, questionPreviewDataUrl: null, updatedAtUtcMs: 2,
  },
]
const problemDetail = {
  id: 'problem-1', subject: '数学', note: '先看定义域。', tags: [], status: 'active',
  timeLimitSeconds: null, updatedAtUtcMs: 1, assets: [],
}
const syncController = {
  run: vi.fn(),
  scheduleMutation: vi.fn(),
  dispose: vi.fn(),
}

function problemPage(items: ProblemSummary[] = problems, nextCursor: string | null = null) {
  return { items, nextCursor }
}

function deferred<T>() {
  let resolve!: (value: T) => void
  const promise = new Promise<T>(finish => { resolve = finish })
  return { promise, resolve }
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

async function renderRoutedView() {
  const router = createAppRouter(createMemoryHistory())
  await router.push('/library')
  await router.isReady()
  render(RouterView, {
    global: {
      plugins: [router],
      provide: { [syncControllerKey as symbol]: syncController },
    },
  })
  await screen.findByText('先看定义域。')
  await userEvent.click(screen.getByRole('button', { name: '批量管理' }))
  return router
}

async function renderGuardedRoutedView() {
  const router = createAppRouter(createMemoryHistory())
  const workspaceTransitionGuard = createWorkspaceTransitionGuard()
  await router.push('/library')
  await router.isReady()
  const view = render(RouterView, {
    global: {
      plugins: [router],
      provide: {
        [syncControllerKey as symbol]: syncController,
        [workspaceTransitionGuardKey as symbol]: workspaceTransitionGuard,
      },
    },
  })
  await screen.findByText('先看定义域。')
  await userEvent.click(screen.getByRole('button', { name: '批量管理' }))
  return { router, view, workspaceTransitionGuard }
}

describe('LibraryView manual review deck', () => {
  beforeEach(() => {
    vi.clearAllMocks()
    api.libraryContext.mockResolvedValue({
      ok: true,
      data: { profileId: 'profile-1', profileName: '本机学习档案', storage: 'ready' },
    })
    api.problemFilterOptions.mockResolvedValue({
      ok: true,
      data: { subjects: ['数学', '物理'], tags: [] },
    })
    api.problemList.mockResolvedValue({ ok: true, data: problemPage() })
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
    api.problemBulkMetadata.mockResolvedValue({ ok: true, data: { updatedCount: 1 } })
    api.problemDetail.mockResolvedValue({ ok: true, data: problemDetail })
    api.problemUpdate.mockResolvedValue({ ok: true, data: true })
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

  it('keeps the library active until session creation finishes and then navigates once', async () => {
    const user = userEvent.setup()
    let resolveManualStart: (value: unknown) => void = () => undefined
    api.reviewManualStart.mockImplementationOnce(() => new Promise(resolve => { resolveManualStart = resolve }))
    const router = await renderRoutedView()
    await user.click(screen.getByRole('checkbox', { name: '选择 数学 错题' }))
    await user.click(screen.getByRole('button', { name: '开始训练 1 道题' }))
    await waitFor(() => expect(api.reviewManualStart).toHaveBeenCalledOnce())

    await router.push({ name: 'dashboard' })
    expect(router.currentRoute.value.name).toBe('library')
    expect(await screen.findByRole('alert')).toHaveTextContent('题库操作正在进行，请等待完成后再离开。')
    resolveManualStart({
      ok: true,
      data: {
        sessionId: 'manual-late', mode: 'manual', resumed: false,
        completedCount: 0, totalCount: 1, items: [],
      },
    })

    await waitFor(() => expect(router.currentRoute.value.name).toBe('review'))
  })

  it('shows a blocked profile transition inside the active problem detail', async () => {
    const user = userEvent.setup()
    let resolveManualStart: (value: unknown) => void = () => undefined
    api.reviewManualStart.mockImplementationOnce(() => new Promise(resolve => { resolveManualStart = resolve }))
    const { router, workspaceTransitionGuard } = await renderGuardedRoutedView()

    await user.click(screen.getByRole('button', { name: '打开 数学 错题详情' }))
    const drawer = await screen.findByRole('dialog', { name: '数学' })
    await user.click(within(drawer).getByRole('button', { name: '用这道题开始训练' }))
    await waitFor(() => expect(api.reviewManualStart).toHaveBeenCalledOnce())

    await expect(workspaceTransitionGuard.attempt()).resolves.toBe(false)
    expect(within(drawer).getByRole('alert')).toHaveTextContent('题库操作正在进行，请等待完成后再离开。')

    resolveManualStart({
      ok: true,
      data: {
        sessionId: 'manual-detail', mode: 'manual', resumed: false,
        completedCount: 0, totalCount: 1, items: [],
      },
    })
    await waitFor(() => expect(router.currentRoute.value.name).toBe('review'))
  })

  it('confirms before leaving a dirty problem detail and preserves its draft on cancel', async () => {
    const user = userEvent.setup()
    const router = await renderRoutedView()
    await user.click(screen.getByRole('button', { name: '打开 数学 错题详情' }))
    await user.click(await screen.findByRole('button', { name: '编辑题目' }))
    const subject = screen.getByRole('textbox', { name: '科目' })
    await user.type(subject, '竞赛')

    const cancelledNavigation = router.push({ name: 'dashboard' })
    expect(await screen.findByRole('alertdialog', { name: '放弃尚未保存的修改？' })).toBeVisible()
    await waitFor(() => expect(screen.getByRole('button', { name: '继续编辑' })).toHaveFocus())
    await user.click(screen.getByRole('button', { name: '继续编辑' }))
    await cancelledNavigation
    expect(router.currentRoute.value.name).toBe('library')
    expect(subject).toHaveValue('数学竞赛')

    const confirmedNavigation = router.push({ name: 'dashboard' })
    await user.click(await screen.findByRole('button', { name: '放弃修改' }))
    await confirmedNavigation
    expect(router.currentRoute.value.name).toBe('dashboard')
  })

  it('allows same-library query changes but blocks dirty navigation while saving', async () => {
    const user = userEvent.setup()
    let resolveUpdate: (value: unknown) => void = () => undefined
    api.problemUpdate.mockImplementationOnce(() => new Promise(resolve => { resolveUpdate = resolve }))
    const router = await renderRoutedView()
    await user.click(screen.getByRole('button', { name: '打开 数学 错题详情' }))
    await user.click(await screen.findByRole('button', { name: '编辑题目' }))
    await user.type(screen.getByRole('textbox', { name: '科目' }), '竞赛')

    await router.push({ name: 'library', query: { section: 'active' } })
    expect(router.currentRoute.value.query).toEqual({ section: 'active' })
    expect(screen.queryByRole('alertdialog')).not.toBeInTheDocument()
    expect(screen.getByRole('textbox', { name: '科目' })).toHaveValue('数学竞赛')

    await user.click(screen.getByRole('button', { name: '保存修改' }))
    await waitFor(() => expect(api.problemUpdate).toHaveBeenCalledOnce())
    await router.push({ name: 'dashboard' })
    expect(router.currentRoute.value.name).toBe('library')
    expect(await screen.findByRole('alert')).toHaveTextContent('题目操作正在完成，请等待完成后再离开。')
    expect(screen.queryByRole('alertdialog')).not.toBeInTheDocument()

    resolveUpdate({ ok: true, data: true })
    await waitFor(() => expect(screen.getByRole('button', { name: '编辑题目' })).toBeEnabled())
    await router.push({ name: 'dashboard' })
    expect(router.currentRoute.value.name).toBe('dashboard')
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

  it('blocks route, workspace, and window transitions during a batch status change', async () => {
    const user = userEvent.setup()
    let resolveStatus: (value: unknown) => void = () => undefined
    api.problemChangeStatus.mockImplementationOnce(() => new Promise(resolve => { resolveStatus = resolve }))
    const { router, view, workspaceTransitionGuard } = await renderGuardedRoutedView()

    await user.click(screen.getByRole('checkbox', { name: '选择 数学 错题' }))
    await user.click(screen.getByRole('button', { name: '移入回收站' }))
    await waitFor(() => expect(api.problemChangeStatus).toHaveBeenCalledOnce())

    await expect(workspaceTransitionGuard.attempt()).resolves.toBe(false)
    expect(await screen.findByRole('alert')).toHaveTextContent('题库操作正在进行，请等待完成后再离开。')
    const busyUnload = new Event('beforeunload', { cancelable: true })
    window.dispatchEvent(busyUnload)
    expect(busyUnload.defaultPrevented).toBe(true)
    await router.push({ name: 'dashboard' })
    expect(router.currentRoute.value.name).toBe('library')

    resolveStatus({ ok: true, data: problems })
    await waitFor(() => expect(syncController.scheduleMutation).toHaveBeenCalledOnce())
    await waitFor(() => expect(api.problemList).toHaveBeenCalledTimes(2))
    await waitFor(() =>
      expect(screen.queryByRole('button', { name: '正在移入回收站…' })).not.toBeInTheDocument())
    await waitFor(async () =>
      expect(await workspaceTransitionGuard.attempt()).toBe(true))
    expect(screen.queryByText('题库操作正在进行，请等待完成后再离开。')).not.toBeInTheDocument()
    const idleUnload = new Event('beforeunload', { cancelable: true })
    window.dispatchEvent(idleUnload)
    expect(idleUnload.defaultPrevented).toBe(false)
    await router.push({ name: 'dashboard' })
    expect(router.currentRoute.value.name).toBe('dashboard')

    view.unmount()
  })

  it('exposes one in-flight batch status transaction and prevents duplicate commands', async () => {
    const user = userEvent.setup()
    let resolveStatus: (value: unknown) => void = () => undefined
    api.problemChangeStatus.mockImplementationOnce(() => new Promise(resolve => { resolveStatus = resolve }))
    await renderView()
    await user.click(screen.getByRole('checkbox', { name: '选择 数学 错题' }))

    await user.click(screen.getByRole('button', { name: '移入回收站' }))
    const pendingAction = await screen.findByRole('button', { name: '正在移入回收站…' })
    expect(pendingAction).toBeDisabled()
    await user.click(pendingAction)

    expect(api.problemChangeStatus).toHaveBeenCalledOnce()
    expect(api.problemChangeStatus).toHaveBeenCalledWith({
      problemIds: ['problem-1'],
      targetStatus: 'trashed',
    })
    resolveStatus({ ok: true, data: 1 })
    await waitFor(() => expect(syncController.scheduleMutation).toHaveBeenCalledOnce())
    await waitFor(() => expect(api.problemList).toHaveBeenCalledTimes(2))
  })

  it('keeps the detail open until its update finishes and then allows closing', async () => {
    const user = userEvent.setup()
    let resolveUpdate: (value: unknown) => void = () => undefined
    api.problemUpdate.mockImplementationOnce(() => new Promise(resolve => { resolveUpdate = resolve }))
    await renderView()
    await user.click(screen.getByRole('button', { name: '打开 数学 错题详情' }))
    expect(await screen.findByRole('dialog', { name: '数学' })).toBeVisible()
    await user.click(screen.getByRole('button', { name: '编辑题目' }))
    await user.click(screen.getByRole('button', { name: '保存修改' }))
    await waitFor(() => expect(api.problemUpdate).toHaveBeenCalledWith({
      problemId: 'problem-1', subject: '数学', note: '先看定义域。', tags: [], timeLimitSeconds: null,
    }))
    await user.click(screen.getByRole('button', { name: '关闭题目详情' }))
    expect(screen.getByRole('dialog', { name: '数学' })).toBeVisible()
    expect(screen.getByRole('alert')).toHaveTextContent('题目操作正在完成，请等待完成后再离开。')

    resolveUpdate({ ok: true, data: true })
    await waitFor(() => expect(api.problemList).toHaveBeenCalledTimes(2))
    await waitFor(() => expect(screen.getByRole('button', { name: '编辑题目' })).toBeEnabled())
    await user.click(screen.getByRole('button', { name: '关闭题目详情' }))
    expect(screen.queryByRole('dialog')).not.toBeInTheDocument()
    expect(api.problemDetail).toHaveBeenCalledTimes(2)
    expect(syncController.scheduleMutation).toHaveBeenCalledOnce()
  })

  it('keeps the current detail visible while reloading the same problem after save', async () => {
    const user = userEvent.setup()
    let resolveDetailReload: (value: unknown) => void = () => undefined
    api.problemDetail
      .mockResolvedValueOnce({ ok: true, data: problemDetail })
      .mockImplementationOnce(() => new Promise(resolve => { resolveDetailReload = resolve }))
    await renderView()
    await user.click(screen.getByRole('button', { name: '打开 数学 错题详情' }))
    await user.click(await screen.findByRole('button', { name: '编辑题目' }))
    await user.type(screen.getByRole('textbox', { name: '科目' }), '竞赛')
    await user.click(screen.getByRole('button', { name: '保存修改' }))
    await waitFor(() => expect(api.problemDetail).toHaveBeenCalledTimes(2))

    expect(screen.getByRole('heading', { name: '数学竞赛' })).toBeVisible()
    expect(screen.getByRole('button', { name: '编辑题目' })).toBeVisible()

    resolveDetailReload({
      ok: true,
      data: { ...problemDetail, subject: '数学竞赛', updatedAtUtcMs: 2 },
    })
    expect(await screen.findByRole('button', { name: '编辑题目' })).toBeVisible()
  })

  it('keeps the saved detail usable when its same-problem reload fails', async () => {
    const user = userEvent.setup()
    api.problemDetail
      .mockResolvedValueOnce({ ok: true, data: problemDetail })
      .mockResolvedValueOnce({
        ok: false,
        error: {
          code: 'problem_detail_failed',
          userMessage: '详情刷新失败，请重试。',
          retryable: true,
          diagnosticId: 'detail-reload',
        },
      })
    await renderView()
    await user.click(screen.getByRole('button', { name: '打开 数学 错题详情' }))
    await user.click(await screen.findByRole('button', { name: '编辑题目' }))
    await user.type(screen.getByRole('textbox', { name: '科目' }), '竞赛')
    await user.click(screen.getByRole('button', { name: '保存修改' }))

    expect(await screen.findByRole('alert')).toHaveTextContent('详情刷新失败，请重试。')
    expect(screen.getByRole('heading', { name: '数学竞赛' })).toBeVisible()
    expect(screen.getByRole('button', { name: '编辑题目' })).toBeEnabled()
  })

  it('keeps advanced filters after closing detail and refetches with typed input', async () => {
    const user = userEvent.setup()
    await renderView()
    await user.click(screen.getByRole('button', { name: '更多筛选' }))
    await user.click(screen.getByRole('checkbox', { name: '数学' }))

    await waitFor(() => expect(api.problemList).toHaveBeenLastCalledWith({
      status: 'active',
      search: null,
      subjects: ['数学'],
      tags: [],
      reviewState: 'any',
      answerState: 'any',
      cursor: null,
    }))
    await user.click(screen.getByRole('button', { name: '打开 数学 错题详情' }))
    await user.click(await screen.findByRole('button', { name: '关闭题目详情' }))
    expect(screen.getByRole('button', { name: '移除科目 数学' })).toBeVisible()
    expect(api.problemFilterOptions).toHaveBeenCalledTimes(1)
  })

  it('persists one normalized bulk metadata command before refresh and sync', async () => {
    const user = userEvent.setup()
    await renderView()
    await user.click(screen.getByRole('checkbox', { name: '选择 数学 错题' }))
    await user.click(screen.getByRole('button', { name: '批量修改' }))
    expect(screen.getByRole('dialog', { name: '修改已选 1 道题' })).toBeVisible()
    await user.type(screen.getByRole('textbox', { name: '添加标签' }), '重点，重点')
    await user.click(screen.getByRole('button', { name: '确认批量修改' }))

    await waitFor(() => expect(api.problemBulkMetadata).toHaveBeenCalledWith({
      problemIds: ['problem-1'],
      subject: null,
      addTags: ['重点'],
      removeTags: [],
    }))
    await waitFor(() => expect(api.problemList).toHaveBeenCalledTimes(2))
    expect(syncController.scheduleMutation).toHaveBeenCalledOnce()
    expect(screen.queryByRole('dialog', { name: '修改已选 1 道题' })).not.toBeInTheDocument()
  })

  it('hydrates a serializable report deep-link into the typed library filter', async () => {
    const router = createAppRouter(createMemoryHistory())
    await router.push({ name: 'library', query: { tag: '错因·计算失误' } })
    await router.isReady()
    render(LibraryView, {
      global: {
        plugins: [router],
        provide: { [syncControllerKey as symbol]: syncController },
      },
    })

    await screen.findByText('先看定义域。')
    expect(api.problemList).toHaveBeenCalledWith({
      status: 'active',
      search: null,
      subjects: [],
      tags: ['错因·计算失误'],
      reviewState: 'any',
      answerState: 'any',
      cursor: null,
    })
    expect(screen.getByRole('button', { name: '移除标签 错因·计算失误' })).toBeVisible()
  })

  it('offers subjects and tags that are outside the first paginated page', async () => {
    const firstPage = Array.from({ length: 40 }, (_, index) => ({
      id: `problem-${index}`,
      subject: '数学',
      note: `第 ${index + 1} 道`,
      tags: ['基础'],
      status: 'active',
      questionAssetCount: 1,
      answerAssetCount: 1,
      questionPreviewDataUrl: null,
      updatedAtUtcMs: 100 - index,
    }))
    api.problemList.mockResolvedValue({ ok: true, data: problemPage(firstPage, 'page-2') })
    api.problemFilterOptions.mockResolvedValue({
      ok: true,
      data: { subjects: ['数学', '化学'], tags: ['基础', '压轴'] },
    })

    const router = createAppRouter(createMemoryHistory())
    await router.push('/library')
    await router.isReady()
    render(LibraryView, {
      global: { plugins: [router], provide: { [syncControllerKey as symbol]: syncController } },
    })

    expect(await screen.findAllByRole('article')).toHaveLength(40)
    await userEvent.click(screen.getByRole('button', { name: '更多筛选' }))
    expect(screen.getByRole('checkbox', { name: '化学' })).toBeVisible()
    expect(screen.getByRole('checkbox', { name: '压轴' })).toBeVisible()
    expect(api.problemFilterOptions).toHaveBeenCalledWith('active')
  })

  it('does not reuse another status options when the requested facets fail', async () => {
    api.problemFilterOptions.mockReset()
      .mockResolvedValueOnce({
        ok: true,
        data: { subjects: ['数学'], tags: ['基础'] },
      })
      .mockResolvedValueOnce({
        ok: false,
        error: {
          code: 'problem_filter_options_failed',
          userMessage: '筛选选项暂时无法读取。',
          retryable: true,
          diagnosticId: 'filter-options',
        },
      })

    await renderView()
    await userEvent.click(screen.getByRole('button', { name: '已归档' }))
    expect(await screen.findByRole('alert')).toHaveTextContent('筛选选项暂时无法读取。')
    await userEvent.click(screen.getByRole('button', { name: '更多筛选' }))
    expect(screen.queryByRole('checkbox', { name: '数学' })).not.toBeInTheDocument()
    expect(api.problemFilterOptions).toHaveBeenNthCalledWith(1, 'active')
    expect(api.problemFilterOptions).toHaveBeenNthCalledWith(2, 'archived')
  })

  it('ignores an old profile facet response after the new profile is visible', async () => {
    const oldProfileOptions = deferred<unknown>()
    api.libraryContext.mockReset()
      .mockResolvedValueOnce({
        ok: true,
        data: { profileId: 'profile-a', profileName: '档案 A', storage: 'ready' },
      })
      .mockResolvedValue({
        ok: true,
        data: { profileId: 'profile-b', profileName: '档案 B', storage: 'ready' },
      })
    api.problemFilterOptions.mockReset()
      .mockReturnValueOnce(oldProfileOptions.promise)
      .mockResolvedValue({
        ok: true,
        data: { subjects: ['生物'], tags: ['新档案'] },
      })

    const router = createAppRouter(createMemoryHistory())
    await router.push('/library')
    await router.isReady()
    render(LibraryView, {
      global: { plugins: [router], provide: { [syncControllerKey as symbol]: syncController } },
    })
    await waitFor(() => expect(api.problemFilterOptions).toHaveBeenCalledTimes(1))
    await userEvent.type(screen.getByRole('searchbox', { name: '搜索题库' }), '函数')
    await screen.findByText('先看定义域。')
    await userEvent.click(screen.getByRole('button', { name: '更多筛选' }))
    expect(screen.getByRole('checkbox', { name: '生物' })).toBeVisible()

    oldProfileOptions.resolve({
      ok: true,
      data: { subjects: ['数学'], tags: ['旧档案'] },
    })
    await new Promise(resolve => window.setTimeout(resolve, 0))

    expect(screen.getByRole('checkbox', { name: '生物' })).toBeVisible()
    expect(screen.queryByRole('checkbox', { name: '数学' })).not.toBeInTheDocument()
  })

  it('appends one server page and admits only one concurrent load-more request', async () => {
    const firstPage = Array.from({ length: 40 }, (_, index) => ({
      id: `problem-${index}`,
      subject: index === 0 ? '数学' : `科目${index}`,
      note: `第 ${index + 1} 道`,
      tags: [],
      status: 'active',
      questionAssetCount: 1,
      answerAssetCount: 1,
      questionPreviewDataUrl: null,
      updatedAtUtcMs: 100 - index,
    }))
    const append = deferred<unknown>()
    api.problemList.mockReset()
      .mockResolvedValueOnce({ ok: true, data: problemPage(firstPage, 'page-2') })
      .mockReturnValueOnce(append.promise)

    const router = createAppRouter(createMemoryHistory())
    await router.push('/library')
    await router.isReady()
    render(LibraryView, { global: { plugins: [router], provide: { [syncControllerKey as symbol]: syncController } } })

    expect(await screen.findAllByRole('article')).toHaveLength(40)
    const loadMore = screen.getByRole('button', { name: '加载更多' })
    loadMore.click()
    loadMore.click()
    await waitFor(() => expect(api.problemList).toHaveBeenCalledTimes(2))
    expect(api.problemList).toHaveBeenLastCalledWith(expect.objectContaining({ cursor: 'page-2' }))

    append.resolve({ ok: true, data: problemPage([{
      id: 'problem-40', subject: '英语', note: '第 41 道', tags: [], status: 'active',
      questionAssetCount: 1, answerAssetCount: 1, questionPreviewDataUrl: null, updatedAtUtcMs: 60,
    }]) })
    expect(await screen.findByText('第 41 道')).toBeVisible()
    expect(screen.getAllByRole('article')).toHaveLength(41)
    expect(screen.queryByRole('button', { name: '加载更多' })).not.toBeInTheDocument()
  })

  it('keeps visible cards and selection after an append failure and allows retry', async () => {
    api.problemList.mockReset()
      .mockResolvedValueOnce({ ok: true, data: problemPage(problems, 'retry-page') })
      .mockResolvedValueOnce({
        ok: false,
        error: { code: 'problem_list_failed', userMessage: '后续题目加载失败。', retryable: true, diagnosticId: 'page-failure' },
      })
      .mockResolvedValueOnce({ ok: true, data: problemPage([{
        id: 'problem-3', subject: '化学', note: '重试加载的题目', tags: [], status: 'active',
        questionAssetCount: 1, answerAssetCount: 1, questionPreviewDataUrl: null, updatedAtUtcMs: 0,
      }]) })

    const router = createAppRouter(createMemoryHistory())
    await router.push('/library')
    await router.isReady()
    render(LibraryView, { global: { plugins: [router], provide: { [syncControllerKey as symbol]: syncController } } })
    await screen.findByText('先看定义域。')
    await userEvent.click(screen.getByRole('button', { name: '批量管理' }))
    await userEvent.click(screen.getByRole('checkbox', { name: '选择 数学 错题' }))
    await userEvent.click(screen.getByRole('button', { name: '加载更多' }))

    expect(await screen.findByRole('alert')).toHaveTextContent('后续题目加载失败。')
    expect(screen.getByText('先看定义域。')).toBeVisible()
    expect(screen.getByRole('checkbox', { name: '选择 数学 错题' })).toBeChecked()
    await userEvent.click(screen.getByRole('button', { name: '加载更多' }))
    expect(await screen.findByText('重试加载的题目')).toBeVisible()
    expect(screen.queryByText('后续题目加载失败。')).not.toBeInTheDocument()
  })

  it('ignores a stale continuation response after the search filter changes', async () => {
    const append = deferred<unknown>()
    api.problemList.mockReset()
      .mockResolvedValueOnce({ ok: true, data: problemPage(problems, 'old-page') })
      .mockReturnValueOnce(append.promise)
      .mockResolvedValueOnce({ ok: true, data: problemPage([{
        id: 'filtered', subject: '化学', note: '筛选后结果', tags: [], status: 'active',
        questionAssetCount: 1, answerAssetCount: 1, questionPreviewDataUrl: null, updatedAtUtcMs: 3,
      }]) })

    const router = createAppRouter(createMemoryHistory())
    await router.push('/library')
    await router.isReady()
    render(LibraryView, { global: { plugins: [router], provide: { [syncControllerKey as symbol]: syncController } } })
    await screen.findByText('先看定义域。')
    screen.getByRole('button', { name: '加载更多' }).click()
    await userEvent.type(screen.getByRole('searchbox', { name: '搜索题库' }), '化学')

    expect(await screen.findByText('筛选后结果')).toBeVisible()
    expect(api.problemFilterOptions).toHaveBeenCalledTimes(1)
    append.resolve({ ok: true, data: problemPage([{
      id: 'stale', subject: '物理', note: '旧分页结果', tags: [], status: 'active',
      questionAssetCount: 1, answerAssetCount: 1, questionPreviewDataUrl: null, updatedAtUtcMs: 0,
    }]) })
    await Promise.resolve()
    expect(screen.queryByText('旧分页结果')).not.toBeInTheDocument()
    expect(screen.getByText('筛选后结果')).toBeVisible()
  })
})
