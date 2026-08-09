import { render, screen, within } from '@testing-library/vue'
import { describe, expect, it } from 'vitest'
import ExportWorkflowGuide from './ExportWorkflowGuide.vue'

describe('ExportWorkflowGuide', () => {
  it('explains the ordered boundary between a reusable snapshot and a local file', () => {
    render(ExportWorkflowGuide)

    const guide = screen.getByRole('complementary', { name: '先保存方案，再生成文件' })
    const steps = within(guide).getAllByRole('listitem')
    expect(steps).toHaveLength(3)
    expect(steps[0]).toHaveTextContent('选择题目')
    expect(steps[1]).toHaveTextContent('保存快照')
    expect(steps[1]).toHaveTextContent('可同步、可恢复')
    expect(steps[2]).toHaveTextContent('生成文件')
    expect(steps[2]).toHaveTextContent('本机位置')
  })
})
