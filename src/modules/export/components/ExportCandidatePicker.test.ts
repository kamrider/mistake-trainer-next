import { render, screen } from '@testing-library/vue'
import userEvent from '@testing-library/user-event'
import { describe, expect, it } from 'vitest'
import type { ExportCandidate } from '../../../shared/api/bindings'
import ExportCandidatePicker from './ExportCandidatePicker.vue'

const candidates: ExportCandidate[] = [
  {
    id: 'math-1',
    subject: '数学',
    note: '圆锥曲线焦点关系',
    questionAssetCount: 2,
    answerAssetCount: 1,
    dueAtUtcMs: 1_700_000_000_000,
    reviewCount: 3,
  },
  {
    id: 'physics-1',
    subject: '物理',
    note: '受力分析',
    questionAssetCount: 1,
    answerAssetCount: 0,
    dueAtUtcMs: null,
    reviewCount: 0,
  },
]

describe('ExportCandidatePicker', () => {
  it('switches source with a single click and exposes the current source', async () => {
    const user = userEvent.setup()
    const view = render(ExportCandidatePicker, {
      props: { candidates, source: 'due', selectedIds: ['math-1'], loading: false },
    })

    expect(screen.getByRole('radio', { name: /到期队列/ })).toHaveAttribute('aria-checked', 'true')
    await user.click(screen.getByRole('radio', { name: /最近训练批次/ }))

    expect(view.emitted('source')).toEqual([['latest_review_session']])
  })

  it('filters locally and selects only visible results', async () => {
    const user = userEvent.setup()
    const view = render(ExportCandidatePicker, {
      props: { candidates, source: 'all_active', selectedIds: [], loading: false },
    })

    await user.type(screen.getByRole('searchbox', { name: '搜索候选题' }), '焦点')
    expect(screen.getByText('圆锥曲线焦点关系')).toBeVisible()
    expect(screen.queryByText('受力分析')).not.toBeInTheDocument()
    await user.click(screen.getByRole('button', { name: '全选当前结果' }))

    expect(view.emitted('selectAll')).toEqual([[['math-1']]])
  })

  it('toggles rows, clears selection, and explains incomplete assets', async () => {
    const user = userEvent.setup()
    const view = render(ExportCandidatePicker, {
      props: { candidates, source: 'due', selectedIds: ['math-1'], loading: false },
    })

    expect(screen.getByText('题答齐全')).toBeVisible()
    expect(screen.getByText('缺少答案')).toBeVisible()
    await user.click(screen.getByRole('checkbox', { name: '选择物理：受力分析' }))
    await user.click(screen.getByRole('button', { name: '清空选择' }))

    expect(view.emitted('toggle')).toEqual([['physics-1']])
    expect(view.emitted('clear')).toHaveLength(1)
  })

  it('shows truthful loading and empty states', async () => {
    const view = render(ExportCandidatePicker, {
      props: { candidates: [], source: 'latest_review_session', selectedIds: [], loading: true },
    })
    expect(screen.getByText('正在读取可导出的题目…')).toBeVisible()

    await view.rerender({ candidates: [], source: 'latest_review_session', selectedIds: [], loading: false })
    expect(screen.getByText('还没有可用的最近训练批次。')).toBeVisible()
  })
})
