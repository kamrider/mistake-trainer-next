import { render, screen } from '@testing-library/vue'
import userEvent from '@testing-library/user-event'
import { describe, expect, it } from 'vitest'
import LibraryWorkspace from './LibraryWorkspace.vue'

describe('LibraryWorkspace', () => {
  it('renders real problem summaries and opens capture from the primary action', async () => {
    const user = userEvent.setup()
    const view = render(LibraryWorkspace, {
      props: {
        profileName: '本机学习档案',
        status: 'active',
        search: '',
        loading: false,
        problems: [
          {
            id: 'problem-1',
            subject: '数学',
            note: '奇函数定义域容易漏掉零点。',
            tags: ['函数', '粗心', '定义域', '基础'],
            status: 'active',
            questionAssetCount: 2,
            answerAssetCount: 1,
            questionPreviewDataUrl: 'data:image/png;base64,preview',
            updatedAtUtcMs: 1_700_000_000_000,
          },
        ],
      },
    })

    expect(screen.getByRole('heading', { name: '题库' })).toBeVisible()
    expect(screen.getByText('数学')).toBeVisible()
    expect(screen.getByText('2 张题图')).toBeVisible()
    expect(screen.getByText('1 张答案')).toBeVisible()
    expect(screen.getByText('函数')).toBeVisible()
    expect(screen.getByText('粗心')).toBeVisible()
    expect(screen.getByText('+1')).toBeVisible()
    expect(screen.getByRole('img', { name: '数学 题图预览' })).toHaveAttribute('src', 'data:image/png;base64,preview')
    expect(screen.queryByText('查看详情')).not.toBeInTheDocument()
    await user.click(screen.getByRole('button', { name: '打开 数学 错题详情' }))
    expect(view.emitted('openDetail')).toEqual([['problem-1']])
    expect(screen.queryByRole('checkbox')).not.toBeInTheDocument()
    await user.click(screen.getByRole('button', { name: '批量管理' }))
    expect(screen.getByRole('checkbox', { name: '选择 数学 错题' })).toBeVisible()
    expect(screen.getByRole('button', { name: '全选当前结果' })).toBeVisible()
    await user.click(screen.getByRole('button', { name: '添加素材' }))

    expect(view.emitted('capture')).toHaveLength(1)
  })

  it('shows an actionable empty state instead of placeholder skeletons', () => {
    render(LibraryWorkspace, {
      props: {
        profileName: '本机学习档案',
        status: 'active',
        search: '',
        loading: false,
        problems: [],
      },
    })

    expect(screen.getByText('题库还是空的')).toBeVisible()
    expect(screen.getByRole('button', { name: '添加第一份素材' })).toBeVisible()
  })

  it('emits searchable text and explains an empty search result', async () => {
    const user = userEvent.setup()
    const view = render(LibraryWorkspace, {
      props: {
        profileName: '本机学习档案',
        status: 'active',
        search: '奇函数',
        loading: false,
        problems: [],
      },
    })

    expect(screen.getByText('没有找到匹配的错题')).toBeVisible()
    const input = screen.getByRole('searchbox', { name: '搜索题库' })
    await user.clear(input)
    await user.type(input, '物理')
    expect(view.emitted('searchChange')?.at(-1)).toEqual(['物理'])
  })

  it('turns an ordered selection into a visible training dock', async () => {
    const user = userEvent.setup()
    const problems = [
      {
        id: 'problem-1', subject: '数学', note: '', tags: [], status: 'active' as const,
        questionAssetCount: 1, answerAssetCount: 1, questionPreviewDataUrl: null, updatedAtUtcMs: 1,
      },
      {
        id: 'problem-2', subject: '物理', note: '', tags: [], status: 'active' as const,
        questionAssetCount: 1, answerAssetCount: 1, questionPreviewDataUrl: null, updatedAtUtcMs: 2,
      },
    ]
    const view = render(LibraryWorkspace, {
      props: {
        profileName: '本机学习档案', status: 'active', search: '', loading: false,
        problems, selectedProblemIds: ['problem-2', 'problem-1'],
      },
    })

    const problemCards = screen.getAllByRole('article')
    for (const card of problemCards) expect(card).not.toHaveAttribute('aria-selected')
    for (const checkbox of screen.getAllByRole('checkbox')) expect(checkbox).toBeChecked()
    const selectionStatus = screen.getByRole('status')
    expect(selectionStatus).toHaveAttribute('aria-live', 'polite')
    expect(selectionStatus).toHaveAttribute('aria-atomic', 'true')
    expect(selectionStatus).toHaveTextContent(/2\s*道题已放入本轮卡组/)

    expect(screen.getByRole('button', { name: '开始训练 2 道题' })).toBeVisible()
    expect(screen.getByRole('button', { name: '模拟考试 2 道题' })).toBeVisible()
    await user.click(screen.getByRole('button', { name: '开始训练 2 道题' }))
    expect(view.emitted('trainSelection')).toHaveLength(1)
    await user.click(screen.getByRole('button', { name: '模拟考试 2 道题' }))
    expect(view.emitted('startExam')).toHaveLength(1)

    await user.click(screen.getByRole('button', { name: '清空选择' }))
    expect(view.emitted('clearSelection')).toHaveLength(1)

    await view.rerender({ startingExperience: 'review' })
    expect(screen.getByRole('button', { name: '正在整理训练卡组…' })).toBeDisabled()
    expect(screen.getByRole('button', { name: '模拟考试 2 道题' })).toBeDisabled()

    await view.rerender({ startingExperience: null, selectedProblemIds: ['problem-1'] })
    expect(selectionStatus).toHaveTextContent(/1\s*道题已放入本轮卡组/)
    expect(screen.getByRole('checkbox', { name: '选择 数学 错题' })).toBeChecked()
    expect(screen.getByRole('checkbox', { name: '选择 物理 错题' })).not.toBeChecked()
  })

  it('locks conflicting controls and identifies the active batch status transaction', async () => {
    const problems = [
      {
        id: 'problem-1', subject: '数学', note: '', tags: [], status: 'active' as const,
        questionAssetCount: 1, answerAssetCount: 1, questionPreviewDataUrl: null, updatedAtUtcMs: 1,
      },
    ]
    const view = render(LibraryWorkspace, {
      props: {
        profileName: '本机学习档案', status: 'active', search: '', loading: false,
        problems, selectedProblemIds: ['problem-1'],
      },
    })

    await view.rerender({ changingBatchStatus: 'trashed' })

    const activeAction = screen.getByRole('button', { name: '正在移入回收站…' })
    expect(activeAction).toBeDisabled()
    expect(activeAction.querySelector('.spin')).toBeInTheDocument()
    expect(screen.getByRole('button', { name: '开始训练 1 道题' })).toBeDisabled()
    expect(screen.getByRole('button', { name: '模拟考试 1 道题' })).toBeDisabled()
    expect(screen.getByRole('button', { name: '归档' })).toBeDisabled()
    expect(screen.getByRole('button', { name: '清空选择' })).toBeDisabled()
    expect(screen.getByRole('button', { name: '全选当前结果' })).toBeDisabled()
    expect(screen.getByRole('button', { name: '退出批量管理' })).toBeDisabled()
    expect(screen.getByRole('button', { name: '正在学习' })).toBeDisabled()
    expect(screen.getByRole('button', { name: '已归档' })).toBeDisabled()
    expect(screen.getByRole('button', { name: '回收站' })).toBeDisabled()
    expect(screen.getByRole('searchbox', { name: '搜索题库' })).toBeDisabled()
    expect(screen.getByRole('checkbox', { name: '选择 数学 错题' })).toBeDisabled()
    expect(screen.getByRole('button', { name: '打开 数学 错题详情' })).toBeDisabled()
  })
})
