import { render, screen } from '@testing-library/vue'
import userEvent from '@testing-library/user-event'
import { describe, expect, it } from 'vitest'
import SchulteFocus from './SchulteFocus.vue'

const focus = {
  kind: 'warmup',
  roundIndex: 0,
  numbers: Array.from({ length: 25 }, (_, index) => index + 1),
  nextNumber: 1,
  elapsedMs: 640,
}

describe('SchulteFocus', () => {
  it('renders one accessible 5×5 board and announces persisted progress', () => {
    render(SchulteFocus, { props: { focus } })

    expect(screen.getByRole('grid', { name: '舒尔特数字方格' })).toBeVisible()
    expect(screen.getAllByRole('gridcell')).toHaveLength(25)
    expect(screen.getByText('下一位 1')).toBeVisible()
    expect(screen.getByText('0 / 25')).toBeVisible()
  })

  it('keeps wrong choices local and emits only the persisted next number', async () => {
    const user = userEvent.setup()
    const view = render(SchulteFocus, { props: { focus } })

    await user.click(screen.getByRole('button', { name: '数字 2' }))
    expect(screen.getByText('请先找到 1')).toBeVisible()
    expect(view.emitted('select')).toBeUndefined()

    await user.click(screen.getByRole('button', { name: '数字 1' }))
    const selections = view.emitted('select') as unknown as Array<[number, number]>
    expect(selections).toHaveLength(1)
    expect(selections[0]![0]).toBe(1)
    expect(selections[0]![1]).toBeGreaterThanOrEqual(640)
  })

  it('supports roving arrow, Home, End and Enter keyboard operation', async () => {
    const user = userEvent.setup()
    const view = render(SchulteFocus, { props: { focus } })
    const first = screen.getByRole('button', { name: '数字 1' })
    const second = screen.getByRole('button', { name: '数字 2' })
    const last = screen.getByRole('button', { name: '数字 25' })

    first.focus()
    await user.keyboard('{ArrowRight}')
    expect(second).toHaveFocus()
    await user.keyboard('{End}')
    expect(last).toHaveFocus()
    await user.keyboard('{Home}')
    expect(first).toHaveFocus()
    await user.keyboard('{Enter}')
    const selections = view.emitted('select') as unknown as Array<[number, number]>
    expect(selections[0]![0]).toBe(1)
  })

  it('offers both safe exits and shows a persisted completion seal', async () => {
    const user = userEvent.setup()
    const view = render(SchulteFocus, { props: { focus, completed: true } })

    expect(screen.getByText('这一轮已保存')).toBeVisible()
    await user.click(screen.getByRole('button', { name: '退出训练台' }))
    await user.click(screen.getByRole('button', { name: '跳过，继续训练' }))
    expect(view.emitted('exit')).toHaveLength(1)
    expect(view.emitted('skip')).toHaveLength(1)
  })
})
