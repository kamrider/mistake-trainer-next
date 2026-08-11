import { render, screen } from '@testing-library/vue'
import userEvent from '@testing-library/user-event'
import { describe, expect, it } from 'vitest'
import LibraryBulkMetadataDialog from './LibraryBulkMetadataDialog.vue'

describe('LibraryBulkMetadataDialog', () => {
  it('previews selection, requires a change, and emits normalized metadata once', async () => {
    const user = userEvent.setup()
    const view = render(LibraryBulkMetadataDialog, {
      props: { open: true, selectedCount: 3 },
    })

    expect(screen.getByRole('dialog', { name: '修改已选 3 道题' })).toBeVisible()
    await user.click(screen.getByRole('button', { name: '确认批量修改' }))
    expect(screen.getByRole('alert')).toHaveTextContent('请至少填写一项要修改的内容。')
    expect(view.emitted('submit')).toBeUndefined()

    await user.type(screen.getByRole('textbox', { name: '添加标签' }), '重点，函数\n重点')
    await user.type(screen.getByRole('textbox', { name: '移除标签' }), '粗心, 旧标签')
    await user.click(screen.getByRole('button', { name: '确认批量修改' }))

    expect(view.emitted('submit')).toEqual([[{
      subject: null,
      addTags: ['重点', '函数'],
      removeTags: ['粗心', '旧标签'],
    }]])
  })

  it('supports replacing or clearing the subject and locks actions while saving', async () => {
    const user = userEvent.setup()
    const view = render(LibraryBulkMetadataDialog, {
      props: { open: true, selectedCount: 1 },
    })
    await user.click(screen.getByRole('checkbox', { name: '统一修改科目' }))
    await user.type(screen.getByRole('textbox', { name: '新科目' }), ' 代数 ')
    await user.click(screen.getByRole('button', { name: '确认批量修改' }))
    expect(view.emitted('submit')).toEqual([[{
      subject: '代数', addTags: [], removeTags: [],
    }]])

    await view.rerender({ busy: true })
    expect(screen.getByRole('button', { name: '正在批量修改…' })).toBeDisabled()
    expect(screen.getByRole('button', { name: '取消' })).toBeDisabled()
  })
})
