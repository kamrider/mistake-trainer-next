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

  it('renders encrypted image previews and supports the remembered keyboard shortcut', async () => {
    const user = userEvent.setup()
    const view = render(ReviewRoom, {
      props: {
        subject: '物理',
        prompt: '复盘提示',
        answer: '答案提示',
        questionImages: ['data:image/png;base64,AA=='],
        answerImages: ['data:image/png;base64,AA=='],
        current: 1,
        total: 1,
      },
    })

    expect(screen.getByRole('img', { name: '训练题图 1' })).toBeVisible()
    expect(screen.queryByRole('img', { name: '训练答案图 1' })).not.toBeInTheDocument()
    await user.click(screen.getByRole('button', { name: '显示答案' }))
    expect(screen.getByRole('img', { name: '训练答案图 1' })).toBeVisible()
    await user.keyboard('2')
    expect(view.emitted('rate')).toEqual([['remembered']])
  })

  it('hides the answer again when consecutive problems share the same prompt', async () => {
    const user = userEvent.setup()
    const view = render(ReviewRoom, {
      props: {
        subject: '数学',
        prompt: '相同的复盘提示',
        answer: '相同题目的正确答案',
        current: 1,
        total: 2,
      },
    })

    await user.click(screen.getByRole('button', { name: '显示答案' }))
    expect(screen.getByText('相同题目的正确答案')).toBeVisible()
    await view.rerender({ current: 2 })
    expect(screen.queryByText('相同题目的正确答案')).not.toBeInTheDocument()
    expect(screen.getByRole('button', { name: '显示答案' })).toBeVisible()
  })

  it('supports opt-in four-rating FSRS controls', async () => {
    const user = userEvent.setup()
    const view = render(ReviewRoom, {
      props: {
        subject: '数学',
        prompt: '题目',
        answer: '答案',
        current: 1,
        total: 1,
      },
    })

    await user.click(screen.getByRole('button', { name: '显示答案' }))
    await user.click(screen.getByRole('button', { name: '使用 FSRS 四档评分' }))
    await user.click(screen.getByRole('button', { name: '有点吃力' }))
    expect(view.emitted('rate')).toEqual([['hard']])
  })
})
