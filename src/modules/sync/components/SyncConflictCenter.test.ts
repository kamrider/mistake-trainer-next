import { render, screen, waitFor, within } from '@testing-library/vue'
import userEvent from '@testing-library/user-event'
import { beforeEach, describe, expect, it, vi } from 'vitest'
import SyncConflictCenter from './SyncConflictCenter.vue'

const api = vi.hoisted(() => ({
  syncConflictList: vi.fn(),
  syncConflictResolve: vi.fn(),
  syncConflictResolveEntity: vi.fn(),
}))

vi.mock('../../../shared/api/bindings', () => ({ commands: api }))

const noteConflict = {
  id: 'conflict-note',
  entityType: 'problem',
  entityId: 'problem-1',
  entityLabel: '数学',
  fieldName: 'note',
  localValue: { kind: 'string', value: '先检查定义域' },
  remoteValue: { kind: 'string', value: '先通分' },
  createdAtUtcMs: 1_725_000_000_000,
}

const tagConflict = {
  id: 'conflict-tags',
  entityType: 'problem',
  entityId: 'problem-1',
  entityLabel: '数学',
  fieldName: 'tags',
  localValue: {
    kind: 'array',
    value: [
      { kind: 'string', value: '函数' },
      { kind: 'string', value: '粗心' },
    ],
  },
  remoteValue: { kind: 'array', value: [] },
  createdAtUtcMs: 1_725_000_000_001,
}

describe('SyncConflictCenter', () => {
  beforeEach(() => {
    vi.clearAllMocks()
    api.syncConflictList.mockResolvedValue({ ok: true, data: [] })
    api.syncConflictResolve.mockResolvedValue({ ok: true, data: [] })
    api.syncConflictResolveEntity.mockResolvedValue({ ok: true, data: [] })
  })

  it('shows both values and resolves every field for one entity atomically', async () => {
    api.syncConflictList.mockResolvedValue({
      ok: true,
      data: [noteConflict, tagConflict],
    })
    const { emitted } = render(SyncConflictCenter)

    const card = await screen.findByRole('article', { name: '数学的同步冲突' })
    expect(within(card).getAllByText('本机版本')).toHaveLength(2)
    expect(within(card).getAllByText('云端版本')).toHaveLength(2)
    expect(within(card).getByText('先检查定义域')).toBeVisible()
    expect(within(card).getByText('先通分')).toBeVisible()
    expect(within(card).getByText('函数')).toBeVisible()
    expect(within(card).getByText('无标签')).toBeVisible()

    await userEvent.click(within(card).getByRole('button', { name: '数学全部采用本机版本' }))

    expect(api.syncConflictResolveEntity).toHaveBeenCalledWith({
      entityType: 'problem',
      entityId: 'problem-1',
      choice: 'local',
    })
    await waitFor(() => expect(screen.queryByRole('article', { name: '数学的同步冲突' })).not.toBeInTheDocument())
    expect(screen.getByText('同步内容没有待处理冲突')).toBeVisible()
    expect(emitted().changed).toHaveLength(1)
  })

  it('resolves one field without hiding the other fields', async () => {
    api.syncConflictList.mockResolvedValue({
      ok: true,
      data: [noteConflict, tagConflict],
    })
    api.syncConflictResolve.mockResolvedValue({
      ok: true,
      data: [tagConflict],
    })
    render(SyncConflictCenter)

    const card = await screen.findByRole('article', { name: '数学的同步冲突' })
    const noteRow = within(card).getByRole('group', { name: '笔记冲突' })
    await userEvent.click(within(noteRow).getByRole('button', { name: '笔记采用云端版本' }))

    expect(api.syncConflictResolve).toHaveBeenCalledWith({
      conflictId: 'conflict-note',
      choice: 'remote',
    })
    await waitFor(() => expect(screen.queryByRole('group', { name: '笔记冲突' })).not.toBeInTheDocument())
    expect(screen.getByRole('group', { name: '标签冲突' })).toBeVisible()
    expect(screen.getByRole('button', { name: '标签采用本机版本' })).toHaveFocus()
  })

  it('keeps the unresolved card visible when resolution fails', async () => {
    api.syncConflictList.mockResolvedValue({ ok: true, data: [noteConflict] })
    api.syncConflictResolveEntity.mockResolvedValue({
      ok: false,
      error: {
        code: 'SYNC_CONFLICT_OPERATION_FAILED',
        userMessage: '冲突暂时无法处理，本地内容保持不变。',
        retryable: false,
        diagnosticId: 'diag-1',
      },
    })
    render(SyncConflictCenter)

    const button = await screen.findByRole('button', { name: '数学全部采用云端版本' })
    await userEvent.click(button)

    expect(await screen.findByRole('alert')).toHaveTextContent('本地内容保持不变')
    expect(screen.getByRole('article', { name: '数学的同步冲突' })).toBeVisible()
    expect(button).toBeEnabled()
  })

  it('renders remote deletion as an explicit keep-or-delete decision', async () => {
    api.syncConflictList.mockResolvedValue({
      ok: true,
      data: [{
        ...noteConflict,
        id: 'conflict-delete',
        fieldName: '__deleted__',
        localValue: {
          kind: 'object',
          value: {
            subject: { kind: 'string', value: '数学' },
            note: { kind: 'string', value: '我的修改' },
          },
        },
        remoteValue: { kind: 'bool', value: true },
      }],
    })
    render(SyncConflictCenter)

    expect(await screen.findByText('云端已删除')).toBeVisible()
    expect(screen.getByText('保留本机内容')).toBeVisible()
    expect(screen.getByRole('button', { name: '删除状态采用本机版本' })).toBeVisible()
    expect(screen.getByRole('button', { name: '删除状态采用云端版本' })).toBeVisible()
  })

  it('does not expose opaque entity ids and can retry a failed load', async () => {
    api.syncConflictList
      .mockResolvedValueOnce({
        ok: false,
        error: {
          code: 'SYNC_CONFLICT_OPERATION_FAILED',
          userMessage: '暂时读不到冲突。',
          retryable: true,
          diagnosticId: 'diag-load',
        },
      })
      .mockResolvedValueOnce({ ok: true, data: [noteConflict] })
    render(SyncConflictCenter)

    expect(await screen.findByRole('alert')).toHaveTextContent('暂时读不到冲突')
    expect(screen.queryByText('problem-1')).not.toBeInTheDocument()
    await userEvent.click(screen.getByRole('button', { name: '重新读取同步冲突' }))
    expect(await screen.findByRole('article', { name: '数学的同步冲突' })).toBeVisible()
    expect(screen.queryByText('problem-1')).not.toBeInTheDocument()
  })
})
