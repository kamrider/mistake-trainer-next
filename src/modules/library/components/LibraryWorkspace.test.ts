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
        loading: false,
        problems: [
          {
            id: 'problem-1',
            subject: '数学',
            note: '奇函数定义域容易漏掉零点。',
            status: 'active',
            questionAssetCount: 2,
            answerAssetCount: 1,
            updatedAtUtcMs: 1_700_000_000_000,
          },
        ],
      },
    })

    expect(screen.getByRole('heading', { name: '题库' })).toBeVisible()
    expect(screen.getByText('数学')).toBeVisible()
    expect(screen.getByText('2 张题图')).toBeVisible()
    expect(screen.getByText('1 张答案')).toBeVisible()
    await user.click(screen.getByRole('button', { name: '录入新错题' }))

    expect(view.emitted('capture')).toHaveLength(1)
  })

  it('shows an actionable empty state instead of placeholder skeletons', () => {
    render(LibraryWorkspace, {
      props: {
        profileName: '本机学习档案',
        status: 'active',
        loading: false,
        problems: [],
      },
    })

    expect(screen.getByText('题库还是空的')).toBeVisible()
    expect(screen.getByRole('button', { name: '录入第一道错题' })).toBeVisible()
  })
})
