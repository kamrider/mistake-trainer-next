import { render, screen } from '@testing-library/vue'
import userEvent from '@testing-library/user-event'
import { describe, expect, it } from 'vitest'
import ReviewRoom from './ReviewRoom.vue'

describe('ReviewRoom', () => {
  it('keeps the answer hidden until requested and submits remembered', async () => {
    const user = userEvent.setup()
    const view = render(ReviewRoom, {
      props: {
        subject: '数学',
        prompt: '已知函数 f(x) 为奇函数，求 f(0)。',
        answer: '由奇函数定义可知 f(0) = 0。',
        current: 1,
        total: 18,
      },
    })

    expect(screen.queryByText('由奇函数定义可知 f(0) = 0。')).not.toBeInTheDocument()
    await user.click(screen.getByRole('button', { name: '显示答案' }))
    expect(screen.getByText('由奇函数定义可知 f(0) = 0。')).toBeVisible()

    await user.click(screen.getByRole('button', { name: '记住了' }))
    expect(view.emitted('rate')).toEqual([['remembered']])
  })

  it('submits forgot after revealing the answer', async () => {
    const user = userEvent.setup()
    const view = render(ReviewRoom, {
      props: {
        subject: '数学',
        prompt: '题目',
        answer: '答案',
        current: 2,
        total: 18,
      },
    })

    await user.click(screen.getByRole('button', { name: '显示答案' }))
    await user.click(screen.getByRole('button', { name: '忘记了' }))

    expect(view.emitted('rate')).toEqual([['forgot']])
  })
})
