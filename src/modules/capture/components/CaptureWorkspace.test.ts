import { fireEvent, render, screen, waitFor, within } from '@testing-library/vue'
import userEvent from '@testing-library/user-event'
import { beforeAll, describe, expect, it, vi } from 'vitest'
import type {
  CaptureBatchDetail,
  CaptureBatchSummary,
  CaptureLanPreflight,
  CaptureRecognitionJob,
  OcrRecognitionFeatureStatus,
} from '../../../shared/api/bindings'
import CaptureWorkspace from './CaptureWorkspace.vue'

const batch: CaptureBatchSummary = {
  id: 'batch-1', subject: '数学', state: 'collecting', itemCount: 2,
  draftCount: 0, readyCount: 0, updatedAtUtcMs: 100, revision: 3,
}

const readyPreflight: CaptureLanPreflight = {
  supported: true,
  activeProfiles: ['public'],
  firewallRule: 'ready',
  canStart: true,
  needsNetworkChange: false,
  needsFirewallRepair: false,
}

function renderWorkspace(
  detail?: CaptureBatchDetail,
  preflight: CaptureLanPreflight | undefined = readyPreflight,
  preflightBusy = false,
  previews: Record<string, string> = {},
) {
  return render(CaptureWorkspace, {
    props: {
      batches: detail ? [detail.batch] : [batch], detail, previews, busy: false,
      errorMessage: '', desktopAvailable: true,
      lanAddresses: [{ label: 'Wi-Fi', address: '192.168.1.2' }],
      lanPreflight: preflight, lanPreflightBusy: preflightBusy, lanSession: undefined,
      saveState: 'saved', commitMessage: '',
      subjectOptions: ['语文', '数学', '英语', '物理', '化学'],
      captureSoundEnabled: true,
    },
  })
}

function collectingDetail(): CaptureBatchDetail {
  return {
    batch,
    items: [{ id: 'new', sourceName: 'new.png', sourceSequence: 0, mediaType: 'image/png', byteLength: 1, width: 800, height: 600, stagedRole: 'question', draftId: null, role: null, position: null, cropDerivationId: null, cropSourceItemId: null }],
    drafts: [],
    unassignedItemIds: ['new'],
  }
}

function organizingDetail(): CaptureBatchDetail {
  return {
    batch: { ...batch, state: 'organizing', itemCount: 5, draftCount: 2, readyCount: 1 },
    items: [
      { id: 'q1', sourceName: '第一题.png', sourceSequence: 0, mediaType: 'image/png', byteLength: 100, width: 1200, height: 900, stagedRole: 'question', draftId: 'd1', role: 'question', position: 0, cropDerivationId: null, cropSourceItemId: null },
      { id: 'a1', sourceName: '第一题答案.png', sourceSequence: 1, mediaType: 'image/png', byteLength: 100, width: 1200, height: 900, stagedRole: 'answer', draftId: 'd1', role: 'answer', position: 0, cropDerivationId: null, cropSourceItemId: null },
      { id: 'q2', sourceName: '第二题.png', sourceSequence: 2, mediaType: 'image/png', byteLength: 100, width: 1200, height: 900, stagedRole: 'question', draftId: 'd2', role: 'question', position: 0, cropDerivationId: null, cropSourceItemId: null },
      { id: 'q2b', sourceName: '第二题续图.png', sourceSequence: 3, mediaType: 'image/png', byteLength: 100, width: 1200, height: 900, stagedRole: 'question', draftId: 'd2', role: 'question', position: 1, cropDerivationId: null, cropSourceItemId: null },
      { id: 'loose', sourceName: '待配对超长文件名图片.png', sourceSequence: 4, mediaType: 'image/png', byteLength: 100, width: 1200, height: 900, stagedRole: 'question', draftId: null, role: null, position: null, cropDerivationId: null, cropSourceItemId: null },
    ],
    drafts: [
      { id: 'd1', position: 0, subject: '数学', tags: [], note: '', questionItemIds: ['q1'], answerItemIds: ['a1'], ready: true },
      { id: 'd2', position: 1, subject: '数学', tags: [], note: '', questionItemIds: ['q2', 'q2b'], answerItemIds: [], ready: false },
    ],
    unassignedItemIds: ['loose'],
  }
}

