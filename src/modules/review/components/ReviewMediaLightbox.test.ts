import { render, screen } from '@testing-library/vue'
import userEvent from '@testing-library/user-event'
import { describe, expect, it } from 'vitest'
import ReviewMediaLightbox from './ReviewMediaLightbox.vue'

const images = ['data:image/png;base64,question-one', 'data:image/png;base64,question-two']

describe('ReviewMediaLightbox', () => {
  it('navigates images with buttons and arrow keys', async () => {
    const user = userEvent.setup()
    render(ReviewMediaLightbox, {
      props: { images, initialIndex: 0, label: '题目图片' },
    })

    expect(screen.getByRole('dialog', { name: '题目图片大图' })).toBeVisible()
    expect(screen.getByText('1 / 2')).toBeVisible()
    expect(screen.getByRole('button', { name: '上一张题目图片' })).toBeDisabled()

    await user.click(screen.getByRole('button', { name: '下一张题目图片' }))
    expect(screen.getByText('2 / 2')).toBeVisible()
    expect(screen.getByRole('img', { name: '题目图片 2' })).toHaveAttribute('src', images[1]!)

    await user.keyboard('{ArrowLeft}')
    expect(screen.getByText('1 / 2')).toBeVisible()
  })

  it('closes with Escape and disables navigation for one image', async () => {
    const user = userEvent.setup()
    const view = render(ReviewMediaLightbox, {
      props: { images: [images[0]!], initialIndex: 0, label: '答案图片' },
    })

    expect(screen.getByRole('button', { name: '上一张答案图片' })).toBeDisabled()
    expect(screen.getByRole('button', { name: '下一张答案图片' })).toBeDisabled()
    await user.keyboard('{Escape}')
    expect(view.emitted('close')).toHaveLength(1)
  })

  it('moves initial focus into the modal and traps Tab navigation', async () => {
    const user = userEvent.setup()
    render(ReviewMediaLightbox, {
      props: { images, initialIndex: 0, label: '题目图片' },
    })

    const close = screen.getByRole('button', { name: '关闭大图' })
    const next = screen.getByRole('button', { name: '下一张题目图片' })
    expect(close).toHaveFocus()

    await user.tab({ shift: true })
    expect(next).toHaveFocus()
    await user.tab()
    expect(close).toHaveFocus()
  })
})
