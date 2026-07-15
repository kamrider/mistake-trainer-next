import { render, screen } from '@testing-library/vue'
import userEvent from '@testing-library/user-event'
import { beforeAll, describe, expect, it, vi } from 'vitest'
import type { CaptureBatchDetail, CaptureBatchSummary } from '../../../shared/api/bindings'
import CaptureWorkspace from './CaptureWorkspace.vue'

const batch: CaptureBatchSummary = {
  id: 'batch-1',
  subject: '数学',
  state: 'collecting',
  itemCount: 2,
  draftCount: 0,
  readyCount: 0,
  updatedAtUtcMs: 100,
  revision: 3,
}

function renderWorkspace(detail?: CaptureBatchDetail) {
  return render(CaptureWorkspace, {
    props: {
      batches: detail ? [detail.batch] : [batch],
      detail,
      previews: {},
      busy: false,
      errorMessage: '',
      desktopAvailable: true,
    },
  })
}

beforeAll(() => {
  vi.stubGlobal('IntersectionObserver', class {
    observe() {}
    disconnect() {}
  })
})

describe('CaptureWorkspace Next', () => {
  it('creates and reopens persistent batches', async () => {
    const user = userEvent.setup()
    const view = renderWorkspace()

    await user.type(screen.getByPlaceholderText('科目，例如：数学（可选）'), '物理')
    await user.click(screen.getByRole('button', { name: '新建批次' }))
    await user.click(screen.getAllByRole('button', { name: /数学/ })[0]!)

    expect(view.emitted('createBatch')).toEqual([['物理']])
    expect(view.emitted('openBatch')).toEqual([['batch-1']])
    expect(screen.getByText('2 张图片 · 0 道草稿')).toBeVisible()
  })

  it('offers phone, desktop and finish actions while collecting', async () => {
    const user = userEvent.setup()
    const detail: CaptureBatchDetail = {
      batch,
      items: [],
      drafts: [],
      unassignedItemIds: [],
    }
    detail.items.push({
      id: 'item-1',
      sourceName: 'question.png',
      sourceSequence: 0,
      mediaType: 'image/png',
      byteLength: 100,
      width: 800,
      height: 600,
      draftId: null,
      role: null,
      position: null,
    })
    const view = renderWorkspace(detail)

    await user.click(screen.getByRole('button', { name: /手机扫码/ }))
    await user.click(screen.getByRole('button', { name: /电脑批量选择/ }))
    await user.click(screen.getByRole('button', { name: /结束采集/ }))

    expect(view.emitted('mobileCapture')).toHaveLength(1)
    expect(view.emitted('importSelect')).toHaveLength(1)
    expect(view.emitted('finishCollecting')).toEqual([['数学']])
  })

  it('enables atomic commit only for ready drafts', async () => {
    const user = userEvent.setup()
    const organizing: CaptureBatchDetail = {
      batch: { ...batch, state: 'organizing', draftCount: 1, readyCount: 1 },
      items: [
        {
          id: 'q', sourceName: 'q.png', sourceSequence: 0, mediaType: 'image/png',
          byteLength: 100, width: 100, height: 100, draftId: 'draft', role: 'question', position: 0,
        },
        {
          id: 'a', sourceName: 'a.png', sourceSequence: 1, mediaType: 'image/png',
          byteLength: 100, width: 100, height: 100, draftId: 'draft', role: 'answer', position: 0,
        },
      ],
      drafts: [{
        id: 'draft', position: 0, subject: '数学', tags: [], note: '',
        questionItemIds: ['q'], answerItemIds: ['a'], ready: true,
      }],
      unassignedItemIds: [],
    }
    const view = renderWorkspace(organizing)

    await user.click(screen.getByRole('button', { name: /保存全部就绪题/ }))

    expect(screen.getByText('1 道已就绪')).toBeVisible()
    expect(view.emitted('commitReady')).toHaveLength(1)
  })
})
