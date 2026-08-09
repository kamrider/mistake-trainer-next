import { render, screen } from '@testing-library/vue'
import userEvent from '@testing-library/user-event'
import { describe, expect, it } from 'vitest'
import SettingsReviewPanel from './SettingsReviewPanel.vue'

const options = [
  { value: 'off' as const, title: '关闭专注插曲', hint: '训练题之间不插入额外环节。' },
  { value: 'session_start' as const, title: '每轮开始前 · 推荐', hint: '进入普通训练时先热身。' },
  { value: 'every_10' as const, title: '每完成 10 题', hint: '每十题短暂停一下。' },
]

const baseProps = {
  preferences: { focusPolicy: 'off' as const },
  options,
  saving: false,
  message: '',
}

describe('SettingsReviewPanel', () => {
  it('renders the selected policy and emits an immutable change intention', async () => {
    const preferences = { focusPolicy: 'off' as const }
    const view = render(SettingsReviewPanel, {
      props: {
        ...baseProps,
        preferences,
      },
    })

    expect(screen.getByRole('radio', { name: /关闭专注插曲/ })).toBeChecked()
    await userEvent.click(screen.getByRole('radio', { name: /每完成 10 题/ }))

    expect(view.emitted().updateFocusPolicy).toEqual([['every_10']])
    expect(preferences.focusPolicy).toBe('off')
  })

  it('emits save intent and preserves the commercial boundary copy', async () => {
    const view = render(SettingsReviewPanel, { props: baseProps })

    expect(screen.getByText(/模拟考试不会插入专注环节/)).toBeVisible()
    expect(screen.getByRole('radio', { name: /每轮开始前 · 推荐/ })).toBeInTheDocument()

    await userEvent.click(screen.getByRole('button', { name: '保存训练节奏' }))
    expect(view.emitted().save).toHaveLength(1)
  })

  it('announces persistence failure and disables save while pending', () => {
    render(SettingsReviewPanel, {
      props: {
        ...baseProps,
        saving: true,
        message: '训练节奏没有保存。',
      },
    })

    expect(screen.getByRole('status')).toHaveTextContent('训练节奏没有保存。')
    expect(screen.getByRole('button', { name: '保存中…' })).toBeDisabled()
  })
})