const recognitionReady: OcrRecognitionFeatureStatus = {
  state: 'ready',
  requiredComponentId: 'opencv_preprocess',
  detail: '智能切图使用内置本地视觉分析，不读取文字、不需要下载模型；确认后结果只进入素材牌库。',
}

function recognitionReviewJob(): CaptureRecognitionJob {
  return {
    id: 'job-1',
    batchId: 'batch-1',
    state: 'review',
    totalItems: 1,
    processedItems: 1,
    suggestions: [{
      id: 'suggestion-1',
      itemId: 'loose',
      confidenceBasisPoints: 7600,
      reviewBand: 'review',
      state: 'proposed',
      reasonCodes: ['weak_anchor'],
      regions: [{
        rect: { x: 0.1, y: 0.1, width: 0.8, height: 0.4 },
        role: 'question',
        groupSlot: 0,
        confidenceBasisPoints: 7600,
      }],
    }],
    createdAtUtcMs: 1,
    updatedAtUtcMs: 2,
  }
}

beforeAll(() => {
  vi.stubGlobal('IntersectionObserver', class { observe() {} disconnect() {} })
})

describe('CaptureWorkspace Next', () => {
  it('creates and reopens persistent batches', async () => {
    const user = userEvent.setup()
    const view = renderWorkspace()
    expect(screen.getByRole('combobox')).toHaveValue('')
    await user.selectOptions(screen.getByRole('combobox'), '物理')
    await user.click(screen.getByRole('button', { name: '新建批次' }))
    await user.click(screen.getAllByRole('button', { name: /数学/ })[0]!)
    expect(view.emitted('createBatch')).toEqual([['物理']])
    expect(view.emitted('openBatch')).toEqual([['batch-1']])
  })

  it('gives unnamed batches a dated identity and hides deletion in a secondary menu', async () => {
    const confirm = vi.spyOn(window, 'confirm').mockReturnValue(true)
    const user = userEvent.setup()
    const unnamed = { ...batch, subject: '', updatedAtUtcMs: 1_700_000_000_000 }
    const view = render(CaptureWorkspace, {
      props: {
        batches: [unnamed], detail: undefined, previews: {}, busy: false,
        errorMessage: '', desktopAvailable: true, lanAddresses: [],
        lanPreflight: readyPreflight, lanPreflightBusy: false, lanSession: undefined,
        saveState: 'saved', commitMessage: '', subjectOptions: ['数学'],
        captureSoundEnabled: true,
      },
    })

    expect(screen.getByRole('heading', { name: /未命名批次 ·/ })).toBeVisible()
    expect(screen.queryByRole('menuitem', { name: '删除批次…' })).not.toBeInTheDocument()
    await user.click(screen.getByRole('button', { name: /未命名批次.*更多操作/ }))
    await user.click(screen.getByRole('menuitem', { name: '删除批次…' }))
    expect(view.emitted('discardBatch')).toEqual([['batch-1']])
    confirm.mockRestore()
  })

  it('starts phone capture directly from the toolbar and keeps desktop actions', async () => {
    const user = userEvent.setup()
    const view = renderWorkspace(collectingDetail())
    await user.click(screen.getByRole('button', { name: /手机扫码/ }))
    await user.click(screen.getByRole('button', { name: /电脑批量选择/ }))
    await user.click(screen.getByRole('button', { name: /结束采集/ }))
    expect(view.emitted('mobileCapture')).toEqual([['192.168.1.2']])
    expect(view.emitted('importSelect')).toHaveLength(1)
    expect(view.emitted('finishCollecting')).toEqual([['数学']])
  })

  it('accepts a ready firewall rule on every Windows network profile', async () => {
    const user = userEvent.setup()
    const view = renderWorkspace(collectingDetail(), readyPreflight)
    await user.click(screen.getByRole('button', { name: /手机扫码/ }))
    expect(view.emitted('mobileCapture')).toEqual([['192.168.1.2']])
    expect(screen.queryByText(/把可信网络设为专用/)).not.toBeInTheDocument()
  })

  it('retries one-time authorization without exposing terminal commands', async () => {
    const user = userEvent.setup()
    const missingRule: CaptureLanPreflight = { ...readyPreflight, firewallRule: 'missing', canStart: false, needsFirewallRepair: true }
    const view = renderWorkspace(collectingDetail(), missingRule)
    await user.click(screen.getByRole('button', { name: /手机扫码/ }))
    expect(screen.getByRole('heading', { name: '下次扫码会再次请求授权' })).toBeVisible()
    expect(screen.queryByText(/netsh|PowerShell|命令提示符/i)).not.toBeInTheDocument()
    await user.click(screen.getByRole('button', { name: '再次授权并生成二维码' }))
    expect(view.emitted('mobileCapture')).toHaveLength(2)
  })

  it('contains keyboard focus in the phone dialog and restores it on Escape', async () => {
    const user = userEvent.setup()
    renderWorkspace(collectingDetail())
    const launcher = screen.getByRole('button', { name: /手机扫码/ })
    await user.click(launcher)
    expect(screen.getByRole('button', { name: '关闭' })).toHaveFocus()
    await user.keyboard('{Shift>}{Tab}{/Shift}')
    expect(screen.getByRole('dialog')).toContainElement(document.activeElement as HTMLElement)
    await user.keyboard('{Escape}')
    expect(screen.queryByRole('dialog')).not.toBeInTheDocument()
    expect(launcher).toHaveFocus()
  })

  it('selects a loose image without changing its role, then changes the role explicitly', async () => {
    const user = userEvent.setup()
    const view = renderWorkspace(organizingDetail())
    const loose = screen.getByLabelText('待配对图片：待配对超长文件名图片.png')
    await user.click(within(loose).getByLabelText('待配对超长文件名图片.png'))
    expect(view.emitted('stageItemRole')).toBeUndefined()
    expect(within(loose).getByText(/已选择/)).toBeVisible()

    await user.click(screen.getByRole('button', { name: '设为答案' }))
    expect(view.emitted('stageItemRole')).toEqual([['loose', 'answer']])
    expect(screen.queryByText(/双击/)).not.toBeInTheDocument()
  })

  it('plays a settled role-change cue after the role save succeeds', async () => {
    const user = userEvent.setup()
    const detail = organizingDetail()
    const view = renderWorkspace(detail)
    const loose = screen.getByLabelText('待配对图片：待配对超长文件名图片.png')

    await user.click(within(loose).getByLabelText('待配对超长文件名图片.png'))
    await user.click(screen.getByRole('button', { name: '设为答案' }))
    await view.rerender({ saveState: 'saving' })
    detail.items[4] = { ...detail.items[4]!, stagedRole: 'answer' }
    await view.rerender({ detail, saveState: 'saved' })

    await waitFor(() => expect(screen.getByLabelText('待配对图片：待配对超长文件名图片.png')).toHaveClass('is-role-changed'))
  })

  it('applies a configured subject to the whole organizing batch from the top', async () => {
    const user = userEvent.setup()
    const view = renderWorkspace(organizingDetail())

    const subjectBar = screen.getByLabelText('整批科目')
    await user.click(within(subjectBar).getByRole('button', { name: '化学' }))
    expect(view.emitted('assignBatchSubject')).toBeUndefined()
    expect(within(subjectBar).getByText(/将覆盖当前题卡科目/)).toBeVisible()
    await user.click(within(subjectBar).getByRole('button', { name: '应用到整批' }))

    expect(view.emitted('assignBatchSubject')).toEqual([['化学']])
  })

  it('uses an in-app impact confirmation before regrouping existing cards', async () => {
    const user = userEvent.setup()
    const view = renderWorkspace(organizingDetail())

    await user.click(screen.getByRole('button', { name: '重新分组全部图片' }))
    expect(view.emitted('applyLayout')).toBeUndefined()
    const dialog = screen.getByRole('alertdialog', { name: '确认重新分组全部图片' })
    expect(within(dialog).getByText(/2 张题卡/)).toBeVisible()
    expect(within(dialog).getByText(/5 张图片都会保留/)).toBeVisible()
    await user.click(within(dialog).getByRole('button', { name: '确认重新分组' }))
    expect(view.emitted('applyLayout')).toEqual([['alternating', 1, 1, null]])
  })

  it('corrects an assigned image role without a whole-card undo button', async () => {
    const user = userEvent.setup()
    const view = renderWorkspace(organizingDetail())
    const secondCard = screen.getByLabelText('第 2 道错题卡')

    expect(within(secondCard).getByText('缺答案')).toBeVisible()
    await user.click(within(secondCard).getByRole('button', { name: '把当前题图转为答案' }))
    expect(view.emitted('moveItem')).toContainEqual([{
      itemId: 'q2',
      targetDraftId: 'd2',
      targetRole: 'answer',
      targetPosition: 0,
    }])
    expect(within(secondCard).queryByRole('button', { name: '撤销这张卡' })).not.toBeInTheDocument()
  })

  it('adds a selected material to a new card or an existing card without dragging', async () => {
    const user = userEvent.setup()
    const view = renderWorkspace(organizingDetail())
    const loose = screen.getByLabelText('待配对图片：待配对超长文件名图片.png')

    await user.click(within(loose).getByLabelText('待配对超长文件名图片.png'))
    await user.click(screen.getByRole('button', { name: '用所选素材新建题卡' }))
    expect(view.emitted('mergeCard')).toContainEqual([['loose'], null, '数学'])

    await user.click(screen.getByRole('button', { name: '加入第 2 题' }))
    expect(view.emitted('moveItem')).toContainEqual([{
      itemId: 'loose',
      targetDraftId: 'd2',
      targetRole: 'question',
      targetPosition: 2,
    }])
  })

  it('drags an assigned card image back to the material library', async () => {
    const view = renderWorkspace(organizingDetail())
    const source = document.querySelector<HTMLElement>('[data-capture-item-id="q2b"]')!
    const target = screen.getByText('素材牌库').closest<HTMLElement>('[data-capture-drop="unassigned"]')!
    Object.defineProperty(document, 'elementFromPoint', {
      configurable: true,
      value: vi.fn(() => target),
    })

    await fireEvent.pointerDown(source, { pointerId: 31, button: 0, clientX: 30, clientY: 30 })
    await fireEvent.pointerMove(window, { pointerId: 31, clientX: 12, clientY: 30 })

    expect(target).toHaveClass('is-drop-active')

    await fireEvent.pointerUp(window, { pointerId: 31, clientX: 12, clientY: 30 })

    expect(view.emitted('moveItem')).toEqual([[{
      itemId: 'q2b',
      targetDraftId: null,
      targetRole: null,
      targetPosition: 0,
    }]])
    expect(screen.queryByRole('button', { name: /移回待配对/ })).not.toBeInTheDocument()
  })

  it('flips to the role and exact image that was dropped into a card', async () => {
    const detail = organizingDetail()
    detail.items[4] = { ...detail.items[4]!, stagedRole: 'answer' }
    const previews = {
      q1: 'data:image/png;base64,cTE=',
      a1: 'data:image/png;base64,YTE=',
      loose: 'data:image/png;base64,bG9vc2U=',
    }
    const view = renderWorkspace(detail, readyPreflight, false, previews)
    const source = document.querySelector<HTMLElement>('[data-capture-item-id="loose"]')!
    const target = screen.getByLabelText('第 1 道错题卡')
    Object.defineProperty(document, 'elementFromPoint', {
      configurable: true,
      value: vi.fn(() => target),
    })

    await fireEvent.pointerDown(source, { pointerId: 41, button: 0, clientX: 10, clientY: 10 })
    await fireEvent.pointerMove(window, { pointerId: 41, clientX: 22, clientY: 10 })
    await fireEvent.pointerUp(window, { pointerId: 41, clientX: 22, clientY: 10 })

    await view.rerender({ saveState: 'saving' })
    const updated: CaptureBatchDetail = {
      ...detail,
      batch: { ...detail.batch, revision: detail.batch.revision + 1 },
      items: detail.items.map(item => item.id === 'loose'
        ? { ...item, draftId: 'd1', role: 'answer', position: 1 }
        : item),
      drafts: detail.drafts.map(draft => draft.id === 'd1'
        ? { ...draft, answerItemIds: ['a1', 'loose'] }
        : draft),
      unassignedItemIds: [],
    }
    await view.rerender({ detail: updated, saveState: 'saved' })

    const firstCard = screen.getByLabelText('第 1 道错题卡')
    expect(within(firstCard).getByRole('button', { name: '翻回题面' })).toBeVisible()
    expect(within(firstCard).getByRole('img', { name: '待配对超长文件名图片.png' })).toBeVisible()
  })

  it('inherits the selected card subject when a loose image creates a new card', async () => {
    const detail = organizingDetail()
    detail.drafts[1] = { ...detail.drafts[1]!, subject: '' }
    const user = userEvent.setup()
    const view = renderWorkspace(detail)
    const secondCard = screen.getByLabelText('第 2 道错题卡')
    await user.click(within(secondCard).getByRole('button', { name: /未填写科目/ }))
    const loose = screen.getByLabelText('待配对图片：待配对超长文件名图片.png')
    const source = within(loose).getByLabelText('待配对超长文件名图片.png')
    const target = screen.getByText(/自动生成一道新题/).closest<HTMLElement>('[data-capture-drop]')!
    Object.defineProperty(document, 'elementFromPoint', {
      configurable: true,
      value: vi.fn(() => target),
    })

    await fireEvent.pointerDown(source, { pointerId: 17, button: 0, clientX: 10, clientY: 10 })
    await fireEvent.pointerMove(window, { pointerId: 17, clientX: 20, clientY: 10 })
    await fireEvent.pointerUp(window, { pointerId: 17, clientX: 20, clientY: 10 })

    expect(view.emitted('mergeCard')).toEqual([[['loose'], null, '数学']])
  })

  it('previews question drops with ink-green card feedback', async () => {
    renderWorkspace(organizingDetail())
    const loose = screen.getByLabelText('待配对图片：待配对超长文件名图片.png')
    const source = within(loose).getByLabelText('待配对超长文件名图片.png')
    const target = screen.getByLabelText('第 2 道错题卡')
    Object.defineProperty(document, 'elementFromPoint', {
      configurable: true,
      value: vi.fn(() => target),
    })

    await fireEvent.pointerDown(source, { pointerId: 29, button: 0, clientX: 10, clientY: 10 })
    await fireEvent.pointerMove(window, { pointerId: 29, clientX: 22, clientY: 10 })

    expect(document.querySelector('.capture-drag-ghost')).toHaveClass('is-question')
    expect(target).toHaveClass('is-drop-question')
    await fireEvent.pointerCancel(window, { pointerId: 29 })
    expect(document.querySelector('.capture-drag-ghost')).not.toBeInTheDocument()
  })

  it('enables atomic commit only for ready cards', async () => {
    const user = userEvent.setup()
    const view = renderWorkspace(organizingDetail())
    await user.click(screen.getByRole('button', { name: '保存全部就绪题（1）' }))
    expect(screen.getByText('1 道完整题卡')).toBeVisible()
    expect(screen.getByText('第 2 题：缺答案')).toBeVisible()
    expect(view.emitted('commitReady')).toHaveLength(1)
  })

  it('offers recognition undo only while the applied revision is still untouched', async () => {
    const user = userEvent.setup()
    const detail = organizingDetail()
    const view = renderWorkspace(detail)
    await view.rerender({
      recognitionNotice: '已切分 2 张题答图片，已放入素材牌库。',
      recognitionOperation: {
        operationId: 'operation-1',
        batchId: detail.batch.id,
        afterRevision: detail.batch.revision,
        createdItemCount: 2,
        reverted: false,
      },
    })

    expect(screen.getByText('智能切图已更新素材牌库')).toBeVisible()
    await user.click(screen.getByRole('button', { name: /撤销本次智能整理/ }))
    expect(view.emitted('recognitionRevert')).toEqual([['operation-1']])

    await view.rerender({
      detail: { ...detail, batch: { ...detail.batch, revision: detail.batch.revision + 1 } },
    })
    expect(screen.queryByRole('button', { name: /撤销本次智能整理/ })).not.toBeInTheDocument()
  })

  it('presents available visual splitting and disabled full recognition as separate modes', async () => {
    const view = renderWorkspace(organizingDetail())
    await view.rerender({ recognitionFeature: recognitionReady })

    expect(screen.getByText('智能切图 · 已开放')).toBeVisible()
    expect(screen.getByText('全自动识题 · 未开放')).toBeVisible()
    expect(screen.getByText(/不会要求下载 small \/ medium 模型/)).toBeVisible()
    expect(screen.getByText('全自动识题 · 未开放').closest('aside')).toHaveClass('future-recognition-note')
    expect(screen.getByText(/只看版面和留白，不读取文字/)).toBeVisible()
    expect(screen.queryByRole('button', { name: /全自动识题/ })).not.toBeInTheDocument()
  })

  it('returns focus to the review launcher after closing recognition suggestions', async () => {
    const user = userEvent.setup()
    const view = renderWorkspace(organizingDetail())
    await view.rerender({
      recognitionFeature: recognitionReady,
      recognitionJob: recognitionReviewJob(),
    })

    const launcher = screen.getByRole('button', { name: '查看切图建议' })
    expect(screen.getByRole('dialog', { name: '快速确认，不替你做决定' })).toBeVisible()
    await waitFor(() => expect(view.emitted('preview')).toContainEqual(['loose']))
    await user.click(screen.getByRole('button', { name: '关闭识别建议' }))
    await waitFor(() => expect(launcher).toHaveFocus())
    expect(screen.queryByRole('heading', { name: '快速确认，不替你做决定' })).not.toBeInTheDocument()
  })

  it('collapses the material library and new-card target when every image is assigned', () => {
    const detail = organizingDetail()
    detail.items = detail.items.filter(item => item.id !== 'loose')
    detail.unassignedItemIds = []
    renderWorkspace(detail)

    expect(screen.getByText('素材已全部配对')).toBeVisible()
    expect(screen.queryByText('拖到这里，自动生成一道新题')).not.toBeInTheDocument()
  })

  it('recovers focus when authorization state changes while the dialog is open', async () => {
    const user = userEvent.setup()
    const missingRule: CaptureLanPreflight = { ...readyPreflight, firewallRule: 'missing', canStart: false, needsFirewallRepair: true }
    const view = renderWorkspace(collectingDetail(), readyPreflight)
    await user.click(screen.getByRole('button', { name: /手机扫码/ }))
    screen.getByRole('button', { name: /生成二维码/ }).focus()
    await view.rerender({ lanPreflight: missingRule })
    await waitFor(() => expect(screen.getByRole('button', { name: '关闭' })).toHaveFocus())
  })
})
