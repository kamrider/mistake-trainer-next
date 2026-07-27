import { fireEvent, render, screen } from '@testing-library/vue'
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
    renderCard()

    expect(screen.getByRole('img', { name: '题目上半部分.png' })).toBeVisible()
    await user.click(screen.getByLabelText('题目下半部分.png'))
    expect(screen.getByRole('img', { name: '题目下半部分.png' })).toBeVisible()

    await user.click(screen.getByRole('button', { name: '翻到答案' }))
    expect(screen.getByRole('img', { name: '完整答案.png' })).toBeVisible()
    expect(screen.getByRole('button', { name: '翻回题面' })).toBeVisible()
  })

  it('starts a drag from the visible card image and keeps return available as a button and shortcut', async () => {
    const view = renderCard()
    const visibleImage = screen.getByRole('img', { name: '题目上半部分.png' })

    await fireEvent.pointerDown(visibleImage, { pointerId: 7, button: 0, clientX: 10, clientY: 10 })
    expect(view.emitted('pointerStart')).toEqual([['q-1', expect.objectContaining({ pointerId: 7 })]])

    const thumbnail = view.container.querySelector<HTMLElement>('[data-capture-item-id="q-1"]')!
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
    const inner = view.container.querySelector('.card-inner') as HTMLElement

    inner.focus()
    await user.keyboard('{Control>}{ArrowDown}{/Control}')
    expect(view.emitted('navigateDraft')).toEqual([['next']])
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
