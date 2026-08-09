import { fireEvent, render, screen, waitFor } from '@testing-library/vue'
import userEvent from '@testing-library/user-event'
import { describe, expect, it } from 'vitest'
import type {
  CaptureLanAddress,
  CaptureLanPreflight,
  CaptureLanSession,
} from '../../../shared/api/bindings'
import CaptureLanDialog from './CaptureLanDialog.vue'

const addresses: CaptureLanAddress[] = [
  { label: '家庭 Wi-Fi', address: '192.168.1.2' },
  { label: '个人热点', address: '10.0.0.4' },
]

const readyPreflight: CaptureLanPreflight = {
  supported: true,
  activeProfiles: ['public'],
  firewallRule: 'ready',
  canStart: true,
  needsNetworkChange: false,
  needsFirewallRepair: false,
}

const repairPreflight: CaptureLanPreflight = {
  ...readyPreflight,
  firewallRule: 'missing',
  canStart: false,
  needsFirewallRepair: true,
}

const session: CaptureLanSession = {
  sessionId: 'session-1',
  batchId: 'batch-1',
  qrSvgDataUrl: 'data:image/svg+xml,test',
  selectedAddress: '192.168.1.2',
  expiresAtUtcMs: Date.now() + 10 * 60_000,
  receivedItemCount: 3,
  receivedBytes: 2 * 1024 * 1024,
}

function renderDialog(overrides: Partial<{
  addresses: CaptureLanAddress[]
  preflight: CaptureLanPreflight | undefined
  preflightBusy: boolean
  session: CaptureLanSession | undefined
  busy: boolean
}> = {}) {
  return render(CaptureLanDialog, {
    props: {
      addresses,
      preflight: readyPreflight,
      preflightBusy: false,
      session: undefined,
      busy: false,
      ...overrides,
    },
  })
}

describe('CaptureLanDialog', () => {
  it('selects a network and emits the exact start payload', async () => {
    const view = renderDialog()

    await userEvent.selectOptions(
      screen.getByRole('combobox', { name: '网络接口' }),
      '10.0.0.4',
    )
    await userEvent.click(screen.getByRole('button', { name: '生成二维码' }))

    expect(view.emitted().start).toEqual([['10.0.0.4']])
  })

  it('keeps focus inside and emits close on Escape and backdrop press', async () => {
    const view = renderDialog()
    const close = screen.getByRole('button', { name: '关闭手机采集' })

    await waitFor(() => expect(close).toHaveFocus())
    await userEvent.keyboard('{Shift>}{Tab}{/Shift}')
    expect(screen.getByRole('dialog')).toContainElement(document.activeElement as HTMLElement)
    await userEvent.keyboard('{Escape}')
    expect(view.emitted().close).toHaveLength(1)

    await fireEvent.mouseDown(view.container.querySelector('.lan-overlay')!)
    expect(view.emitted().close).toHaveLength(2)
  })

  it('explains authorization retry without exposing terminal commands', async () => {
    const view = renderDialog({ preflight: repairPreflight })

    expect(screen.getByRole('heading', { name: '下次扫码会再次请求授权' })).toBeVisible()
    expect(screen.queryByText(/netsh|PowerShell|命令提示符/i)).not.toBeInTheDocument()
    await userEvent.click(screen.getByRole('button', { name: '再次授权并生成二维码' }))

    expect(view.emitted().start).toEqual([['192.168.1.2']])
  })

  it('shows an active QR session and stops it explicitly', async () => {
    const view = renderDialog({ session })

    expect(screen.getByRole('img', { name: '手机采集二维码' })).toBeVisible()
    expect(screen.getByText('3 张 · 2.0 MB')).toBeVisible()
    expect(screen.getByText(/约 10 分钟后/)).toBeVisible()
    await userEvent.click(screen.getByRole('button', { name: '停止手机采集' }))

    expect(view.emitted().stop).toHaveLength(1)
  })

  it('offers a recoverable empty-network state', async () => {
    const view = renderDialog({ addresses: [] })

    expect(screen.getByText(/没有检测到家庭网络地址/)).toBeVisible()
    await userEvent.click(screen.getByRole('button', { name: '重新检测网络' }))

    expect(view.emitted().refreshAddresses).toHaveLength(1)
  })

  it('recovers focus when a conditional preflight action disappears', async () => {
    const view = renderDialog()
    const generate = screen.getByRole('button', { name: '生成二维码' })
    await waitFor(() => expect(screen.getByRole('button', { name: '关闭手机采集' })).toHaveFocus())
    generate.focus()

    await view.rerender({ preflight: repairPreflight })

    await waitFor(() => {
      expect(screen.getByRole('button', { name: '关闭手机采集' })).toHaveFocus()
    })
  })
})
