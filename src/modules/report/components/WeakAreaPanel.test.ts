import { render, screen } from '@testing-library/vue'
import { describe, expect, it } from 'vitest'
import WeakAreaPanel from './WeakAreaPanel.vue'

describe('WeakAreaPanel', () => {
  it('renders ordered evidence, reason labels and serializable library links', () => {
    render(WeakAreaPanel, {
      props: {
        areas: [
          { label: '错因·计算失误', kind: 'reason', reviewedCount: 5, lapseCount: 3, lapseRate: 0.6, averageDurationMs: 45_000 },
          { label: '物理', kind: 'subject', reviewedCount: 4, lapseCount: 2, lapseRate: 0.5, averageDurationMs: 90_000 },
        ],
      },
    })

    expect(screen.getByRole('heading', { name: '本周最值得修正' })).toBeVisible()
    expect(screen.getByText('计算失误')).toBeVisible()
    expect(screen.getByText('60%')).toBeVisible()
    expect(screen.getByText(/5 次复习 · 平均 45 秒/)).toBeVisible()
    const link = screen.getByRole('link', { name: '筛选题库中的错因 错因·计算失误' })
    expect(link).toHaveAttribute('href', expect.stringContaining('#/library?tag='))
  })

  it('explains sparse evidence instead of making a claim', () => {
    render(WeakAreaPanel, { props: { areas: [] } })
    expect(screen.getByText(/至少完成 2 次真实评分/)).toBeVisible()
  })
})
