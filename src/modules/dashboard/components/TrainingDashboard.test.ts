import { render, screen } from '@testing-library/vue'
import userEvent from '@testing-library/user-event'
import { describe, expect, it } from 'vitest'
import TrainingDashboard from './TrainingDashboard.vue'

describe('TrainingDashboard', () => {
  it('starts the due review from the primary action', async () => {
    const user = userEvent.setup()
    const view = render(TrainingDashboard, {
      props: {
        learnerName: '小树',
        dueCount: 18,
        streakDays: 7,
        retention: 0.91,
      },
    })

    await user.click(screen.getByRole('button', { name: '开始复习 18 道' }))

    expect(view.emitted('start-review')).toHaveLength(1)
  })

  it('shows the learner and calm progress summary', () => {
    render(TrainingDashboard, {
      props: {
        learnerName: '小树',
        dueCount: 18,
        streakDays: 7,
        retention: 0.91,
      },
    })

    expect(screen.getByText('小树，今天从 18 道到期题开始。')).toBeVisible()
    expect(screen.getByText('91%')).toBeVisible()
    expect(screen.getByText('连续 7 天')).toBeVisible()
  })
})
