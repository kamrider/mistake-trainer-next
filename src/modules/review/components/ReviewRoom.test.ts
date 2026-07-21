import { render, screen, waitFor } from '@testing-library/vue'
import userEvent from '@testing-library/user-event'
import { describe, expect, it } from 'vitest'
import ReviewRoom from './ReviewRoom.vue'

const baseProps = {
  subject: '数学',
  prompt: '已知函数 f(x) 为奇函数，求 f(0)。',
  answer: '由奇函数定义可知 f(0) = 0。',
  current: 1,
  total: 3,
  elapsedText: '00:12',
}

describe('ReviewRoom', () => {
  it('keeps the answer hidden, reveals it, and submits a simple rating', async () => {
    const user = userEvent.setup()
    const view = render(ReviewRoom, { props: baseProps })

    expect(screen.queryByText(baseProps.answer)).not.toBeInTheDocument()
    await user.click(screen.getByRole('button', { name: '显示答案' }))
    expect(screen.getByText(baseProps.answer)).toBeVisible()
    expect(view.emitted('reveal')).toHaveLength(1)

    await user.click(screen.getByRole('button', { name: '记住了' }))
    expect(view.emitted('rate')).toEqual([['remembered']])
  })

  it('uses Space to reveal and number keys to rate without hijacking editable fields', async () => {
    const user = userEvent.setup()
    const view = render(ReviewRoom, { props: baseProps })
    const input = document.createElement('input')
    document.body.append(input)
    input.focus()
    await user.keyboard(' ')
    expect(screen.queryByText(baseProps.answer)).not.toBeInTheDocument()

    input.blur()
    input.remove()
    await user.keyboard(' ')
    expect(screen.getByText(baseProps.answer)).toBeVisible()
    await user.keyboard('2')
    expect(view.emitted('rate')).toEqual([['remembered']])
  })

  it('supports four-rating controls and A/S/D/F shortcuts', async () => {
    const user = userEvent.setup()
    const view = render(ReviewRoom, { props: baseProps })

    await user.keyboard(' ')
    await user.click(screen.getByRole('button', { name: '使用 FSRS 四档评分' }))
    await user.keyboard('s')
    expect(view.emitted('rate')).toEqual([['hard']])
  })

  it('opens ordered question and answer images in the accessible lightbox', async () => {
    const user = userEvent.setup()
    render(ReviewRoom, {
      props: {
        ...baseProps,
        questionImages: ['data:image/png;base64,Q1', 'data:image/png;base64,Q2'],
        answerImages: ['data:image/png;base64,A1'],
      },
    })

    await user.click(screen.getByRole('button', { name: '放大题目图片 2' }))
    expect(screen.getByRole('dialog', { name: '题目图片大图' })).toBeVisible()
    expect(screen.getByText('2 / 2')).toBeVisible()
    await user.keyboard('{Escape}')
    expect(screen.queryByRole('dialog', { name: '题目图片大图' })).not.toBeInTheDocument()

    await user.keyboard(' ')
    await user.click(screen.getByRole('button', { name: '放大答案图片 1' }))
    expect(screen.getByRole('dialog', { name: '答案图片大图' })).toBeVisible()
  })

  it('shows persisted progress, countdown state, and resets reveal for the next card', async () => {
    const user = userEvent.setup()
    const view = render(ReviewRoom, {
      props: { ...baseProps, current: 2, resumed: true, timeLimitSeconds: 60, elapsedText: '00:00', expired: true },
    })

    expect(screen.getByText('已恢复上次进度')).toBeVisible()
    expect(screen.getByText('剩余 00:00')).toBeVisible()
    expect(screen.getByText('已到建议时限，仍可继续作答')).toBeVisible()

    await user.click(screen.getByRole('button', { name: '显示答案' }))
    await view.rerender({ current: 3, resumed: false, expired: false, elapsedText: '01:00' })
    expect(screen.queryByText(baseProps.answer)).not.toBeInTheDocument()
    expect(screen.getByRole('button', { name: '显示答案' })).toBeVisible()
  })

  it('does not emit duplicate ratings while a save is in progress', async () => {
    const user = userEvent.setup()
    const view = render(ReviewRoom, { props: { ...baseProps, submitting: true } })
    await user.keyboard(' ')
    await user.keyboard('2')
    expect(view.emitted('rate')).toBeUndefined()
  })

  it('labels a user-selected deck without changing the review controls', () => {
    render(ReviewRoom, { props: { ...baseProps, mode: 'manual' } })

    expect(screen.getByText('自选训练')).toBeVisible()
    expect(screen.getByRole('button', { name: '显示答案' })).toBeVisible()
  })

  it('keeps every answer secret while navigating the exam answering pass', async () => {
    const user = userEvent.setup()
    const view = render(ReviewRoom, {
      props: { ...baseProps, mode: 'exam', examPhase: 'answering', current: 1, total: 2 },
    })

    expect(screen.getByText('模拟考试 · 独立作答')).toBeVisible()
    await waitFor(() => expect(screen.getByRole('heading', { name: '先独立完成整组，再统一看答案' })).toHaveFocus())
    expect(screen.queryByText(baseProps.answer)).not.toBeInTheDocument()
    expect(screen.queryByRole('button', { name: '显示答案' })).not.toBeInTheDocument()
    expect(screen.getByRole('button', { name: '上一题' })).toBeDisabled()
    await user.click(screen.getByRole('button', { name: '下一题' }))
    expect(view.emitted('next')).toHaveLength(1)

    await view.rerender({ current: 2 })
    expect(screen.getByRole('button', { name: '开始核对答案' })).toBeVisible()
    await user.click(screen.getByRole('button', { name: '开始核对答案' }))
    expect(view.emitted('beginGrading')).toHaveLength(1)

    await user.keyboard('{ArrowLeft}')
    expect(view.emitted('previous')).toHaveLength(1)
  })

  it('shows answers immediately in exam grading and only exposes right or wrong', async () => {
    const user = userEvent.setup()
    const view = render(ReviewRoom, {
      props: { ...baseProps, mode: 'exam', examPhase: 'grading' },
    })

    expect(screen.getByText('模拟考试 · 核对答案')).toBeVisible()
    expect(screen.getByText(baseProps.answer)).toBeVisible()
    expect(screen.queryByRole('button', { name: '使用 FSRS 四档评分' })).not.toBeInTheDocument()
    await user.click(screen.getByRole('button', { name: '答错' }))
    expect(view.emitted('rate')).toEqual([['forgot']])
    await user.click(screen.getByRole('button', { name: '答对' }))
    expect(view.emitted('rate')).toEqual([['forgot'], ['remembered']])
  })
})
