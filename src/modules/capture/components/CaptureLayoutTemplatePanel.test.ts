import { fireEvent, render, screen, waitFor, within } from '@testing-library/vue'
import userEvent from '@testing-library/user-event'
import { describe, expect, it } from 'vitest'
import CaptureLayoutTemplatePanel from './CaptureLayoutTemplatePanel.vue'

function renderPanel(overrides: Partial<{
  itemCount: number
  draftCount: number
  affectedNoteCount: number
  busy: boolean
}> = {}) {
  return render(CaptureLayoutTemplatePanel, {
    props: {
      itemCount: 6,
      draftCount: 0,
      affectedNoteCount: 0,
      busy: false,
      ...overrides,
    },
  })
}

describe('CaptureLayoutTemplatePanel', () => {
  it('applies the default template immediately when no cards exist', async () => {
    const user = userEvent.setup()
    const view = renderPanel()

    await user.click(screen.getByRole('button', { name: '按模板生成题卡' }))

    expect(view.emitted('apply')).toEqual([['alternating', 1, 1, null]])
    expect(screen.queryByRole('alertdialog')).not.toBeInTheDocument()
  })

  it('explains destructive impact and waits for explicit confirmation', async () => {
    const user = userEvent.setup()
    const view = renderPanel({ itemCount: 5, draftCount: 2, affectedNoteCount: 1 })

    await user.click(screen.getByRole('button', { name: '重新分组全部图片' }))

    expect(view.emitted('apply')).toBeUndefined()
    const dialog = screen.getByRole('alertdialog', { name: '确认重新分组全部图片' })
    expect(within(dialog).getByText(/2 张题卡会被重新生成/)).toBeVisible()
    expect(within(dialog).getByText(/1 张含标签或笔记/)).toBeVisible()
    expect(within(dialog).getByText(/5 张图片都会保留/)).toBeVisible()

    await user.click(within(dialog).getByRole('button', { name: '确认重新分组' }))
    expect(view.emitted('apply')).toEqual([['alternating', 1, 1, null]])
  })

  it('tracks the current batch midpoint for the split template', async () => {
    const user = userEvent.setup()
    const view = renderPanel({ itemCount: 5 })

    await user.selectOptions(screen.getByRole('combobox', { name: '整理模板' }), 'split')
    expect(screen.getByRole('spinbutton', { name: '从第几张分开' })).toHaveValue(3)

    await view.rerender({ itemCount: 8 })
    expect(screen.getByRole('spinbutton', { name: '从第几张分开' })).toHaveValue(4)
    await user.click(screen.getByRole('button', { name: '按模板生成题卡' }))

    expect(view.emitted('apply')).toEqual([['split', 1, 1, 4]])
  })

  it('blocks invalid alternating counts with local accessible feedback', async () => {
    const user = userEvent.setup()
    const view = renderPanel({ draftCount: 2 })
    const questionCount = screen.getByRole('spinbutton', { name: '题图/题' })
    const answerCount = screen.getByRole('spinbutton', { name: '答案/题' })
    const launcher = screen.getByRole('button', { name: '重新分组全部图片' })

    for (const invalidValue of ['', '0', '1.5', '11']) {
      await user.clear(questionCount)
      if (invalidValue) await user.type(questionCount, invalidValue)
      expect(screen.getByRole('alert')).toHaveTextContent('题图/题和答案/题必须是 1–10 的整数。')
      expect(questionCount).toHaveAttribute('aria-invalid', 'true')
      expect(questionCount).toHaveAttribute('aria-describedby', 'layout-validation-message')
      expect(launcher).toBeDisabled()
    }

    await user.clear(questionCount)
    await user.type(questionCount, '10')
    await user.clear(answerCount)
    await user.type(answerCount, '11')
    expect(answerCount).toHaveAttribute('aria-invalid', 'true')
    expect(launcher).toBeDisabled()
    await user.click(launcher)
    expect(screen.queryByRole('alertdialog')).not.toBeInTheDocument()
    expect(view.emitted('apply')).toBeUndefined()
  })

  it('rejects zero, fractional, and out-of-range split positions', async () => {
    const user = userEvent.setup()
    const view = renderPanel({ itemCount: 6 })
    await user.selectOptions(screen.getByRole('combobox', { name: '整理模板' }), 'split')
    const split = screen.getByRole('spinbutton', { name: '从第几张分开' })
    const launcher = screen.getByRole('button', { name: '按模板生成题卡' })

    for (const invalidValue of ['0', '1.5', '7']) {
      await user.clear(split)
      await user.type(split, invalidValue)

      expect(screen.getByRole('alert')).toHaveTextContent('分开位置必须是 1–6 的整数。')
      expect(split).toHaveAttribute('aria-invalid', 'true')
      expect(split).toHaveAttribute('aria-describedby', 'layout-validation-message')
      expect(launcher).toBeDisabled()
    }

    await user.click(launcher)
    expect(view.emitted('apply')).toBeUndefined()
  })

  it('does not let invalid hidden fields block manual layout', async () => {
    const user = userEvent.setup()
    const view = renderPanel()
    await user.clear(screen.getByRole('spinbutton', { name: '题图/题' }))
    expect(screen.getByRole('alert')).toBeVisible()

    await user.selectOptions(screen.getByRole('combobox', { name: '整理模板' }), 'manual')

    expect(screen.queryByRole('alert')).not.toBeInTheDocument()
    const launcher = screen.getByRole('button', { name: '按模板生成题卡' })
    expect(launcher).toBeEnabled()
    await user.click(launcher)
    expect(view.emitted('apply')).toEqual([['manual', 1, 1, null]])
  })

  it('contains focus, locks scrolling, restores the launcher, and blocks dismissal while busy', async () => {
    const user = userEvent.setup()
    const previousOverflow = document.body.style.overflow
    document.body.style.overflow = 'auto'
    const view = renderPanel({ draftCount: 2 })

    try {
      const launcher = screen.getByRole('button', { name: '重新分组全部图片' })
      await user.click(launcher)
      const dialog = screen.getByRole('alertdialog', { name: '确认重新分组全部图片' })
      const confirm = within(dialog).getByRole('button', { name: '确认重新分组' })
      const back = within(dialog).getByRole('button', { name: '返回' })

      await waitFor(() => expect(back).toHaveFocus())
      expect(document.body.style.overflow).toBe('hidden')
      expect(launcher.closest('.layout-bar')).toHaveAttribute('inert')
      confirm.focus()
      await fireEvent.keyDown(confirm, { key: 'Tab', shiftKey: true })
      expect(back).toHaveFocus()
      await fireEvent.keyDown(back, { key: 'Tab' })
      expect(confirm).toHaveFocus()

      await view.rerender({ busy: true })
      await fireEvent.keyDown(dialog, { key: 'Escape' })
      expect(screen.getByRole('alertdialog')).toBeVisible()
      expect(confirm).toBeDisabled()
      expect(back).toBeDisabled()

      await view.rerender({ busy: false })
      await fireEvent.keyDown(dialog, { key: 'Escape' })
      await waitFor(() => expect(launcher).toHaveFocus())
      expect(screen.queryByRole('alertdialog')).not.toBeInTheDocument()
      expect(document.body.style.overflow).toBe('auto')
      expect(launcher.closest('.layout-bar')).not.toHaveAttribute('inert')
    }
    finally {
      view.unmount()
      document.body.style.overflow = previousOverflow
    }
  })

  it('releases the document boundary when removed while confirmation is open', async () => {
    const user = userEvent.setup()
    const previousOverflow = document.body.style.overflow
    document.body.style.overflow = 'auto'
    const view = renderPanel({ draftCount: 2 })

    try {
      await user.click(screen.getByRole('button', { name: '重新分组全部图片' }))
      await waitFor(() => expect(document.body.style.overflow).toBe('hidden'))

      view.unmount()

      expect(document.body.style.overflow).toBe('auto')
    }
    finally {
      view.unmount()
      document.body.style.overflow = previousOverflow
    }
  })
})
