import { render, screen } from '@testing-library/vue'
import userEvent from '@testing-library/user-event'
import { describe, expect, it } from 'vitest'
import SettingsBackupPanel from './SettingsBackupPanel.vue'

const created = {
  formatVersion: 1,
  createdAtUtcMs: 1_725_000_000_000,
  assetCount: 4,
  encryptedBytes: 2_097_152,
  label: 'mistake-trainer-backup-safe',
  readyForRestore: false,
}

const candidate = {
  id: 'candidate-opaque-id',
  expiresAtUtcMs: 1_725_086_400_000,
  summary: {
    ...created,
    readyForRestore: true,
  },
  path: String.raw`C:\Users\Private\backup.mtb`,
}

const baseProps = {
  created: undefined,
  candidate: undefined,
  creating: false,
  preparing: false,
  restoring: false,
  message: '',
}

describe('SettingsBackupPanel', () => {
  it('emits explicit create and prepare intentions', async () => {
    const view = render(SettingsBackupPanel, { props: baseProps })

    await userEvent.click(screen.getByRole('button', { name: '创建加密备份' }))
    await userEvent.click(screen.getByRole('button', { name: '选择备份并准备恢复' }))

    expect(view.emitted().create).toHaveLength(1)
    expect(view.emitted().prepare).toHaveLength(1)
  })

  it('renders encrypted summaries and keeps restore explicit and path-free', async () => {
    const view = render(SettingsBackupPanel, {
      props: {
        ...baseProps,
        created,
        candidate,
      },
    })

    expect(screen.getByText('加密备份已创建')).toBeVisible()
    expect(screen.getAllByText('mistake-trainer-backup-safe')).toHaveLength(2)
    expect(screen.getAllByText(/4 个资源 · 2.0 MB/)).toHaveLength(2)
    expect(screen.getByText(/已复制到隔离区并再次校验/)).toBeVisible()
    expect(document.body).not.toHaveTextContent(/C:\\Users|Private\\backup/)

    await userEvent.click(screen.getByRole('button', { name: '查看风险并确认恢复' }))
    expect(view.emitted().openRestore).toHaveLength(1)
  })

  it('keeps backup command feedback next to its actions', () => {
    render(SettingsBackupPanel, {
      props: {
        ...baseProps,
        message: '备份包校验失败，当前资料库没有改变。',
      },
    })

    expect(screen.getByRole('alert')).toHaveTextContent('当前资料库没有改变')
  })

  it('mutually disables every backup action while native work is pending', async () => {
    const view = render(SettingsBackupPanel, {
      props: {
        ...baseProps,
        candidate,
        creating: true,
      },
    })

    expect(screen.getByRole('button', { name: '正在创建…' })).toBeDisabled()
    expect(screen.getByRole('button', { name: '选择备份并准备恢复' })).toBeDisabled()
    expect(screen.getByRole('button', { name: '查看风险并确认恢复' })).toBeDisabled()

    await view.rerender({ creating: false, preparing: true })
    expect(screen.getByRole('button', { name: '创建加密备份' })).toBeDisabled()
    expect(screen.getByRole('button', { name: '正在校验并暂存…' })).toBeDisabled()
    expect(screen.getByRole('button', { name: '查看风险并确认恢复' })).toBeDisabled()
  })
})
