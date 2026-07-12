import { render, screen } from '@testing-library/vue'
import userEvent from '@testing-library/user-event'
import { describe, expect, it } from 'vitest'
import CaptureWorkspace from './CaptureWorkspace.vue'

describe('CaptureWorkspace', () => {
  it('selects question and answer images through explicit Rust-owned actions', async () => {
    const user = userEvent.setup()
    const view = render(CaptureWorkspace, {
      props: { assets: [], saving: false },
    })

    await user.click(screen.getByRole('button', { name: '选择题图' }))
    await user.click(screen.getByRole('button', { name: '选择答案图' }))

    expect(view.emitted('select')).toEqual([['question'], ['answer']])
  })

  it('shows staged images and submits only their opaque ids with the form', async () => {
    const user = userEvent.setup()
    const view = render(CaptureWorkspace, {
      props: {
        saving: false,
        assets: [
          {
            id: 'question-stage-id',
            fileName: 'question.png',
            role: 'question',
            mediaType: 'image/png',
            byteLength: 1200,
            width: 800,
            height: 600,
          },
          {
            id: 'answer-stage-id',
            fileName: 'answer.png',
            role: 'answer',
            mediaType: 'image/png',
            byteLength: 900,
            width: 800,
            height: 600,
          },
        ],
      },
    })

    await user.type(screen.getByLabelText('科目'), '数学')
    await user.type(screen.getByLabelText('错因或笔记'), '定义域漏掉了零点')
    await user.click(screen.getByRole('button', { name: '保存到题库' }))

    expect(screen.getByText('question.png')).toBeVisible()
    expect(screen.getByText('answer.png')).toBeVisible()
    expect(view.emitted('commit')).toEqual([[
      {
        subject: '数学',
        note: '定义域漏掉了零点',
        stagedAssetIds: ['question-stage-id', 'answer-stage-id'],
      },
    ]])
  })

  it('keeps save disabled until both a question and an answer exist', () => {
    render(CaptureWorkspace, {
      props: {
        saving: false,
        assets: [
          {
            id: 'question-only',
            fileName: 'question.png',
            role: 'question',
            mediaType: 'image/png',
            byteLength: 1200,
            width: 800,
            height: 600,
          },
        ],
      },
    })

    expect(screen.getByRole('button', { name: '保存到题库' })).toBeDisabled()
    expect(screen.getByText('还需要至少一张答案图')).toBeVisible()
  })
})
