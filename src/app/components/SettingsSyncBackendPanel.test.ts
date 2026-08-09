import { render, screen } from '@testing-library/vue'
import userEvent from '@testing-library/user-event'
import { describe, expect, it } from 'vitest'
import type { AppResult } from '../../shared/api/app-result'
import type { CloudBackendStatus } from '../../shared/api/bindings'
import SettingsSyncBackendPanel, { type SettingsBackendOption } from './SettingsSyncBackendPanel.vue'

const options: SettingsBackendOption[] = [
  {
    kind: 'local-only',
    title: '仅本地（推荐）',
    hint: '所有资料保存在当前设备。',
    available: true,
    badge: undefined,
  },
  {
    kind: 'supabase',
    title: 'Supabase',
    hint: '可选远程同步服务。',
    available: true,
    badge: undefined,
  },
  {
    kind: 'tencent',
    title: '腾讯云',
    hint: '国内适配器尚未启用。',
    available: false,
    badge: '规划中',
  },
]

const localStatus: AppResult<CloudBackendStatus> = {
  ok: true,
  data: {
    kind: 'local-only',
    configured: true,
    ready: true,
    syncEnabled: false,
  },
}

describe('SettingsSyncBackendPanel', () => {
  it('renders the local-first status and emits an available backend intention', async () => {
    const view = render(SettingsSyncBackendPanel, {
      props: {
        status: localStatus,
        options,
        busy: false,
        message: '',
      },
    })

    expect(screen.getByRole('heading', { name: '数据始终先保存在本地' })).toBeVisible()
    expect(screen.getByRole('status')).toHaveTextContent('本地优先 · 不需要网络')
    expect(screen.getByRole('status')).toHaveTextContent('云端同步未启用，待同步变更会安全保留')
    expect(screen.getByRole('button', { name: /^仅本地/ })).toHaveClass('selected')

    await userEvent.click(screen.getByRole('button', { name: /^Supabase/ }))

    expect(view.emitted().select).toEqual([['supabase']])
  })

  it('keeps unavailable providers inert and disables every choice while pending', async () => {
    const view = render(SettingsSyncBackendPanel, {
      props: {
        status: localStatus,
        options,
        busy: true,
        message: '',
      },
    })

    const tencent = screen.getByRole('button', { name: /腾讯云.*规划中/ })
    expect(tencent).toBeDisabled()
    expect(screen.getByRole('button', { name: /^仅本地/ })).toBeDisabled()
    expect(screen.getByRole('button', { name: /^Supabase/ })).toBeDisabled()

    await userEvent.click(tencent)
    expect(view.emitted().select).toBeUndefined()
  })

  it('announces backend failures and follow-up status messages', () => {
    render(SettingsSyncBackendPanel, {
      props: {
        status: {
          ok: false,
          error: {
            code: 'SYNC_STATUS_UNAVAILABLE',
            userMessage: '暂时无法读取同步设置，本地数据仍可正常使用',
            retryable: true,
            diagnosticId: 'sync-status',
          },
        },
        options,
        busy: false,
        message: '已保持本地模式',
      },
    })

    expect(screen.getByRole('alert')).toHaveTextContent('暂时无法读取同步设置')
    expect(screen.getByRole('status')).toHaveTextContent('已保持本地模式')
  })
})
