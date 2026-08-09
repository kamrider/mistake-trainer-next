import { fireEvent, render, screen, waitFor } from '@testing-library/vue'
import userEvent from '@testing-library/user-event'
import { describe, expect, it, vi } from 'vitest'
import type { CaptureDraftSummary, CaptureItemSummary } from '../../../shared/api/bindings'
import CaptureDraftCard from './CaptureDraftCard.vue'

const draft: CaptureDraftSummary = {
  id: 'draft-1',
  position: 0,
  subject: '数学',
  tags: [],
  note: '',
  questionItemIds: ['q-1', 'q-2'],
  answerItemIds: ['a-1'],
  ready: true,
}

const items: CaptureItemSummary[] = [
  { id: 'q-1', sourceName: '题目上半部分.png', sourceSequence: 0, mediaType: 'image/png', byteLength: 1, width: 1200, height: 900, stagedRole: 'question', draftId: draft.id, role: 'question', position: 0, cropDerivationId: null, cropSourceItemId: null },
  { id: 'q-2', sourceName: '题目下半部分.png', sourceSequence: 1, mediaType: 'image/png', byteLength: 1, width: 1200, height: 900, stagedRole: 'question', draftId: draft.id, role: 'question', position: 1, cropDerivationId: null, cropSourceItemId: null },
  { id: 'a-1', sourceName: '完整答案.png', sourceSequence: 2, mediaType: 'image/png', byteLength: 1, width: 1200, height: 900, stagedRole: 'answer', draftId: draft.id, role: 'answer', position: 0, cropDerivationId: null, cropSourceItemId: null },
]

function renderCard(card = draft, cardItems = items) {
  vi.stubGlobal('IntersectionObserver', class {
    observe() {}
    disconnect() {}
  })
  return render(CaptureDraftCard, {
    props: {
      draft: card,
      draftIndex: 0,
      items: cardItems,
      previews: {
        'q-1': 'data:image/png;base64,cTE=',
        'q-2': 'data:image/png;base64,cTI=',
        'a-1': 'data:image/png;base64,YTE=',
      },
      selected: true,
      busy: false,
      subjectOptions: ['语文', '数学', '英语', '物理', '化学'],
    },
  })
}

