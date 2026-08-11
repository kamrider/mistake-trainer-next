import { render, screen } from '@testing-library/vue'
import userEvent from '@testing-library/user-event'
import { describe, expect, it } from 'vitest'
import type { DashboardOverview } from '@/shared/api/bindings'
import TrainingDashboard from './TrainingDashboard.vue'

const overview = (changes: Partial<DashboardOverview> = {}): DashboardOverview => ({
  profileName: '小树',
  activeProblemCount: 24,
  dueProblemCount: 18,
  reviewedTodayCount: 3,
  rememberedRate30Days: 0.91,
  currentStreakDays: 7,
  pendingCaptureBatchCount: 2,
  pendingCaptureItemCount: 14,
  ...changes,
})

describe('TrainingDashboard', () => {
  it('starts the real due review from the primary action', async () => {
    const user = userEvent.setup()
    const view = render(TrainingDashboard, { props: { overview: overview() } })

    await user.click(screen.getByRole('button', { name: '开始复习 18 道' }))

    expect(view.emitted('start-review')).toHaveLength(1)
    await user.click(screen.getByRole('button', { name: '快速训练' }))
    expect(view.emitted('open-quick')).toHaveLength(1)
    expect(screen.getByText('2 个批次 · 14 张图片待整理')).toBeVisible()
    expect(screen.queryByRole('button', { name: '整理采集箱' })).not.toBeInTheDocument()
  })

  it('guides an empty library to capture instead of inventing review data', async () => {
    const user = userEvent.setup()
    const view = render(TrainingDashboard, {
      props: { overview: overview({ activeProblemCount: 0, dueProblemCount: 0, rememberedRate30Days: null, currentStreakDays: 0 }) },
    })

    expect(screen.getByText('暂无复习数据')).toBeVisible()
    expect(screen.getByText('从今天开始')).toBeVisible()
    await user.click(screen.getByRole('button', { name: '录入第一道错题' }))
    expect(view.emitted('open-inbox')).toHaveLength(1)
    expect(screen.queryByRole('button', { name: '快速训练' })).not.toBeInTheDocument()
  })

  it('shows an all-clear state and opens the library when nothing is due', async () => {
    const user = userEvent.setup()
    const view = render(TrainingDashboard, { props: { overview: overview({ dueProblemCount: 0 }) } })

    expect(screen.getByText('小树，今天的到期题已经清空。')).toBeVisible()
    await user.click(screen.getByRole('button', { name: '查看题库' }))
    expect(view.emitted('open-library')).toHaveLength(1)
  })

  it('keeps loading and failure states explicit', async () => {
    const loading = render(TrainingDashboard, { props: { overview: null, loading: true } })
    expect(screen.getByText('正在读取训练台')).toBeInTheDocument()
    loading.unmount()

    const user = userEvent.setup()
    const failed = render(TrainingDashboard, { props: { overview: null, errorMessage: '本地资料库暂时忙碌。' } })
    expect(screen.getByRole('alert')).toHaveTextContent('没有用旧数字替代')
    await user.click(screen.getByRole('button', { name: '重新读取' }))
    expect(failed.emitted('retry')).toHaveLength(1)
  })
})
