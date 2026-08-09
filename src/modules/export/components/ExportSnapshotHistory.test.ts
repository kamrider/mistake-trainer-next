import { render, screen } from '@testing-library/vue'
import userEvent from '@testing-library/user-event'
import { describe, expect, it } from 'vitest'
import type { DeletedExportSnapshotSummary, ExportSnapshotSummary } from '../../../shared/api/bindings'
import ExportSnapshotHistory from './ExportSnapshotHistory.vue'

const snapshots: ExportSnapshotSummary[] = [
  { id: 'snapshot-1', title: '本周复盘', problemCount: 12, layout: 'question_answer_alternating', createdAtUtcMs: 1_700_000_000_000 },
  { id: 'snapshot-2', title: '期中题册', problemCount: 30, layout: 'questions_then_answers', createdAtUtcMs: 1_700_100_000_000 },
]

const deleted: DeletedExportSnapshotSummary[] = [{
  snapshot: { id: 'deleted-1', title: '旧题册', problemCount: 4, layout: 'original_image_folder', createdAtUtcMs: 1_699_000_000_000 },
  deletedAtUtcMs: 1_700_200_000_000,
  purgeAfterUtcMs: 1_702_792_000_000,
}]

describe('ExportSnapshotHistory', () => {
  it('renders safe metadata and emits generate and delete actions', async () => {
    const user = userEvent.setup()
    const view = render(ExportSnapshotHistory, {
      props: {
        snapshots, deletedSnapshots: [], snapshotsLoaded: true, trashLoaded: true,
        generatingId: '', deletingId: '', restoringId: '', operationBusy: false,
      },
    })

    expect(screen.getByText('12 题 · 题答交替')).toBeVisible()
    expect(screen.queryByText(/C:\\/)).not.toBeInTheDocument()
    await user.click(screen.getByRole('button', { name: '生成导出文件：本周复盘' }))
    await user.click(screen.getByRole('button', { name: '删除导出快照：期中题册' }))

    expect(view.emitted('generate')).toEqual([[snapshots[0]]])
    expect(view.emitted('delete')).toEqual([['snapshot-2']])
  })

  it('marks only the active generation row while preventing a second native dialog', () => {
    render(ExportSnapshotHistory, {
      props: {
        snapshots, deletedSnapshots: [], snapshotsLoaded: true, trashLoaded: true,
        generatingId: 'snapshot-1', deletingId: '', restoringId: '', operationBusy: true,
      },
    })

    expect(screen.getByRole('button', { name: '正在生成：本周复盘' })).toHaveTextContent('生成中…')
    expect(screen.getByRole('button', { name: '生成导出文件：期中题册' })).toHaveTextContent('生成文件')
    expect(screen.getByRole('button', { name: '生成导出文件：期中题册' })).toBeDisabled()
  })

  it('renders the recycle area and emits restore with the full deleted snapshot', async () => {
    const user = userEvent.setup()
    const view = render(ExportSnapshotHistory, {
      props: {
        snapshots: [], deletedSnapshots: deleted, snapshotsLoaded: true, trashLoaded: true,
        generatingId: '', deletingId: '', restoringId: '', operationBusy: false,
      },
    })

    expect(screen.getByText('原图文件夹')).toBeVisible()
    await user.click(screen.getByRole('button', { name: '恢复导出快照：旧题册' }))
    expect(view.emitted('restore')).toEqual([[deleted[0]]])
  })

  it('distinguishes loading from a genuinely empty history', async () => {
    const view = render(ExportSnapshotHistory, {
      props: {
        snapshots: [], deletedSnapshots: [], snapshotsLoaded: false, trashLoaded: false,
        generatingId: '', deletingId: '', restoringId: '', operationBusy: false,
      },
    })
    expect(screen.getByText('正在读取导出快照…')).toBeVisible()

    await view.rerender({
      snapshots: [], deletedSnapshots: [], snapshotsLoaded: true, trashLoaded: true,
      generatingId: '', deletingId: '', restoringId: '', operationBusy: false,
    })
    expect(screen.getByText('还没有保存过导出快照。')).toBeVisible()
    expect(screen.getByText('回收区为空。')).toBeVisible()
  })

  it('locks every conflicting mutation while identifying the active row', async () => {
    const view = render(ExportSnapshotHistory, {
      props: {
        snapshots, deletedSnapshots: deleted, snapshotsLoaded: true, trashLoaded: true,
        generatingId: '', deletingId: 'snapshot-1', restoringId: '', operationBusy: true,
      },
    })

    expect(screen.getByRole('button', { name: '正在删除导出快照：本周复盘' })).toBeDisabled()
    expect(screen.getByRole('button', { name: '删除导出快照：期中题册' })).toBeDisabled()
    expect(screen.getByRole('button', { name: '恢复导出快照：旧题册' })).toBeDisabled()
    expect(screen.getByRole('button', { name: '生成导出文件：期中题册' })).toBeDisabled()

    await view.rerender({ deletingId: '', restoringId: 'deleted-1', operationBusy: true })
    expect(screen.getByRole('button', { name: '正在恢复导出快照：旧题册' })).toBeDisabled()
    expect(screen.getByRole('button', { name: '删除导出快照：本周复盘' })).toBeDisabled()
  })
})
