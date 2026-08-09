import { render, screen, waitFor } from '@testing-library/vue'
import userEvent from '@testing-library/user-event'
import { describe, expect, it, vi } from 'vitest'
import type { NavigationAttempt } from '../../../app/composables/useUnsavedChangesGuard'
import ProblemDetailDrawer from './ProblemDetailDrawer.vue'

describe('ProblemDetailDrawer', () => {
  it('renders question and answer previews and exposes close and train actions', async () => {
    const user = userEvent.setup()
    const view = render(ProblemDetailDrawer, {
      props: {
        loading: false,
        previousProblemId: 'problem-0',
        nextProblemId: 'problem-2',
        detail: {
          id: 'problem-1',
          subject: '数学',
          note: '先看定义域',
          tags: ['函数', '粗心'],
          status: 'active',
          timeLimitSeconds: null,
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
    await user.click(screen.getByRole('button', { name: '下一题' }))
    expect(screen.queryByRole('menuitem', { name: '归档' })).not.toBeInTheDocument()
    await user.click(screen.getByRole('button', { name: '更多题目操作' }))
    expect(screen.getByRole('menuitem', { name: '归档' })).toBeVisible()
    expect(screen.getByRole('menuitem', { name: '移入回收站' })).toBeVisible()
    await user.click(screen.getByRole('button', { name: '用这道题开始训练' }))
    await user.click(screen.getByRole('button', { name: '关闭题目详情' }))

    expect(view.emitted('train')).toEqual([['problem-1']])
    expect(view.emitted('navigate')).toEqual([['problem-2']])
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
        nextProblemId: 'problem-2',
        detail: {
          id: 'problem-1', subject: '数学', note: '', tags: [], status: 'active', timeLimitSeconds: null, updatedAtUtcMs: null, assets: [],
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

  it('implements the complete keyboard and dismissal model for more actions', async () => {
    const user = userEvent.setup()
    const view = render(ProblemDetailDrawer, {
      props: {
        loading: false,
        detail: {
          id: 'problem-1', subject: '数学', note: '', tags: [], status: 'active', timeLimitSeconds: null, updatedAtUtcMs: null, assets: [],
        },
      },
    })
    const close = screen.getByRole('button', { name: '关闭题目详情' })
    await waitFor(() => expect(close).toHaveFocus())
    const trigger = screen.getByRole('button', { name: '更多题目操作' })

    expect(trigger).toHaveAttribute('aria-haspopup', 'menu')
    expect(trigger).toHaveAttribute('aria-controls', 'problem-detail-actions-menu')
    expect(trigger).toHaveAttribute('aria-expanded', 'false')
    trigger.focus()
    await user.keyboard('{ArrowDown}')

    const menu = screen.getByRole('menu', { name: '更多题目操作' })
    const archive = screen.getByRole('menuitem', { name: '归档' })
    const trash = screen.getByRole('menuitem', { name: '移入回收站' })
    expect(menu).toBeVisible()
    await waitFor(() => expect(archive).toHaveFocus())
    expect(trigger).toHaveAttribute('aria-expanded', 'true')
    expect(archive).toHaveAttribute('tabindex', '-1')
    expect(trash).toHaveAttribute('tabindex', '-1')

    await user.keyboard('{ArrowDown}')
    expect(trash).toHaveFocus()
    await user.keyboard('{ArrowDown}')
    expect(archive).toHaveFocus()
    await user.keyboard('{ArrowUp}')
    expect(trash).toHaveFocus()
    await user.keyboard('{Home}')
    expect(archive).toHaveFocus()
    await user.keyboard('{End}')
    expect(trash).toHaveFocus()

    await user.keyboard('{Escape}')
    expect(screen.queryByRole('menu')).not.toBeInTheDocument()
    await waitFor(() => expect(trigger).toHaveFocus())
    expect(view.emitted('close')).toBeUndefined()

    await user.keyboard('{ArrowUp}')
    await waitFor(() => expect(screen.getByRole('menuitem', { name: '移入回收站' })).toHaveFocus())
    await user.keyboard('{Tab}')
    expect(screen.queryByRole('menu')).not.toBeInTheDocument()

    trigger.focus()
    await user.keyboard('{ArrowDown}')
    await waitFor(() => expect(screen.getByRole('menuitem', { name: '归档' })).toHaveFocus())
    await user.keyboard('{Shift>}{Tab}{/Shift}')
    expect(screen.queryByRole('menu')).not.toBeInTheDocument()
    expect(trigger).toHaveFocus()

    await user.click(trigger)
    await waitFor(() => expect(screen.getByRole('menuitem', { name: '归档' })).toHaveFocus())
    await user.click(screen.getByRole('heading', { name: '数学' }))
    expect(screen.queryByRole('menu')).not.toBeInTheDocument()
    expect(view.emitted('status')).toBeUndefined()

    await user.click(trigger)
    await view.rerender({
      detail: {
        id: 'problem-2', subject: '物理', note: '', tags: [], status: 'active', timeLimitSeconds: null, updatedAtUtcMs: null, assets: [],
      },
    })
    expect(screen.queryByRole('menu')).not.toBeInTheDocument()
  })

  it('keeps dirty edits after cancellation and discards them only after explicit confirmation', async () => {
    const user = userEvent.setup()
    const view = render(ProblemDetailDrawer, {
      props: {
        loading: false,
        nextProblemId: 'problem-2',
        detail: {
          id: 'problem-1', subject: '数学', note: '', tags: [], status: 'active', timeLimitSeconds: null, updatedAtUtcMs: null, assets: [],
        },
      },
    })

    await user.click(screen.getByRole('button', { name: '编辑题目' }))
    await user.type(screen.getByRole('textbox', { name: '科目' }), '竞赛')
    await user.click(screen.getByRole('button', { name: '关闭题目详情' }))
    expect(screen.getByRole('alertdialog', { name: '放弃尚未保存的修改？' })).toBeVisible()
    expect(view.emitted('close')).toBeUndefined()
    await user.click(screen.getByRole('button', { name: '继续编辑' }))
    expect(screen.getByRole('textbox', { name: '科目' })).toHaveValue('数学竞赛')
    expect(view.emitted('close')).toBeUndefined()

    await user.click(screen.getByRole('button', { name: '下一题' }))
    expect(screen.getByRole('alertdialog', { name: '放弃尚未保存的修改？' })).toBeVisible()
    expect(view.emitted('navigate')).toBeUndefined()
    await user.click(screen.getByRole('button', { name: '继续编辑' }))
    expect(screen.getByRole('textbox', { name: '科目' })).toHaveValue('数学竞赛')

    await user.click(screen.getByRole('button', { name: '关闭题目详情' }))
    await user.click(screen.getByRole('button', { name: '放弃修改' }))
    expect(view.emitted('close')).toHaveLength(1)
  })

  it('keeps dirty fields and edit mode across a same-problem detail refresh', async () => {
    const user = userEvent.setup()
    const view = render(ProblemDetailDrawer, {
      props: {
        loading: false,
        detail: {
          id: 'problem-1', subject: '数学', note: '', tags: [], status: 'active', timeLimitSeconds: null, updatedAtUtcMs: 1, assets: [],
        },
      },
    })

    await user.click(screen.getByRole('button', { name: '编辑题目' }))
    await user.type(screen.getByRole('textbox', { name: '科目' }), '竞赛')
    await view.rerender({
      detail: {
        id: 'problem-1', subject: '数学', note: '', tags: [], status: 'active', timeLimitSeconds: null, updatedAtUtcMs: 2,
        assets: [{ id: 'q-1', role: 'question', position: 0, mediaType: 'image/png', dataUrl: 'data:image/png;base64,AA==' }],
      },
    })

    expect(screen.getByRole('textbox', { name: '科目' })).toHaveValue('数学竞赛')
    expect(screen.queryByRole('button', { name: '编辑题目' })).not.toBeInTheDocument()
  })

  it('closes a submitted editor only after matching refreshed detail arrives', async () => {
    const user = userEvent.setup()
    const view = render(ProblemDetailDrawer, {
      props: {
        loading: false,
        detail: {
          id: 'problem-1', subject: '数学', note: '', tags: [], status: 'active', timeLimitSeconds: null, updatedAtUtcMs: 1, assets: [],
        },
      },
    })

    await user.click(screen.getByRole('button', { name: '编辑题目' }))
    await user.type(screen.getByRole('textbox', { name: '科目' }), '竞赛')
    await user.click(screen.getByRole('button', { name: '保存修改' }))
    expect(screen.getByRole('textbox', { name: '科目' })).toHaveValue('数学竞赛')

    await view.rerender({
      saving: false,
      detail: {
        id: 'problem-1', subject: '数学竞赛', note: '', tags: [], status: 'active', timeLimitSeconds: null, updatedAtUtcMs: 2, assets: [],
      },
    })

    expect(screen.getByRole('button', { name: '编辑题目' })).toBeVisible()
    expect(screen.queryByRole('textbox', { name: '科目' })).not.toBeInTheDocument()
  })

  it('does not let an older save result erase newer typing', async () => {
    const user = userEvent.setup()
    const view = render(ProblemDetailDrawer, {
      props: {
        loading: false,
        detail: {
          id: 'problem-1', subject: '数学', note: '', tags: [], status: 'active', timeLimitSeconds: null, updatedAtUtcMs: 1, assets: [],
        },
      },
    })

    await user.click(screen.getByRole('button', { name: '编辑题目' }))
    const subject = screen.getByRole('textbox', { name: '科目' })
    await user.type(subject, '竞赛')
    await user.click(screen.getByRole('button', { name: '保存修改' }))
    await user.type(subject, '进阶')
    await view.rerender({
      saving: false,
      detail: {
        id: 'problem-1', subject: '数学竞赛', note: '', tags: [], status: 'active', timeLimitSeconds: null, updatedAtUtcMs: 2, assets: [],
      },
    })

    expect(screen.getByRole('textbox', { name: '科目' })).toHaveValue('数学竞赛进阶')
    expect(screen.queryByRole('button', { name: '编辑题目' })).not.toBeInTheDocument()
  })

  it('shares the drawer discard decision with registered route navigation', async () => {
    const user = userEvent.setup()
    let attempt: NavigationAttempt | undefined
    let contextAttempt: NavigationAttempt | undefined
    const unregisterNavigation = vi.fn()
    const unregisterContext = vi.fn()
    const registerNavigation = vi.fn((candidate: NavigationAttempt) => {
      attempt = candidate
      return unregisterNavigation
    })
    const registerContextTransition = vi.fn((candidate: NavigationAttempt) => {
      contextAttempt = candidate
      return unregisterContext
    })
    const view = render(ProblemDetailDrawer, {
      props: {
        loading: false,
        registerNavigation,
        registerContextTransition,
        detail: {
          id: 'problem-1', subject: '数学', note: '', tags: [], status: 'active', timeLimitSeconds: null, updatedAtUtcMs: null, assets: [],
        },
      },
    })

    await user.click(screen.getByRole('button', { name: '编辑题目' }))
    const subject = screen.getByRole('textbox', { name: '科目' })
    await user.type(subject, '竞赛')
    expect(contextAttempt).toBe(attempt)
    const cancelled = attempt!()
    expect(await screen.findByRole('alertdialog', { name: '放弃尚未保存的修改？' })).toBeVisible()
    await user.click(screen.getByRole('button', { name: '继续编辑' }))
    await expect(cancelled).resolves.toBe(false)
    expect(subject).toHaveValue('数学竞赛')

    const confirmed = attempt!()
    await user.click(await screen.findByRole('button', { name: '放弃修改' }))
    await expect(confirmed).resolves.toBe(true)
    view.unmount()
    expect(registerNavigation).toHaveBeenCalledOnce()
    expect(registerContextTransition).toHaveBeenCalledOnce()
    expect(unregisterNavigation).toHaveBeenCalledOnce()
    expect(unregisterContext).toHaveBeenCalledOnce()
  })

  it('blocks dirty drawer exits while its update is saving', async () => {
    const user = userEvent.setup()
    let attempt: NavigationAttempt | undefined
    const view = render(ProblemDetailDrawer, {
      props: {
        loading: false,
        saving: false,
        registerNavigation: (candidate: NavigationAttempt) => {
          attempt = candidate
          return () => undefined
        },
        detail: {
          id: 'problem-1', subject: '数学', note: '', tags: [], status: 'active', timeLimitSeconds: null, updatedAtUtcMs: null, assets: [],
        },
      },
    })

    await user.click(screen.getByRole('button', { name: '编辑题目' }))
    await user.type(screen.getByRole('textbox', { name: '科目' }), '竞赛')
    await view.rerender({ saving: true })

    await expect(attempt!()).resolves.toBe(false)
    expect(await screen.findByRole('alert')).toHaveTextContent('题目操作正在完成，请等待完成后再离开。')
    expect(screen.queryByRole('alertdialog')).not.toBeInTheDocument()
    await user.click(screen.getByRole('button', { name: '关闭题目详情' }))
    expect(view.emitted('close')).toBeUndefined()
    expect(screen.queryByRole('alertdialog')).not.toBeInTheDocument()
  })

  it('blocks a busy detail transition even when there are no dirty edits', async () => {
    let attempt: NavigationAttempt | undefined
    const view = render(ProblemDetailDrawer, {
      props: {
        loading: false,
        saving: true,
        registerNavigation: (candidate: NavigationAttempt) => {
          attempt = candidate
          return () => undefined
        },
        detail: {
          id: 'problem-1', subject: '数学', note: '', tags: [], status: 'active', timeLimitSeconds: null, updatedAtUtcMs: null, assets: [],
        },
      },
    })

    await expect(attempt!()).resolves.toBe(false)
    expect(await screen.findByRole('alert')).toHaveTextContent('题目操作正在完成，请等待完成后再离开。')
    expect(screen.queryByRole('alertdialog')).not.toBeInTheDocument()

    await view.rerender({ saving: false })
    await expect(attempt!()).resolves.toBe(true)
  })

  it('edits and emits a persistent answer time limit', async () => {
    const user = userEvent.setup()
    const view = render(ProblemDetailDrawer, {
      props: {
        loading: false,
        detail: {
          id: 'problem-1', subject: '物理', note: '', tags: ['力学'], status: 'active', timeLimitSeconds: 60, updatedAtUtcMs: null, assets: [],
        },
      },
    })

    await user.click(screen.getByRole('button', { name: '编辑题目' }))
    const input = screen.getByRole('spinbutton', { name: '答题时限（秒）' })
    await user.clear(input)
    await user.type(input, '180')
    await user.type(screen.getByRole('textbox', { name: '标签' }), '受力{Enter}')
    await user.click(screen.getByRole('button', { name: '保存修改' }))

    expect(view.emitted('update')).toEqual([[
      { problemId: 'problem-1', subject: '物理', note: '', tags: ['力学', '受力'], timeLimitSeconds: 180 },
    ]])
  })

  it('rejects an out-of-range answer time limit before saving', async () => {
    const user = userEvent.setup()
    const view = render(ProblemDetailDrawer, {
      props: {
        loading: false,
        detail: {
          id: 'problem-1', subject: '物理', note: '', tags: [], status: 'active', timeLimitSeconds: null, updatedAtUtcMs: null, assets: [],
        },
      },
    })

    await user.click(screen.getByRole('button', { name: '编辑题目' }))
    const input = screen.getByRole('spinbutton', { name: '答题时限（秒）' })
    await user.type(input, '0')

    expect(input).toHaveAttribute('aria-invalid', 'true')
    expect(screen.getByRole('alert')).toBeVisible()
    expect(screen.getByRole('button', { name: '保存修改' })).toBeDisabled()
    expect(view.emitted('update')).toBeUndefined()
  })
})
