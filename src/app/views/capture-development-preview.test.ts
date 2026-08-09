import { describe, expect, it } from 'vitest'
import {
  createCaptureDevelopmentCropEditor,
  createCaptureDevelopmentPreview,
} from './capture-development-preview'

describe('capture development preview', () => {
  it('builds a consistent local-only card editing fixture', () => {
    const preview = createCaptureDevelopmentPreview(1_750_000_000_000)

    expect(preview.batches).toEqual([preview.detail.batch])
    expect(preview.detail.batch).toMatchObject({
      state: 'organizing',
      draftCount: 2,
      readyCount: 1,
    })
    expect(preview.detail.drafts).toHaveLength(2)
    expect(preview.detail.drafts.map(draft => draft.ready)).toEqual([true, false])

    const itemIds = new Set(preview.detail.items.map(item => item.id))
    const referencedIds = preview.detail.drafts.flatMap(draft => [
      ...draft.questionItemIds,
      ...draft.answerItemIds,
    ])
    expect(referencedIds.every(itemId => itemIds.has(itemId))).toBe(true)
    expect(preview.detail.unassignedItemIds.every(itemId => itemIds.has(itemId))).toBe(true)

    expect(Object.keys(preview.previews).sort()).toEqual([...itemIds].sort())
    expect(Object.values(preview.previews).every(value =>
      value.startsWith('data:image/svg+xml;charset=utf-8,'),
    )).toBe(true)
  })

  it('builds a crop editor state from the same in-memory preview', () => {
    const preview = createCaptureDevelopmentPreview(1_750_000_000_000)

    expect(createCaptureDevelopmentCropEditor(preview)).toEqual({
      itemId: 'preview-q1',
      itemName: '圆锥曲线题面.png',
      dataUrl: preview.previews['preview-q1'],
    })
  })
})
