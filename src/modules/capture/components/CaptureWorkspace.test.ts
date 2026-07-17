import { render, screen, waitFor, within } from '@testing-library/vue'
import userEvent from '@testing-library/user-event'
import { beforeAll, describe, expect, it, vi } from 'vitest'
import type { CaptureBatchDetail, CaptureBatchSummary, CaptureLanPreflight } from '../../../shared/api/bindings'
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

function renderWorkspace(detail?: CaptureBatchDetail, preflight: CaptureLanPreflight | undefined = readyPreflight, preflightBusy = false) {
  return render(CaptureWorkspace, {
    props: {
      batches: detail ? [detail.batch] : [batch], detail, previews: {}, busy: false,
      errorMessage: '', desktopAvailable: true,
      lanAddresses: [{ label: 'Wi-Fi', address: '192.168.1.2' }],
      lanPreflight: preflight, lanPreflightBusy: preflightBusy, lanSession: undefined,
      saveState: 'saved', commitMessage: '',
    },
  })
}

function collectingDetail(): CaptureBatchDetail {
  return {
    batch,
    items: [{ id: 'new', sourceName: 'new.png', sourceSequence: 0, mediaType: 'image/png', byteLength: 1, width: 800, height: 600, stagedRole: 'question', draftId: null, role: null, position: null }],
    drafts: [],
    unassignedItemIds: ['new'],
  }
}

function organizingDetail(): CaptureBatchDetail {
  return {
    batch: { ...batch, state: 'organizing', itemCount: 5, draftCount: 2, readyCount: 1 },
    items: [
      { id: 'q1', sourceName: '第一题.png', sourceSequence: 0, mediaType: 'image/png', byteLength: 100, width: 1200, height: 900, stagedRole: 'question', draftId: 'd1', role: 'question', position: 0 },
      { id: 'a1', sourceName: '第一题答案.png', sourceSequence: 1, mediaType: 'image/png', byteLength: 100, width: 1200, height: 900, stagedRole: 'answer', draftId: 'd1', role: 'answer', position: 0 },
      { id: 'q2', sourceName: '第二题.png', sourceSequence: 2, mediaType: 'image/png', byteLength: 100, width: 1200, height: 900, stagedRole: 'question', draftId: 'd2', role: 'question', position: 0 },
      { id: 'q2b', sourceName: '第二题续图.png', sourceSequence: 3, mediaType: 'image/png', byteLength: 100, width: 1200, height: 900, stagedRole: 'question', draftId: 'd2', role: 'question', position: 1 },
      { id: 'loose', sourceName: '待配对超长文件名图片.png', sourceSequence: 4, mediaType: 'image/png', byteLength: 100, width: 1200, height: 900, stagedRole: 'question', draftId: null, role: null, position: null },
    ],
    drafts: [
      { id: 'd1', position: 0, subject: '数学', tags: [], note: '', questionItemIds: ['q1'], answerItemIds: ['a1'], ready: true },
      { id: 'd2', position: 1, subject: '数学', tags: [], note: '', questionItemIds: ['q2', 'q2b'], answerItemIds: [], ready: false },
    ],
    unassignedItemIds: ['loose'],
  }
}

beforeAll(() => {
  vi.stubGlobal('IntersectionObserver', class { observe() {} disconnect() {} })
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

  it('uses one click to toggle a loose image role without a double-click shortcut', async () => {
    const user = userEvent.setup()
    const view = renderWorkspace(organizingDetail())
    const loose = screen.getByLabelText('待配对图片：待配对超长文件名图片.png')
    await user.click(within(loose).getByLabelText('待配对超长文件名图片.png'))
    expect(view.emitted('stageItemRole')).toEqual([['loose', 'answer']])
    expect(screen.queryByText(/双击/)).not.toBeInTheDocument()
  })

  it('enables atomic commit only for ready cards', async () => {
    const user = userEvent.setup()
    const view = renderWorkspace(organizingDetail())
    await user.click(screen.getByRole('button', { name: /将 1 道题加入题库/ }))
    expect(screen.getByText('1 道完整题卡')).toBeVisible()
    expect(view.emitted('commitReady')).toHaveLength(1)
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