describe('CaptureDraftCard', () => {
  it('shows a readable question face and flips to the answer', async () => {
    const user = userEvent.setup()
    const view = renderCard()

    expect(screen.getByRole('img', { name: '题目上半部分.png' })).toBeVisible()
    expect(view.container.querySelectorAll('[data-crop-item-id="q-1"]')).toHaveLength(2)
    await user.click(screen.getByLabelText('题目下半部分.png'))
    expect(screen.getByRole('img', { name: '题目下半部分.png' })).toBeVisible()

    await user.click(screen.getByRole('button', { name: '翻到答案' }))
    expect(screen.getByRole('img', { name: '完整答案.png' })).toBeVisible()
    expect(screen.getByRole('button', { name: '翻回题面' })).toBeVisible()
  })

  it('contains expanded-image focus, scroll, Escape, and launcher return', async () => {
    const user = userEvent.setup()
    const previousOverflow = document.body.style.overflow
    document.body.style.overflow = 'auto'
    const view = renderCard()

    try {
      const launcher = screen.getByRole('button', { name: '放大查看 题目上半部分.png' })
      await user.click(launcher)
      const dialog = screen.getByRole('dialog', { name: '大图查看 题目上半部分.png' })
      const close = screen.getByRole('button', { name: '关闭大图' })

      await waitFor(() => expect(close).toHaveFocus())
      expect(document.body.style.overflow).toBe('hidden')
      const tab = new KeyboardEvent('keydown', {
        key: 'Tab',
        bubbles: true,
        cancelable: true,
      })
      close.dispatchEvent(tab)
      expect(tab.defaultPrevented).toBe(true)
      expect(close).toHaveFocus()

      await fireEvent.keyDown(dialog, { key: 'Escape' })
      await waitFor(() => expect(launcher).toHaveFocus())
      expect(screen.queryByRole('dialog', { name: '大图查看 题目上半部分.png' })).not.toBeInTheDocument()
      expect(document.body.style.overflow).toBe('auto')
    }
    finally {
      view.unmount()
      document.body.style.overflow = previousOverflow
    }
  })

  it('marks derived crop controls as return-focus results, not crop launchers', () => {
    const derivedItems = items.map(item => item.id === 'q-1'
      ? { ...item, cropDerivationId: 'crop-1', cropSourceItemId: 'source-1' }
      : item)
    renderCard(draft, derivedItems)

    const resultControls = screen.getAllByRole('button', { name: '恢复裁剪前原图' })
    expect(resultControls).toHaveLength(2)
    for (const control of resultControls) {
      expect(control).not.toHaveAttribute('data-crop-item-id')
      expect(control).toHaveAttribute('data-crop-result-item-id', 'q-1')
    }
  })

  it('starts a drag from the visible card image and keeps return available as a button and shortcut', async () => {
    const view = renderCard()
    const visibleImage = screen.getByRole('img', { name: '题目上半部分.png' })

    await fireEvent.pointerDown(visibleImage, { pointerId: 7, button: 0, clientX: 10, clientY: 10 })
    expect(view.emitted('pointerStart')).toEqual([['q-1', expect.objectContaining({ pointerId: 7 })]])

    const thumbnail = view.container.querySelector<HTMLElement>('[data-capture-item-id="q-1"] .thumbnail-activate')!
    thumbnail.focus()
    await fireEvent.keyDown(thumbnail, { key: 'Delete' })
    expect(view.emitted('returnItem')).toEqual([['q-1']])
    await userEvent.click(screen.getByRole('button', { name: '将当前图片移回素材库' }))
    expect(view.emitted('returnItem')).toEqual([['q-1'], ['q-1']])
    expect(screen.queryByRole('button', { name: '撤销这张卡' })).not.toBeInTheDocument()
  })

  it('reveals the exact newly added image and flips to its role', async () => {
    const view = renderCard()

    await view.rerender({ revealItemId: 'a-1', revealRequestKey: 1 })

    expect(screen.getByRole('button', { name: '翻回题面' })).toBeVisible()
    expect(screen.getByRole('img', { name: '完整答案.png' })).toBeVisible()

    await view.rerender({ revealItemId: 'q-2', revealRequestKey: 2 })

    expect(screen.getByRole('button', { name: '翻到答案' })).toBeVisible()
    expect(screen.getByRole('img', { name: '题目下半部分.png' })).toBeVisible()
  })

  it('supports keyboard navigation and role shortcuts on the focused thumbnail', async () => {
    const user = userEvent.setup()
    const view = renderCard()
    const firstQuestion = screen.getByLabelText('题目上半部分.png')

    firstQuestion.focus()
    await user.keyboard('{ArrowRight}')
    expect(view.emitted('preview')).toContainEqual(['q-2'])

    await user.keyboard('a')
    expect(view.emitted('changeItemRole')).toContainEqual(['q-2', 'answer', 1])
  })

  it('reorders multiple images with shift plus the arrow keys', async () => {
    const user = userEvent.setup()
    const view = renderCard()
    const secondQuestion = screen.getByLabelText('题目下半部分.png')

    secondQuestion.focus()
    await user.keyboard('{Shift>}{ArrowLeft}{/Shift}')

    expect(view.emitted('changeItemRole')).toContainEqual(['q-2', 'question', 0])
  })

  it('moves between adjacent cards with ctrl or command arrows', async () => {
    const user = userEvent.setup()
    const view = renderCard()
    const firstQuestion = screen.getByLabelText('题目上半部分.png')
    const inner = view.container.querySelector('.card-inner') as HTMLElement

    firstQuestion.focus()
    await user.keyboard('{Control>}{ArrowDown}{/Control}')
    expect(view.emitted('navigateDraft')).toEqual([['next']])
    expect(view.emitted('preview')).not.toContainEqual(['q-2'])

    inner.focus()
    await user.keyboard('{Control>}{ArrowUp}{/Control}')
    expect(view.emitted('navigateDraft')).toEqual([['next'], ['previous']])
  })

  it('explains why a card is incomplete and corrects an assigned image in place', async () => {
    const user = userEvent.setup()
    const incomplete: CaptureDraftSummary = {
      ...draft,
      subject: '',
      answerItemIds: [],
      ready: false,
    }
    const view = renderCard(incomplete, items.filter(item => item.id !== 'a-1'))

    expect(screen.getByText('缺答案 · 缺科目')).toBeVisible()
    expect(screen.queryByRole('button', { name: '翻到答案' })).not.toBeInTheDocument()
    await user.click(screen.getByRole('button', { name: '添加答案' }))
    expect(view.emitted('requestAnswer')).toEqual([['draft-1']])
    await user.click(screen.getByRole('button', { name: '把当前题图转为答案' }))
    expect(view.emitted('changeItemRole')).toEqual([['q-1', 'answer', 0]])
  })

})
