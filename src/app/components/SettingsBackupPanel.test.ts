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
  portableReceipt: undefined,
  automaticStatus: undefined,
  automaticBusy: false,
  creating: false,
  creatingPortable: false,
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

  it('creates a portable backup and renders the recovery key only in the explicit receipt', async () => {
    const view = render(SettingsBackupPanel, {
      props: {
        ...baseProps,
        portableReceipt: {
          summary: created,
          recoveryKey: 'portable-recovery-key',
        },
      },
    })

    expect(screen.getByText('恢复密钥只显示这一次')).toBeVisible()
    expect(screen.getByText('portable-recovery-key')).toBeVisible()
    expect(screen.getByText(/丢失后无法找回/)).toBeVisible()
    await userEvent.click(screen.getByRole('button', { name: '创建便携加密备份' }))
    await userEvent.click(screen.getByRole('button', { name: '我已安全保存，隐藏密钥' }))

    expect(view.emitted().createPortable).toHaveLength(1)
    expect(view.emitted().dismissPortable).toHaveLength(1)
  })

  it('emits portable restore and automatic backup policy intentions', async () => {
    const view = render(SettingsBackupPanel, { props: baseProps })

    await userEvent.type(screen.getByLabelText('跨设备恢复密钥'), 'recovery-key')
    await userEvent.click(screen.getByRole('button', { name: '选择便携备份并准备恢复' }))
    await userEvent.clear(screen.getByLabelText('间隔天数'))
    await userEvent.type(screen.getByLabelText('间隔天数'), '14')
    await userEvent.clear(screen.getByLabelText('保留份数'))
    await userEvent.type(screen.getByLabelText('保留份数'), '8')
    await userEvent.click(screen.getByRole('button', { name: '选择目录并启用' }))

    expect(view.emitted().preparePortable?.[0]).toEqual(['recovery-key'])
    expect(view.emitted().configureAutomatic?.[0]).toEqual([14, 8])
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

  it('disables every manual backup and restore entry point during automatic configuration', async () => {
    render(SettingsBackupPanel, {
      props: {
        ...baseProps,
        candidate,
        automaticBusy: true,
      },
    })

    expect(screen.getByRole('button', { name: '创建加密备份' })).toBeDisabled()
    expect(screen.getByRole('button', { name: '创建便携加密备份' })).toBeDisabled()
    expect(screen.getByRole('button', { name: '选择备份并准备恢复' })).toBeDisabled()
    expect(screen.getByRole('button', { name: '查看风险并确认恢复' })).toBeDisabled()

    await userEvent.type(screen.getByLabelText('跨设备恢复密钥'), 'recovery-key')
    expect(screen.getByRole('button', { name: '选择便携备份并准备恢复' })).toBeDisabled()
  })
})
