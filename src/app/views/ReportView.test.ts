import { render, screen, waitFor } from '@testing-library/vue'
import userEvent from '@testing-library/user-event'
import { createMemoryHistory, RouterView } from 'vue-router'
import { beforeEach, describe, expect, it, vi } from 'vitest'
import { createAppRouter } from '../router'
import { syncControllerKey } from '../sync-controller'
import {
  createWorkspaceTransitionGuard,
  workspaceTransitionGuardKey,
} from '../workspace-transition-guard'
import ReportView from './ReportView.vue'

const api = vi.hoisted(() => ({
  reportSummary: vi.fn(), exportList: vi.fn(), exportTrashList: vi.fn(), exportCandidates: vi.fn(), exportCreate: vi.fn(), exportGenerate: vi.fn(), exportDelete: vi.fn(), exportRestore: vi.fn(),
}))

vi.mock('@tauri-apps/api/core', () => ({ isTauri: () => true }))
vi.mock('../../shared/api/bindings', () => ({ commands: api }))

function deferred<T>() {
  let resolve!: (value: T) => void
  let reject!: (reason?: unknown) => void
  const promise = new Promise<T>((finish, fail) => { resolve = finish; reject = fail })
  return { promise, resolve, reject }
}

describe('ReportView', () => {
  const syncController = {
    run: vi.fn(),
    scheduleMutation: vi.fn(),
    dispose: vi.fn(),
  }
  const renderView = () => render(ReportView, {
    global: { provide: { [syncControllerKey as symbol]: syncController } },
  })
  const renderRoutedView = async () => {
    const router = createAppRouter(createMemoryHistory())
    const workspaceTransitionGuard = createWorkspaceTransitionGuard()
    await router.push('/report')
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
    return { router, view, workspaceTransitionGuard }
  }
  const mathCandidate = {
    id: 'problem-1', subject: '数学', note: '圆锥曲线', questionAssetCount: 1,
    answerAssetCount: 1, dueAtUtcMs: null, reviewCount: 0,
  }
  const physicsCandidate = {
    id: 'problem-2', subject: '物理', note: '受力分析', questionAssetCount: 1,
    answerAssetCount: 1, dueAtUtcMs: 1_700_000_000_000, reviewCount: 2,
  }
  const chemistryCandidate = {
    id: 'problem-3', subject: '化学', note: '反应路线', questionAssetCount: 1,
    answerAssetCount: 0, dueAtUtcMs: null, reviewCount: 0,
  }

  beforeEach(() => {
    vi.clearAllMocks()
    api.reportSummary.mockResolvedValue({ ok: true, data: {
      activeProblemCount: 1, dueProblemCount: 1, reviewCount: 4, rememberedRate: 0.75,
      totalDurationMs: 120_000, currentStreakDays: 2,
      dailyActivity: [{ dayStartUtcMs: 1_700_000_000_000, reviewCount: 4, durationMs: 120_000 }],
      subjectActivity: [{ subject: '数学', problemCount: 1, reviewCount: 4 }],
      weakAreas: [{ label: '错因·计算失误', kind: 'reason', reviewedCount: 4, lapseCount: 2, lapseRate: 0.5, averageDurationMs: 30_000 }],
      dueForecast: Array.from({ length: 7 }, (_, index) => ({
        localDate: `2026-08-${String(10 + index).padStart(2, '0')}`,
        dueCount: index,
        overdueCount: index === 0 ? 2 : 0,
      })),
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

    expect(await screen.findByRole('heading', { level: 1, name: '学习报告' })).toBeVisible()
    expect(api.reportSummary).toHaveBeenCalledWith(-new Date().getTimezoneOffset())
    expect(screen.getByRole('heading', { name: '本周最值得修正' })).toBeVisible()
    expect(await screen.findByRole('list', { name: '未来七天到期题预测' })).toBeVisible()
    expect(screen.getByText(/把练习变成看得见的节奏/)).toBeVisible()
    expect(screen.getByRole('navigation', { name: '报告页面目录' })).toHaveTextContent('学习概览导出中心')
    expect(screen.getByRole('region', { name: '学习概览' })).toHaveAttribute('id', 'report-overview')
    expect(screen.getByRole('region', { name: '保存配置并生成学习材料' })).toHaveAttribute('id', 'export-center')
    expect(screen.getByRole('complementary', { name: '先保存方案，再生成文件' })).toBeVisible()
    expect(await screen.findByText('75')).toBeVisible()
    expect(screen.getByText('按本地训练日计算')).toBeVisible()
    expect(screen.queryByText('按 UTC 训练日计算')).not.toBeInTheDocument()
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

  it('admits only one full refresh in the same event-loop turn', async () => {
    const reportRefresh = deferred<Awaited<ReturnType<typeof api.reportSummary>>>()
    renderView()
    const refresh = await screen.findByRole('button', { name: '刷新' })
    await waitFor(() => expect(refresh).toBeEnabled())
    api.reportSummary.mockClear()
    api.exportList.mockClear()
    api.exportTrashList.mockClear()
    api.exportCandidates.mockClear()
    api.reportSummary.mockReturnValueOnce(reportRefresh.promise)

    refresh.click()
    refresh.click()

    await waitFor(() => expect(api.reportSummary).toHaveBeenCalledOnce())
    expect(api.exportList).toHaveBeenCalledOnce()
    expect(api.exportTrashList).toHaveBeenCalledOnce()
    expect(api.exportCandidates).toHaveBeenCalledOnce()
    reportRefresh.resolve({ ok: true, data: {
      activeProblemCount: 1, dueProblemCount: 1, reviewCount: 4, rememberedRate: 0.75,
      totalDurationMs: 120_000, currentStreakDays: 2, dailyActivity: [], subjectActivity: [],
      weakAreas: [], dueForecast: [],
    } })
    await waitFor(() => expect(refresh).toBeEnabled())
  })

  it('keeps explicit candidates visible during a same-source refresh and after failure', async () => {
    const user = userEvent.setup()
    const candidateRefresh = deferred<Awaited<ReturnType<typeof api.exportCandidates>>>()
    api.exportCandidates.mockResolvedValueOnce({ ok: true, data: [mathCandidate, physicsCandidate] })
    renderView()
    const math = await screen.findByRole('checkbox', { name: '选择数学：圆锥曲线' })
    const physics = screen.getByRole('checkbox', { name: '选择物理：受力分析' })
    await user.click(physics)
    api.exportCandidates.mockReturnValueOnce(candidateRefresh.promise)

    await user.click(screen.getByRole('button', { name: '刷新' }))

    expect(math).toBeChecked()
    expect(physics).not.toBeChecked()
    candidateRefresh.resolve({ ok: false, error: {
      code: 'export_candidates_failed', userMessage: '候选题刷新失败。',
      retryable: true, diagnosticId: 'diag-refresh',
    } })
    expect(await screen.findByRole('alert')).toHaveTextContent('候选题刷新失败。')
    expect(screen.getByRole('checkbox', { name: '选择数学：圆锥曲线' })).toBeChecked()
    expect(screen.getByRole('checkbox', { name: '选择物理：受力分析' })).not.toBeChecked()
  })

  it('reconciles a successful same-source refresh without selecting new candidates', async () => {
    const user = userEvent.setup()
    api.exportCandidates
      .mockResolvedValueOnce({ ok: true, data: [mathCandidate, physicsCandidate] })
      .mockResolvedValueOnce({ ok: true, data: [physicsCandidate, chemistryCandidate, mathCandidate] })
    renderView()
    await user.click(await screen.findByRole('checkbox', { name: '选择物理：受力分析' }))

    await user.click(screen.getByRole('button', { name: '刷新' }))

    expect(await screen.findByRole('checkbox', { name: '选择化学：反应路线' })).not.toBeChecked()
    expect(screen.getByRole('checkbox', { name: '选择数学：圆锥曲线' })).toBeChecked()
    expect(screen.getByRole('checkbox', { name: '选择物理：受力分析' })).not.toBeChecked()
  })

  it('keeps authoritative empty snapshot states after a refresh failure', async () => {
    const user = userEvent.setup()
    renderView()
    expect(await screen.findByText('还没有保存过导出快照。')).toBeVisible()
    expect(screen.getByText('回收区为空。')).toBeVisible()
    api.exportList.mockResolvedValueOnce({ ok: false, error: {
      code: 'export_list_failed', userMessage: '快照读取失败。',
      retryable: true, diagnosticId: 'diag-list-refresh',
    } })
    api.exportTrashList.mockResolvedValueOnce({ ok: false, error: {
      code: 'export_trash_failed', userMessage: '回收区读取失败。',
      retryable: true, diagnosticId: 'diag-trash-refresh',
    } })

    await user.click(screen.getByRole('button', { name: '刷新' }))

    expect(await screen.findByRole('alert')).toHaveTextContent('导出快照没有读取成功。')
    expect(screen.getByText('还没有保存过导出快照。')).toBeVisible()
    expect(screen.getByText('回收区为空。')).toBeVisible()
    expect(screen.queryByText('正在读取导出快照…')).not.toBeInTheDocument()
    expect(screen.queryByText('正在读取回收区…')).not.toBeInTheDocument()
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

  it('requires an in-app decision before moving a snapshot to the recycle area', async () => {
    const user = userEvent.setup()
    api.exportList.mockResolvedValue({ ok: true, data: [{
      id: 'snapshot-1', title: '本周复盘', problemCount: 1,
      layout: 'question_answer_alternating', createdAtUtcMs: 1,
    }] })
    renderView()

    await user.click(await screen.findByRole('button', { name: '删除导出快照：本周复盘' }))
    expect(screen.getByRole('alertdialog', { name: '将“本周复盘”移入回收区？' })).toBeVisible()
    expect(api.exportDelete).not.toHaveBeenCalled()
    await user.click(screen.getByRole('button', { name: '保留快照' }))
    expect(api.exportDelete).not.toHaveBeenCalled()

    await user.click(screen.getByRole('button', { name: '删除导出快照：本周复盘' }))
    await user.click(screen.getByRole('button', { name: '移入回收区' }))

    await waitFor(() => expect(api.exportDelete).toHaveBeenCalledWith('snapshot-1'))
    expect(screen.queryByRole('button', { name: '删除导出快照：本周复盘' })).not.toBeInTheDocument()
  })

  it('prevents restore from racing a confirmed deletion and recycle-bin refresh', async () => {
    const user = userEvent.setup()
    let resolveDelete: (value: unknown) => void = () => undefined
    const deleted = {
      snapshot: {
        id: 'snapshot-deleted', title: '上月复盘', problemCount: 3,
        layout: 'questions_then_answers', createdAtUtcMs: 1_700_000_000_000,
      },
      deletedAtUtcMs: 1_700_100_000_000,
      purgeAfterUtcMs: 1_702_692_000_000,
    }
    api.exportList.mockResolvedValue({ ok: true, data: [{
      id: 'snapshot-1', title: '本周复盘', problemCount: 1,
      layout: 'question_answer_alternating', createdAtUtcMs: 1,
    }] })
    api.exportTrashList
      .mockResolvedValueOnce({ ok: true, data: [deleted] })
      .mockResolvedValueOnce({ ok: true, data: [deleted] })
    api.exportDelete.mockImplementationOnce(() => new Promise(resolve => { resolveDelete = resolve }))
    renderView()

    await user.click(await screen.findByRole('button', { name: '删除导出快照：本周复盘' }))
    await user.click(screen.getByRole('button', { name: '移入回收区' }))
    await waitFor(() => expect(api.exportDelete).toHaveBeenCalledWith('snapshot-1'))
    const restore = screen.getByRole('button', { name: '恢复导出快照：上月复盘' })
    expect(restore).toBeDisabled()
    await user.click(restore)
    expect(api.exportRestore).not.toHaveBeenCalled()

    resolveDelete({ ok: true, data: true })
    await waitFor(() => expect(syncController.scheduleMutation).toHaveBeenCalledOnce())
    await waitFor(() => expect(api.exportTrashList).toHaveBeenCalledTimes(2))
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

  it('does not allow a new snapshot before the initial snapshot list is authoritative', async () => {
    const listRequest = deferred<{ ok: true, data: never[] }>()
    api.exportList.mockReturnValueOnce(listRequest.promise)
    renderView()

    expect(await screen.findByRole('checkbox', { name: '选择数学：圆锥曲线' })).toBeChecked()
    const save = screen.getByRole('button', { name: /保存 1 道题的导出快照/ })
    expect(save).toBeDisabled()
    await userEvent.click(save)
    expect(api.exportCreate).not.toHaveBeenCalled()

    listRequest.resolve({ ok: true, data: [] })
    await waitFor(() => expect(save).toBeEnabled())
  })

  it('locks generation, deletion, and restore behind one native-operation flight', async () => {
    const generateRequest = deferred<Awaited<ReturnType<typeof api.exportGenerate>>>()
    api.exportList.mockResolvedValue({ ok: true, data: [{
      id: 'snapshot-1', title: '本周复盘', problemCount: 1,
      layout: 'question_answer_alternating', createdAtUtcMs: 1,
    }] })
    api.exportTrashList.mockResolvedValue({ ok: true, data: [{
      snapshot: {
        id: 'snapshot-deleted', title: '上月复盘', problemCount: 3,
        layout: 'questions_then_answers', createdAtUtcMs: 1_700_000_000_000,
      },
      deletedAtUtcMs: 1_700_100_000_000,
      purgeAfterUtcMs: 1_702_692_000_000,
    }] })
    api.exportGenerate.mockReturnValueOnce(generateRequest.promise)
    renderView()

    await userEvent.click(await screen.findByRole('button', { name: '生成导出文件：本周复盘' }))
    await waitFor(() => expect(api.exportGenerate).toHaveBeenCalledWith('snapshot-1'))
    const remove = screen.getByRole('button', { name: '删除导出快照：本周复盘' })
    const restore = screen.getByRole('button', { name: '恢复导出快照：上月复盘' })
    expect(remove).toBeDisabled()
    expect(restore).toBeDisabled()

    generateRequest.resolve({ ok: true, data: {
      snapshotId: 'snapshot-1', outputName: '本周复盘.docx', problemCount: 1,
      layout: 'question_answer_alternating',
    } })
    await waitFor(() => expect(remove).toBeEnabled())
    expect(restore).toBeEnabled()
  })

  it('blocks route, workspace, and window transitions during an export operation', async () => {
    const generateRequest = deferred<Awaited<ReturnType<typeof api.exportGenerate>>>()
    api.exportList.mockResolvedValue({ ok: true, data: [{
      id: 'snapshot-1', title: '本周复盘', problemCount: 1,
      layout: 'question_answer_alternating', createdAtUtcMs: 1,
    }] })
    api.exportGenerate.mockReturnValueOnce(generateRequest.promise)
    const { router, view, workspaceTransitionGuard } = await renderRoutedView()

    await userEvent.click(await screen.findByRole('button', { name: '生成导出文件：本周复盘' }))
    await waitFor(() => expect(api.exportGenerate).toHaveBeenCalledWith('snapshot-1'))

    await expect(workspaceTransitionGuard.attempt()).resolves.toBe(false)
    expect(await screen.findByRole('alert')).toHaveTextContent('导出操作正在进行，请等待完成后再离开。')
    const busyUnload = new Event('beforeunload', { cancelable: true })
    window.dispatchEvent(busyUnload)
    expect(busyUnload.defaultPrevented).toBe(true)
    await router.push({ name: 'dashboard' })
    expect(router.currentRoute.value.name).toBe('report')

    generateRequest.resolve({ ok: true, data: {
      snapshotId: 'snapshot-1', outputName: '本周复盘.docx', problemCount: 1,
      layout: 'question_answer_alternating',
    } })
    expect(await screen.findByRole('status')).toHaveTextContent('已生成 本周复盘.docx，共 1 题。')
    expect(screen.queryByText('导出操作正在进行，请等待完成后再离开。')).not.toBeInTheDocument()
    await expect(workspaceTransitionGuard.attempt()).resolves.toBe(true)
    const idleUnload = new Event('beforeunload', { cancelable: true })
    window.dispatchEvent(idleUnload)
    expect(idleUnload.defaultPrevented).toBe(false)
    await router.push({ name: 'dashboard' })
    expect(router.currentRoute.value.name).toBe('dashboard')

    view.unmount()
    const afterUnmount = new Event('beforeunload', { cancelable: true })
    window.dispatchEvent(afterUnmount)
    expect(afterUnmount.defaultPrevented).toBe(false)
  })
})
