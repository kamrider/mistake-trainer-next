import { render, screen } from '@testing-library/vue'
import userEvent from '@testing-library/user-event'
import { describe, expect, it } from 'vitest'
import ReviewHistoryFilters from './ReviewHistoryFilters.vue'

describe('ReviewHistoryFilters', () => {
  it('submits explicit filters and resets them to the bounded default', async () => {
    const user = userEvent.setup()
    const view = render(ReviewHistoryFilters, { props: { subjects: ['数学', '物理'], loading: false } })

    await user.selectOptions(screen.getByLabelText('时间'), '7_days')
    await user.selectOptions(screen.getByLabelText('评分'), 'good')
    await user.selectOptions(screen.getByLabelText('科目'), '数学')
    await user.type(screen.getByLabelText('笔记关键词'), '  圆锥曲线  ')
    await user.click(screen.getByRole('button', { name: '应用筛选' }))

    expect(view.emitted().submit?.[0]).toEqual([{ range: '7_days', rating: 'good', subject: '数学', search: '圆锥曲线' }])
    await user.click(screen.getByRole('button', { name: '重置' }))
    expect(view.emitted().reset).toHaveLength(1)
    expect(screen.getByLabelText('时间')).toHaveValue('30_days')
    expect(screen.getByLabelText('笔记关键词')).toHaveValue('')
  })
})
