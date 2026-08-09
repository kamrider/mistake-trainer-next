import { render, screen, within } from '@testing-library/vue'
import userEvent from '@testing-library/user-event'
import { describe, expect, it } from 'vitest'
import SettingsStoragePanel from './SettingsStoragePanel.vue'

const baseProps = {
  status: {
    kind: 'custom' as const,
    locationLabel: '自定义位置 · StudyDisk',
    databaseBytes: 4096,
    assetBytes: 8192,
    migrationPending: false,
  },
  statusMessage: '',
  receipt: undefined,
  migrating: false,
}

describe('SettingsStoragePanel', () => {
  it('renders bounded capacity and never expands a safe location label into a path', () => {
    render(SettingsStoragePanel, { props: baseProps })

    const panel = screen.getByRole('region', { name: '资料库存储位置' })
    expect(within(panel).getByText('4.0 KB')).toBeVisible()
    expect(within(panel).getByText('8.0 KB')).toBeVisible()
    expect(within(panel).getByText('12.0 KB')).toBeVisible()
    expect(panel).toHaveTextContent('自定义位置 · StudyDisk')
    expect(panel).not.toHaveTextContent(/C:\\|Users\\|Lytree/)
  })

  it('announces migration receipts with severity and safe summaries', () => {
    render(SettingsStoragePanel, {
      props: {
        ...baseProps,
        receipt: {
          kind: 'warning',
          title: '新位置已启用，原副本需手动清理',
          detail: '自定义位置 · StudyDisk · 4 个加密资源 · 12.0 KB。',
        },
      },
    })

    expect(screen.getByRole('alert', { name: '存储迁移结果' })).toHaveTextContent(
      '原副本需手动清理',
    )
  })

  it('emits migration intent only when the current location is stable', async () => {
    const view = render(SettingsStoragePanel, { props: baseProps })
    await userEvent.click(screen.getByRole('button', { name: '迁移资料库' }))
    expect(view.emitted().migrate).toHaveLength(1)

    await view.rerender({
      ...baseProps,
      status: {
        ...baseProps.status,
        migrationPending: true,
      },
    })
    expect(screen.getByRole('button', { name: '等待安全重启' })).toBeDisabled()
  })

  it('shows an honest bounded message when storage status is unavailable', () => {
    render(SettingsStoragePanel, {
      props: {
        ...baseProps,
        status: undefined,
        statusMessage: '容量和迁移会在 Windows 桌面应用中显示。',
      },
    })

    expect(screen.getByRole('status')).toHaveTextContent('Windows 桌面应用中显示')
    expect(screen.queryByRole('button', { name: '迁移资料库' })).not.toBeInTheDocument()
  })
})
