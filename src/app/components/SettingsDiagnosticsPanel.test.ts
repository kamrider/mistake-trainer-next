import { render, screen } from '@testing-library/vue'
import userEvent from '@testing-library/user-event'
import { describe, expect, it } from 'vitest'
import SettingsDiagnosticsPanel from './SettingsDiagnosticsPanel.vue'

const baseProps = {
  receipt: undefined,
  exporting: false,
  message: '',
  nativeAvailable: true,
  generatedAtLabel: '',
}

describe('SettingsDiagnosticsPanel', () => {
  it('emits an export intention without owning native orchestration', async () => {
    const view = render(SettingsDiagnosticsPanel, { props: baseProps })

    await userEvent.click(screen.getByRole('button', { name: '生成安全诊断报告' }))

    expect(view.emitted().export).toHaveLength(1)
  })

  it('disables export outside the native runtime and while an export is pending', async () => {
    const view = render(SettingsDiagnosticsPanel, {
      props: {
        ...baseProps,
        nativeAvailable: false,
      },
    })

    expect(screen.getByRole('button', { name: '生成安全诊断报告' })).toBeDisabled()

    await view.rerender({
      ...baseProps,
      exporting: true,
    })
    expect(screen.getByRole('button', { name: '正在检查并生成…' })).toBeDisabled()
  })

  it('renders only the privacy-safe receipt contract', () => {
    const receipt = {
      reportId: '019f4b87-4cab-7b83-a4a0-46acac7d1362',
      fileLabel: 'Mistake-Trainer-Diagnostics-safe.json',
      generatedAtUtcMs: 1_700_000_000_000,
      warningCount: 2,
      path: String.raw`C:\Users\Private\Diagnostics`,
    }
    render(SettingsDiagnosticsPanel, {
      props: {
        ...baseProps,
        receipt,
        generatedAtLabel: '2023年11月15日 06:13',
      },
    })

    const status = screen.getByRole('status', { name: '诊断报告已生成' })
    expect(status).toHaveTextContent('Mistake-Trainer-Diagnostics-safe.json')
    expect(status).toHaveTextContent('019f4b87-4cab-7b83-a4a0-46acac7d1362')
    expect(status).toHaveTextContent('2 项需留意')
    expect(status).not.toHaveTextContent(/C:\\Users|Private\\Diagnostics/)
  })
})
