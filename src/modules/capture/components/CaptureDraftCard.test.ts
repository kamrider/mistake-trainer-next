import { render, screen } from '@testing-library/vue'
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
  { id: 'q-1', sourceName: '题目上半部分.png', sourceSequence: 0, mediaType: 'image/png', byteLength: 1, width: 1200, height: 900, stagedRole: 'question', draftId: draft.id, role: 'question', position: 0 },
  { id: 'q-2', sourceName: '题目下半部分.png', sourceSequence: 1, mediaType: 'image/png', byteLength: 1, width: 1200, height: 900, stagedRole: 'question', draftId: draft.id, role: 'question', position: 1 },
  { id: 'a-1', sourceName: '完整答案.png', sourceSequence: 2, mediaType: 'image/png', byteLength: 1, width: 1200, height: 900, stagedRole: 'answer', draftId: draft.id, role: 'answer', position: 0 },
]

function renderCard() {
  vi.stubGlobal('IntersectionObserver', class {
    observe() {}
    disconnect() {}
  })
  return render(CaptureDraftCard, {
    props: {
      draft,
      draftIndex: 0,
      items,
      previews: {
        'q-1': 'data:image/png;base64,cTE=',
        'q-2': 'data:image/png;base64,cTI=',
        'a-1': 'data:image/png;base64,YTE=',
      },
      selected: true,
      busy: false,
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

  it('returns the active image with one compact keyboard-accessible action', async () => {
    const user = userEvent.setup()
    const view = renderCard()

    await user.click(screen.getByRole('button', { name: '把当前题图移回待配对' }))
    expect(view.emitted('returnItem')).toEqual([['q-1']])
  })
})
