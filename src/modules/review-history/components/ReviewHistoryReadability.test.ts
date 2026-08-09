import { readFileSync } from 'node:fs'
import { resolve } from 'node:path'
import { describe, expect, it } from 'vitest'

const reviewHistorySources = [
  ['ReviewHistoryView.vue', 'src/app/views/ReviewHistoryView.vue'],
  ['ReviewHistoryFilters.vue', 'src/modules/review-history/components/ReviewHistoryFilters.vue'],
  ['ReviewHistoryTimeline.vue', 'src/modules/review-history/components/ReviewHistoryTimeline.vue'],
  ['ReviewHistoryDetail.vue', 'src/modules/review-history/components/ReviewHistoryDetail.vue'],
] as const

describe('review history readability contract', () => {
  it('keeps explicit user-facing type at 12px or larger', () => {
    const violations = reviewHistorySources.flatMap(([file, path]) => {
      const source = readFileSync(resolve(path), 'utf8')

      return [...source.matchAll(/font-size:\s*(\d+(?:\.\d+)?)px/g)]
        .filter((match) => Number(match[1]) < 12)
        .map((match) => ({ file, declaration: match[0] }))
    })

    expect(violations).toEqual([])
  })

  it('keeps recovery actions at least 44px high', () => {
    const historyView = readFileSync(
      resolve('src/app/views/ReviewHistoryView.vue'),
      'utf8',
    )

    expect(historyView).toMatch(/\.error-banner button,\.initial-error button\{min-height:44px/)
  })
})
