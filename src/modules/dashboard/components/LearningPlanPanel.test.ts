import { render, screen } from '@testing-library/vue'
import userEvent from '@testing-library/user-event'
import { describe, expect, it } from 'vitest'
import LearningPlanPanel from './LearningPlanPanel.vue'

const plan = {
  reviewTarget: 20,
  minutesTarget: 25,
  completedReviews: 7,
  remainingReviews: 13,
  dueReviews: 9,
  suggestedReviews: 13,
  estimatedMinutes: 18,
}

describe('LearningPlanPanel', () => {
  it('shows real progress and emits a bounded goal update', async () => {
    const user = userEvent.setup()
    const view = render(LearningPlanPanel, { props: { plan } })
    expect(screen.getByText('今日完成 7 / 20 道')).toBeVisible()
    expect(screen.getByText('建议再完成 13 道，预计约 18 分钟。')).toBeVisible()

    await user.click(screen.getByRole('button', { name: '调整学习目标' }))
    await user.clear(screen.getByLabelText('每日复习题数'))
    await user.type(screen.getByLabelText('每日复习题数'), '30')
    await user.click(screen.getByRole('button', { name: '保存目标' }))
    expect(view.emitted('save')).toEqual([[{ dailyReviewTarget: 30, dailyMinutesTarget: 25 }]])
  })

  it('keeps invalid values local and explains an overdue workload', async () => {
    const user = userEvent.setup()
    const view = render(LearningPlanPanel, {
      props: { plan: { ...plan, dueReviews: 28, suggestedReviews: 28 } },
    })
    expect(screen.getByText('有 28 道到期题，超过目标时优先处理到期内容。')).toBeVisible()
    await user.click(screen.getByRole('button', { name: '调整学习目标' }))
    await user.clear(screen.getByLabelText('每日学习时间（分钟）'))
    await user.type(screen.getByLabelText('每日学习时间（分钟）'), '2')
    await user.click(screen.getByRole('button', { name: '保存目标' }))
    expect(screen.getByRole('alert')).toHaveTextContent('5–240')
    expect(view.emitted('save')).toBeUndefined()
  })
})
