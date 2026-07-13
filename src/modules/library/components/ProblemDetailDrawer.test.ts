import { render, screen, waitFor } from '@testing-library/vue'
import userEvent from '@testing-library/user-event'
import { describe, expect, it, vi } from 'vitest'
import ProblemDetailDrawer from './ProblemDetailDrawer.vue'

describe('ProblemDetailDrawer', () => {
  it('renders question and answer previews and exposes close and train actions', async () => {
    const user = userEvent.setup()
    const view = render(ProblemDetailDrawer, {
      props: {
        loading: false,
        detail: {
          id: 'problem-1',
          subject: '数学',
          note: '先看定义域',
          status: 'active',
          updatedAtUtcMs: 1_700_000_000_000,
          assets: [
            { id: 'q-1', role: 'question', position: 0, mediaType: 'image/png', dataUrl: 'data:image/png;base64,AA==' },
            { id: 'a-1', role: 'answer', position: 0, mediaType: 'image/png', dataUrl: 'data:image/png;base64,AA==' },
          ],
        },
      },
    })

    expect(screen.getByRole('heading', { name: '数学' })).toBeVisible()
    expect(screen.getByRole('img', { name: '题目图片 1' })).toBeVisible()
    expect(screen.getByRole('img', { name: '答案图片 1' })).toBeVisible()
    await user.click(screen.getByRole('button', { name: '用这道题开始训练' }))
    await user.click(screen.getByRole('button', { name: '关闭题目详情' }))

    expect(view.emitted('train')).toEqual([['problem-1']])
    expect(view.emitted('close')).toHaveLength(1)
  })

  it('traps keyboard focus, closes with Escape, and restores prior focus', async () => {
    const outside = document.createElement('button')
    document.body.append(outside)
    outside.focus()
    const user = userEvent.setup()
    const view = render(ProblemDetailDrawer, {
      props: {
        loading: false,
        detail: {
          id: 'problem-1', subject: '数学', note: '', status: 'active', updatedAtUtcMs: null, assets: [],
        },
      },
    })
    const close = screen.getByRole('button', { name: '关闭题目详情' })
    const train = screen.getByRole('button', { name: '用这道题开始训练' })

    await waitFor(() => expect(close).toHaveFocus())
    await user.keyboard('{Shift>}{Tab}{/Shift}')
    expect(train).toHaveFocus()
    await user.keyboard('{Tab}')
    expect(close).toHaveFocus()
    await user.keyboard('{Escape}')
    expect(view.emitted('close')).toHaveLength(1)

    view.unmount()
    expect(outside).toHaveFocus()
    outside.remove()
  })

  it('does not discard dirty edits without confirmation', async () => {
    const confirm = vi.spyOn(window, 'confirm').mockReturnValue(false)
    const user = userEvent.setup()
    const view = render(ProblemDetailDrawer, {
      props: {
        loading: false,
        detail: {
          id: 'problem-1', subject: '数学', note: '', status: 'active', updatedAtUtcMs: null, assets: [],
        },
      },
    })

    await user.click(screen.getByRole('button', { name: '编辑题目' }))
    await user.type(screen.getByRole('textbox', { name: '科目' }), '竞赛')
    await user.click(screen.getByRole('button', { name: '关闭题目详情' }))
    expect(confirm).toHaveBeenCalledOnce()
    expect(view.emitted('close')).toBeUndefined()

    confirm.mockReturnValue(true)
    await user.click(screen.getByRole('button', { name: '关闭题目详情' }))
    expect(view.emitted('close')).toHaveLength(1)
    confirm.mockRestore()
  })
})
