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
  importProgress?: { completed: number, total: number, failed: number },
) {
  return render(CaptureWorkspace, {
    props: {
      batches: detail ? [detail.batch] : [batch], detail, previews, busy: false,
      errorMessage: '', desktopAvailable: true,
      lanAddresses: [{ label: 'Wi-Fi', address: '192.168.1.2' }],
      lanPreflight: preflight, lanPreflightBusy: preflightBusy, lanSession: undefined,
      saveState: 'saved', draftSaveRetryAvailable: false, commitMessage: '',
      subjectOptions: ['语文', '数学', '英语', '物理', '化学'],
      captureSoundEnabled: true,
      importProgress,
    },
  })
}

function collectingDetail(): CaptureBatchDetail {
  return {
    batch,
    items: [{ id: 'new', sourceName: 'new.png', sourceSequence: 0, mediaType: 'image/png', byteLength: 1, width: 800, height: 600, stagedRole: 'question', draftId: null, role: null, position: null, cropDerivationId: null, cropSourceItemId: null }],
    drafts: [],
    unassignedItemIds: ['new'],
    pairSuggestions: [],
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
    pairSuggestions: [],
  }
}

const recognitionReady: OcrRecognitionFeatureStatus = {
  state: 'ready',
  requiredComponentId: 'opencv_preprocess',
  detail: '基础版面预切可直接使用；确认后结果只进入素材牌库。',
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
    const user = userEvent.setup()
    const unnamed = { ...batch, subject: '', updatedAtUtcMs: 1_700_000_000_000 }
    const view = render(CaptureWorkspace, {
      props: {
        batches: [unnamed], detail: undefined, previews: {}, busy: false,
        errorMessage: '', desktopAvailable: true, lanAddresses: [],
        lanPreflight: readyPreflight, lanPreflightBusy: false, lanSession: undefined,
        saveState: 'saved', draftSaveRetryAvailable: false, commitMessage: '', subjectOptions: ['数学'],
        captureSoundEnabled: true,
      },
    })

    expect(screen.getByRole('heading', { name: /未命名批次 ·/ })).toBeVisible()
    expect(screen.queryByRole('menuitem', { name: '删除批次…' })).not.toBeInTheDocument()
    const menuTrigger = screen.getByRole('button', { name: /未命名批次.*更多操作/ })
    await user.click(menuTrigger)
    await user.click(screen.getByRole('menuitem', { name: '删除批次…' }))
    expect(screen.getByRole('alertdialog', { name: /删除.*未命名批次/ })).toBeVisible()
    expect(view.emitted('discardBatch')).toBeUndefined()
    await user.click(screen.getByRole('button', { name: '保留批次' }))
    expect(view.emitted('discardBatch')).toBeUndefined()
    await waitFor(() => expect(menuTrigger).toHaveFocus())

    await user.click(screen.getByRole('button', { name: /未命名批次.*更多操作/ }))
    await user.click(screen.getByRole('menuitem', { name: '删除批次…' }))
    await user.click(screen.getByRole('button', { name: '删除批次' }))
    expect(view.emitted('discardBatch')).toEqual([['batch-1']])
  })

  it('implements the complete keyboard and dismissal model for the batch action menu', async () => {
    const user = userEvent.setup()
    renderWorkspace()
    const trigger = screen.getByRole('button', { name: /更多操作/ })

    expect(trigger).toHaveAttribute('aria-haspopup', 'menu')
    expect(trigger).toHaveAttribute('aria-expanded', 'false')
    trigger.focus()
    await user.keyboard('{ArrowDown}')
    const item = screen.getByRole('menuitem', { name: '删除批次…' })
    await waitFor(() => expect(item).toHaveFocus())
    expect(trigger).toHaveAttribute('aria-expanded', 'true')

    await user.keyboard('{Escape}')
    expect(screen.queryByRole('menuitem', { name: '删除批次…' })).not.toBeInTheDocument()
    await waitFor(() => expect(trigger).toHaveFocus())

    await user.keyboard('{ArrowUp}')
    await waitFor(() => expect(screen.getByRole('menuitem', { name: '删除批次…' })).toHaveFocus())
    await user.keyboard('{Tab}')
    expect(screen.queryByRole('menuitem', { name: '删除批次…' })).not.toBeInTheDocument()

    await user.click(trigger)
    await waitFor(() => expect(screen.getByRole('menuitem', { name: '删除批次…' })).toHaveFocus())
    await user.click(document.body)
    expect(screen.queryByRole('menuitem', { name: '删除批次…' })).not.toBeInTheDocument()
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

  it('forwards dropped files unchanged and disables the drop target while busy', async () => {
    const view = renderWorkspace(collectingDetail())
    const dropZone = screen.getByText('拖入一组图片，按文件顺序进入当前批次')
      .closest<HTMLElement>('.external-drop')!
    const image = new File(['image'], 'photo.PNG', { type: '' })
    const pdf = new File(['pdf'], 'notes.pdf', { type: 'application/pdf' })

    await fireEvent.drop(dropZone, { dataTransfer: { files: [image, pdf] } })
    expect(view.emitted('importFiles')).toEqual([[[image, pdf]]])

    await view.rerender({ busy: true })
    expect(dropZone).toHaveAttribute('aria-disabled', 'true')
    await fireEvent.dragEnter(dropZone)
    expect(dropZone).not.toHaveClass('is-active')
    await fireEvent.drop(dropZone, { dataTransfer: { files: [image] } })
    expect(view.emitted('importFiles')).toHaveLength(1)
  })

  it('keeps the collecting subject while new images refresh the same batch', async () => {
    const user = userEvent.setup()
    const detail = collectingDetail()
    const view = renderWorkspace(detail)
    const subject = screen.getByLabelText('批次科目')

    await user.selectOptions(subject, '物理')
    const refreshed = collectingDetail()
    refreshed.batch = {
      ...refreshed.batch,
      itemCount: 2,
      revision: refreshed.batch.revision + 1,
    }
    refreshed.items.push({
      ...refreshed.items[0]!,
      id: 'new-2',
      sourceName: 'new-2.png',
      sourceSequence: 1,
    })
    refreshed.unassignedItemIds.push('new-2')
    await view.rerender({ detail: refreshed })

    expect(subject).toHaveValue('物理')
    await user.click(screen.getByRole('button', { name: '结束采集，开始整理' }))
    expect(view.emitted('finishCollecting')).toEqual([['物理']])
  })

  it('announces batch import progress and failures', () => {
    renderWorkspace(
      collectingDetail(),
      readyPreflight,
      false,
      {},
      { completed: 3, total: 10, failed: 1 },
    )

    const status = screen.getByRole('status')
    expect(status).toHaveTextContent('正在导入 3/10 张，1 张失败')
    const progress = status.querySelector('progress')
    expect(progress).toHaveAttribute('max', '10')
    expect(progress).toHaveAttribute('value', '3')
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
    const view = renderWorkspace(collectingDetail())
    const launcher = screen.getByRole('button', { name: /手机扫码/ })
    await user.click(launcher)
    expect(view.emitted('mobileCapture')).toEqual([['192.168.1.2']])
    expect(screen.getByRole('button', { name: '关闭手机采集' })).toHaveFocus()
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
    expect(view.emitted('qualityCheck')).toEqual([['loose']])
    expect(within(loose).getByText(/已选择/)).toBeVisible()

    const questionRole = screen.getByRole('button', { name: '设为题面' })
    const answerRole = screen.getByRole('button', { name: '设为答案' })
    expect(questionRole).toHaveAttribute('aria-pressed', 'true')
    expect(answerRole).toHaveAttribute('aria-pressed', 'false')
    await user.click(answerRole)
    expect(view.emitted('stageItemRole')).toEqual([['loose', 'answer']])
    expect(screen.queryByText(/双击/)).not.toBeInTheDocument()
  })

  it('shows a non-blocking quality warning for the selected material', async () => {
    const user = userEvent.setup()
    const view = renderWorkspace(organizingDetail())
    const loose = screen.getByLabelText('待配对图片：待配对超长文件名图片.png')
    await user.click(within(loose).getByLabelText('待配对超长文件名图片.png'))
    await view.rerender({
      qualityReports: {
        loose: {
          itemId: 'loose', issues: ['possible_edge_cut'], sharpnessScore: 0.4,
          darkFraction: 0, brightFraction: 0.4, contrastScore: 0.7,
          suggestedRotationDegrees: 0, suggestedCrop: null,
        },
      },
    })

    expect(screen.getByText(/内容贴近图片边缘/)).toBeVisible()
    expect(within(loose).getByText('需检查')).toBeVisible()
    await user.click(screen.getByRole('button', { name: '继续使用' }))
    expect(view.emitted('qualityDismiss')).toEqual([['loose']])
    await user.click(screen.getByRole('button', { name: '打开裁剪修正' }))
    expect(view.emitted('crop')).toContainEqual(['loose'])
  })

  it('keeps unfinished draft text through a same-card refresh and saves with the latest subject', async () => {
    const user = userEvent.setup()
    const detail = organizingDetail()
    const view = renderWorkspace(detail)
    const inspector = screen.getByLabelText('当前题卡信息')
    const tags = within(inspector).getByLabelText('标签')
    const note = within(inspector).getByLabelText('笔记')

    await user.clear(tags)
    await user.type(tags, '本地标签，新标签')
    await user.clear(note)
    await user.type(note, '仍在输入的笔记')

    const refreshed = organizingDetail()
    refreshed.batch = { ...refreshed.batch, revision: refreshed.batch.revision + 1 }
    refreshed.drafts[0] = { ...refreshed.drafts[0]!, subject: '物理' }
    await view.rerender({ detail: refreshed, busy: true })

    expect(tags).toHaveValue('本地标签，新标签')
    expect(note).toHaveValue('仍在输入的笔记')

    const emissionCount = view.emitted('updateDraft')?.length ?? 0
    await fireEvent.change(tags)
    expect(view.emitted('updateDraft')).toHaveLength(emissionCount + 1)
    expect(view.emitted('updateDraft')?.at(-1)).toEqual([
      refreshed.drafts[0],
      '物理',
      ['本地标签', '新标签'],
      '仍在输入的笔记',
    ])
  })

  it('quickly classifies the selected draft with a canonical mistake reason', async () => {
    const user = userEvent.setup()
    const detail = organizingDetail()
    detail.drafts[0] = { ...detail.drafts[0]!, tags: ['函数'] }
    const view = renderWorkspace(detail)

    const inspector = screen.getByLabelText('当前题卡信息')
    const reasons = within(inspector).getByRole('group', { name: '常见错因（可多选）' })
    const calculation = within(reasons).getByRole('button', { name: /计算失误/ })
    expect(calculation).toHaveAttribute('aria-pressed', 'false')

    await user.click(calculation)

    expect(view.emitted('updateDraft')?.at(-1)).toEqual([
      detail.drafts[0],
      '数学',
      ['函数', '错因·计算失误'],
      '',
    ])
  })

  it('shows matched question and answer crops inside the material library and applies them in one click', async () => {
    const user = userEvent.setup()
    const detail = organizingDetail()
    detail.items.push({
      id: 'paired-answer',
      sourceName: '第一题智能答案.png',
      sourceSequence: 5,
      mediaType: 'image/png',
      byteLength: 100,
      width: 1200,
      height: 900,
      stagedRole: 'answer',
      draftId: null,
      role: null,
      position: null,
      cropDerivationId: null,
      cropSourceItemId: null,
    })
    detail.unassignedItemIds = ['loose', 'paired-answer']
    detail.pairSuggestions = [{
      id: 'pair-1',
      questionItemIds: ['loose'],
      answerItemIds: ['paired-answer'],
      confidenceBasisPoints: 9100,
    }]
    const view = renderWorkspace(detail)

    expect(screen.getByRole('heading', { name: '已找到 1 组题面与答案' })).toBeVisible()
    expect(screen.getByText('可信度 91%')).toBeVisible()
    expect(screen.getByLabelText('待配对超长文件名图片.png')).toBeVisible()
    expect(screen.getByLabelText('第一题智能答案.png')).toBeVisible()
    await user.click(screen.getByRole('button', { name: '一键生成 1 张题卡' }))

    expect(view.emitted('applyPairSuggestions')).toEqual([[['pair-1']]])
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
    const mathematics = within(subjectBar).getByRole('button', { name: '数学' })
    const chemistry = within(subjectBar).getByRole('button', { name: '化学' })
    expect(mathematics).toHaveAttribute('aria-pressed', 'true')
    expect(chemistry).toHaveAttribute('aria-pressed', 'false')
    await user.click(chemistry)
    expect(mathematics).toHaveAttribute('aria-pressed', 'false')
    expect(chemistry).toHaveAttribute('aria-pressed', 'true')
    expect(view.emitted('assignBatchSubject')).toBeUndefined()
    expect(within(subjectBar).getByText(/将覆盖当前题卡科目/)).toBeVisible()
    await user.click(within(subjectBar).getByRole('button', { name: '应用到整批' }))

    expect(view.emitted('assignBatchSubject')).toEqual([['化学']])
  })

  it('keeps an unconfirmed organizing subject through a same-batch refresh', async () => {
    const user = userEvent.setup()
    const view = renderWorkspace(organizingDetail())
    const subjectBar = screen.getByLabelText('整批科目')
    const chemistry = within(subjectBar).getByRole('button', { name: '化学' })

    await user.click(chemistry)
    const refreshed = organizingDetail()
    refreshed.batch = {
      ...refreshed.batch,
      revision: refreshed.batch.revision + 1,
    }
    await view.rerender({ detail: refreshed })

    expect(chemistry).toHaveClass('selected')
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

  it('contains layout confirmation focus, scroll, Escape, and launcher return', async () => {
    const user = userEvent.setup()
    const previousOverflow = document.body.style.overflow
    document.body.style.overflow = 'auto'
    const view = renderWorkspace(organizingDetail())

    try {
      const launcher = screen.getByRole('button', { name: '重新分组全部图片' })
      await user.click(launcher)
      const dialog = screen.getByRole('alertdialog', { name: '确认重新分组全部图片' })
      const confirm = within(dialog).getByRole('button', { name: '确认重新分组' })
      const back = within(dialog).getByRole('button', { name: '返回' })

      await waitFor(() => expect(back).toHaveFocus())
      expect(document.body.style.overflow).toBe('hidden')
      confirm.focus()
      await fireEvent.keyDown(confirm, { key: 'Tab', shiftKey: true })
      expect(back).toHaveFocus()
      await fireEvent.keyDown(back, { key: 'Tab' })
      expect(confirm).toHaveFocus()

      await fireEvent.keyDown(dialog, { key: 'Escape' })
      await waitFor(() => expect(launcher).toHaveFocus())
      expect(screen.queryByRole('alertdialog', { name: '确认重新分组全部图片' })).not.toBeInTheDocument()
      expect(document.body.style.overflow).toBe('auto')
    }
    finally {
      view.unmount()
      document.body.style.overflow = previousOverflow
    }
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
    const source = document.querySelector<HTMLElement>('[data-capture-item-id="q2b"] .thumbnail-activate')!
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
    const source = document.querySelector<HTMLElement>('[data-capture-item-id="loose"] .thumbnail-activate')!
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

    expect(screen.getByText('智能切图 · 基础预切')).toBeVisible()
    expect(screen.getByText('全自动识题 · 未开放')).toBeVisible()
    expect(screen.getByText(/当前可按题号锚点给出题答配对建议/)).toBeVisible()
    expect(screen.getByText('全自动识题 · 未开放').closest('aside')).toHaveClass('future-recognition-note')
    expect(screen.getByText(/按版面给出预切建议/)).toBeVisible()
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

  it('propagates review-saving and exclusive recognition states separately', async () => {
    const view = renderWorkspace(organizingDetail())
    await view.rerender({
      recognitionFeature: recognitionReady,
      recognitionJob: recognitionReviewJob(),
      recognitionBusy: true,
      recognitionOperationBusy: false,
    })

    expect(screen.getByText(
      '正在后台保存审核决定；你可以继续确认下一条，应用切图需等待保存完成。',
    )).toBeVisible()
    expect(screen.getByRole('button', { name: '接受建议' })).toBeEnabled()
    expect(screen.getByRole('button', { name: '调整边界' })).toBeDisabled()

    await view.rerender({ recognitionOperationBusy: true })
    expect(screen.getByRole('button', { name: '接受建议' })).toBeDisabled()
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
    await waitFor(() => expect(screen.getByRole('button', { name: '关闭手机采集' })).toHaveFocus())
  })

  it('offers an explicit retry only for a retained failed draft save', async () => {
    const user = userEvent.setup()
    const view = renderWorkspace(organizingDetail())

    await view.rerender({
      saveState: 'error',
      draftSaveRetryAvailable: true,
    })
    const retry = screen.getByRole('button', { name: '重试保存草稿' })
    expect(retry).toBeEnabled()
    await user.click(retry)
    expect(view.emitted('retryDraftSave')).toHaveLength(1)

    await view.rerender({ busy: true })
    expect(screen.getByRole('button', { name: '重试保存草稿' })).toBeDisabled()

    await view.rerender({
      busy: false,
      draftSaveRetryAvailable: false,
    })
    expect(screen.queryByRole('button', { name: '重试保存草稿' })).not.toBeInTheDocument()
  })
})
