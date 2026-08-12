import { render, screen } from '@testing-library/vue'
import { describe, expect, it } from 'vitest'
import DueForecastPanel from './DueForecastPanel.vue'

describe('DueForecastPanel', () => {
  it('renders seven accessible dates and separates overdue work calmly', () => {
    const days = Array.from({ length: 7 }, (_, index) => ({
      localDate: `2026-08-${String(10 + index).padStart(2, '0')}`,
      dueCount: index,
      overdueCount: index === 0 ? 3 : 0,
    }))
    render(DueForecastPanel, { props: { days } })

    expect(screen.getByRole('list', { name: '未来七天到期题预测' })).toBeVisible()
    expect(screen.getAllByRole('listitem')).toHaveLength(7)
    expect(screen.getByText('另有 3 题已到复习时间')).toBeVisible()
    expect(screen.queryByText(/逾期未完成|警告/)).not.toBeInTheDocument()
  })
})
