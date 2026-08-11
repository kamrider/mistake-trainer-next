import { render, screen } from '@testing-library/vue'
import userEvent from '@testing-library/user-event'
import { describe, expect, it } from 'vitest'
import LibraryFilterPanel from './LibraryFilterPanel.vue'

describe('LibraryFilterPanel', () => {
  it('exposes keyboard-accessible filters and emits selected values', async () => {
    const user = userEvent.setup()
    const view = render(LibraryFilterPanel, {
      props: {
        modelValue: { subjects: [], tags: [], reviewState: 'any', answerState: 'any' },
        subjectOptions: ['数学', '物理'],
        tagOptions: ['错因·计算失误'],
      },
    })

    const trigger = screen.getByRole('button', { name: '更多筛选' })
    trigger.focus()
    await user.keyboard('{Enter}')
    expect(screen.getByRole('region', { name: '高级筛选条件' })).toBeVisible()
    await user.click(screen.getByRole('checkbox', { name: '数学' }))

    expect(view.emitted('update:modelValue')?.at(-1)).toEqual([{
      subjects: ['数学'], tags: [], reviewState: 'any', answerState: 'any',
    }])
  })

  it('renders removable active chips and clears explicitly', async () => {
    const user = userEvent.setup()
    const view = render(LibraryFilterPanel, {
      props: {
        modelValue: {
          subjects: ['数学'], tags: ['错因·计算失误'],
          reviewState: 'recently_forgotten', answerState: 'missing_answer',
        },
      },
    })

    await user.click(screen.getByRole('button', { name: '移除科目 数学' }))
    expect(view.emitted('update:modelValue')).toEqual([[{
      subjects: [], tags: ['错因·计算失误'],
      reviewState: 'recently_forgotten', answerState: 'missing_answer',
    }]])
    await user.click(screen.getByRole('button', { name: '清除全部筛选' }))
    expect(view.emitted('update:modelValue')?.at(-1)).toEqual([{
      subjects: [], tags: [], reviewState: 'any', answerState: 'any',
    }])
  })
})
