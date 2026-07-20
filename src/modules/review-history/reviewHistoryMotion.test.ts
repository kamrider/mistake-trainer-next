import { describe, expect, it } from 'vitest'
import reportSource from '@/app/views/ReportView.vue?raw'
import detailSource from './components/ReviewHistoryDetail.vue?raw'
import filtersSource from './components/ReviewHistoryFilters.vue?raw'
import timelineSource from './components/ReviewHistoryTimeline.vue?raw'

function lastRule(source: string, selector: string) {
  const escaped = selector.replace(/[.*+?^${}()|[\]\\]/g, '\\$&')
  return [...source.matchAll(new RegExp(`${escaped}\\{([^}]*)\\}`, 'g'))].at(-1)?.[1] ?? ''
}

describe('review-history motion contract', () => {
  it('limits the final interactive transitions to transform and opacity', () => {
    const declarations = [
      lastRule(filtersSource, 'button'),
      lastRule(timelineSource, '.history-row'),
      lastRule(reportSource, '.history-link'),
    ]
    for (const declaration of declarations) {
      expect(declaration).toContain('transition:')
      expect(declaration).not.toMatch(/box-shadow|border-color|background|color/)
    }
    expect(declarations[0]).toContain('transform')
    expect(declarations[1]).toContain('opacity')
    expect(declarations[1]).toContain('transform')
    expect(declarations[2]).toContain('transform')
  })

  it('provides reduced-motion fallbacks for the timeline and mobile detail', () => {
    expect(timelineSource).toMatch(/prefers-reduced-motion:reduce[\s\S]*?\.history-row\{animation:none;transition:none\}/)
    expect(detailSource).toMatch(/prefers-reduced-motion:reduce[\s\S]*?\.detail-layer,.history-detail,.loading-mark\{animation:none\}/)
  })
})
