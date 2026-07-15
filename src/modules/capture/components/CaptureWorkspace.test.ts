import { render, screen } from '@testing-library/vue'
import userEvent from '@testing-library/user-event'
import { beforeAll, describe, expect, it, vi } from 'vitest'
import type {
  CaptureBatchDetail, CaptureBatchSummary, CaptureLanPreflight,
} from '../../../shared/api/bindings'
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

const readyPreflight: CaptureLanPreflight = {
  supported: true,
  activeProfiles: ['private'],
  firewallRule: 'ready',
  canStart: true,
  needsNetworkChange: false,
  needsFirewallRepair: false,
}

function renderWorkspace(
  detail?: CaptureBatchDetail,
  preflight: CaptureLanPreflight | undefined = readyPreflight,
) {
  return render(CaptureWorkspace, {
    props: {
      batches: detail ? [detail.batch] : [batch],
      detail,
      previews: {},
      busy: false,
      errorMessage: '',
      desktopAvailable: true,
      lanAddresses: [{ label: 'Wi-Fi', address: '192.168.1.2' }],
      lanPreflight: preflight,
      lanPreflightBusy: false,
      lanSession: undefined,
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
    await user.click(screen.getByRole('button', { name: /生成二维码/ }))
    await user.click(screen.getByRole('button', { name: /电脑批量选择/ }))
    await user.click(screen.getByRole('button', { name: /结束采集/ }))

    expect(view.emitted('mobileCapture')).toEqual([['192.168.1.2']])
    expect(view.emitted('importSelect')).toHaveLength(1)
    expect(view.emitted('finishCollecting')).toEqual([['数学']])
  })

  it('blocks QR generation on a public network and opens Windows settings', async () => {
    const user = userEvent.setup()
    const publicNetwork: CaptureLanPreflight = {
      supported: true,
      activeProfiles: ['public'],
      firewallRule: 'ready',
      canStart: false,
      needsNetworkChange: true,
      needsFirewallRepair: false,
    }
    const detail: CaptureBatchDetail = {
      batch,
      items: [],
      drafts: [],
      unassignedItemIds: [],
    }
    const view = renderWorkspace(detail, publicNetwork)

    await user.click(screen.getByRole('button', { name: /手机扫码/ }))
    expect(screen.getByRole('heading', { name: '跟着 4 步，把可信网络设为专用' })).toBeVisible()
    expect(screen.queryByRole('button', { name: /生成二维码/ })).not.toBeInTheDocument()
    expect(screen.getByText(/不会开放公用网络/)).toBeVisible()
    expect(screen.getByText('点击当前已连接的 Wi‑Fi 名称')).toBeVisible()

    await user.click(screen.getByRole('button', { name: '打开 Wi‑Fi 设置' }))
    expect(view.emitted('openLanNetworkSettings')).toEqual([['wifi']])
    await user.click(screen.getByRole('button', { name: '网线 / 扩展坞' }))
    expect(screen.getByText('进入“网络配置文件类型”')).toBeVisible()
    await user.click(screen.getByRole('button', { name: '打开以太网设置' }))
    expect(view.emitted('openLanNetworkSettings')).toEqual([['wifi'], ['ethernet']])
    expect(view.emitted('mobileCapture')).toBeUndefined()
  })

  it('offers one-click repair without exposing commands', async () => {
    const user = userEvent.setup()
    const missingRule: CaptureLanPreflight = {
      supported: true,
      activeProfiles: ['private'],
      firewallRule: 'missing',
      canStart: false,
      needsNetworkChange: false,
      needsFirewallRepair: true,
    }
    const detail: CaptureBatchDetail = {
      batch,
      items: [],
      drafts: [],
      unassignedItemIds: [],
    }
    const view = renderWorkspace(detail, missingRule)

    await user.click(screen.getByRole('button', { name: /手机扫码/ }))
    expect(screen.getByRole('heading', { name: '允许手机连接这台电脑' })).toBeVisible()
    expect(screen.queryByRole('button', { name: /生成二维码/ })).not.toBeInTheDocument()
    expect(screen.queryByText(/netsh|PowerShell|命令提示符/i)).not.toBeInTheDocument()
    expect(screen.getByText(/Windows 弹窗中点击“是”/)).toBeVisible()
    await user.click(screen.getByText('没有弹窗，或者我没有管理员权限'))
    expect(screen.getByText(/管理员密码/)).toBeVisible()

    await user.click(screen.getByRole('button', { name: '修复手机连接' }))
    expect(view.emitted('repairLanFirewall')).toHaveLength(1)
    expect(view.emitted('mobileCapture')).toBeUndefined()
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
