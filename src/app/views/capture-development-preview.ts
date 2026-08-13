import type {
  CaptureBatchDetail,
  CaptureBatchSummary,
  CaptureItemSummary,
} from '../../shared/api/bindings'
import type { CaptureCropEditorState } from '@/modules/capture'

export interface CaptureDevelopmentPreview {
  batches: CaptureBatchSummary[]
  detail: CaptureBatchDetail
  previews: Record<string, string>
}

export function createCaptureDevelopmentCropEditor(
  preview: CaptureDevelopmentPreview,
  itemId = 'preview-q1',
): CaptureCropEditorState {
  const item = preview.detail.items.find(value => value.id === itemId)
  const dataUrl = preview.previews[itemId]
  if (!item || !dataUrl) throw new Error(`Missing capture preview item: ${itemId}`)
  return { itemId, itemName: item.sourceName, dataUrl }
}

function previewSvg(label: string, accent: string) {
  const svg = `<svg xmlns="http://www.w3.org/2000/svg" width="1200" height="900" viewBox="0 0 1200 900">
    <rect width="1200" height="900" fill="#fffdf7"/>
    <rect x="56" y="52" width="1088" height="796" rx="28" fill="#f4f1e8" stroke="${accent}" stroke-width="5"/>
    <path d="M118 196h760M118 270h934M118 344h840M118 508h910M118 582h690" stroke="#9aa49d" stroke-width="18" stroke-linecap="round" opacity=".55"/>
    <text x="118" y="138" fill="${accent}" font-family="Microsoft YaHei, sans-serif" font-size="44" font-weight="700">${label}</text>
    <text x="118" y="736" fill="#52645d" font-family="Microsoft YaHei, sans-serif" font-size="31">浏览器设计预览 · 本地示例素材</text>
  </svg>`
  return `data:image/svg+xml;charset=utf-8,${encodeURIComponent(svg)}`
}

function item(
  id: string,
  sourceName: string,
  sourceSequence: number,
  stagedRole: 'question' | 'answer',
  draftId: string | null,
  role: 'question' | 'answer' | null,
  position: number | null,
): CaptureItemSummary {
  return {
    id,
    sourceName,
    sourceSequence,
    mediaType: 'image/svg+xml',
    byteLength: 2048,
    width: 1200,
    height: 900,
    stagedRole,
    draftId,
    role,
    position,
    cropDerivationId: null,
    cropSourceItemId: null,
  }
}

export function createCaptureDevelopmentPreview(
  now = Date.now(),
): CaptureDevelopmentPreview {
  const batch: CaptureBatchSummary = {
    id: 'capture-preview-batch',
    subject: '数学',
    state: 'organizing',
    itemCount: 5,
    draftCount: 2,
    readyCount: 1,
    updatedAtUtcMs: now,
    revision: 7,
  }
  const items = [
    item('preview-q1', '圆锥曲线题面.png', 0, 'question', 'preview-draft-1', 'question', 0),
    item('preview-a1', '圆锥曲线答案.png', 1, 'answer', 'preview-draft-1', 'answer', 0),
    item('preview-q2', '电磁感应题面上半页.png', 2, 'question', 'preview-draft-2', 'question', 0),
    item('preview-q2b', '电磁感应题面下半页.png', 3, 'question', 'preview-draft-2', 'question', 1),
    item('preview-loose', '待整理的化学实验长文件名素材.png', 4, 'answer', null, null, null),
  ]
  const detail: CaptureBatchDetail = {
    batch,
    items,
    drafts: [
      {
        id: 'preview-draft-1',
        position: 0,
        subject: '数学',
        tags: ['圆锥曲线'],
        note: '先核对焦点与长轴方向。',
        questionItemIds: ['preview-q1'],
        answerItemIds: ['preview-a1'],
        ready: true,
      },
      {
        id: 'preview-draft-2',
        position: 1,
        subject: '物理',
        tags: ['电磁感应'],
        note: '题面有两张连续图片，尚缺答案。',
        questionItemIds: ['preview-q2', 'preview-q2b'],
        answerItemIds: [],
        ready: false,
      },
    ],
    unassignedItemIds: ['preview-loose'],
    pairSuggestions: [],
  }
  const previews: Record<string, string> = {
    'preview-q1': previewSvg('01 · 圆锥曲线题面', '#b9583f'),
    'preview-a1': previewSvg('01 · 圆锥曲线答案', '#21332d'),
    'preview-q2': previewSvg('02 · 电磁感应题面（上）', '#b9583f'),
    'preview-q2b': previewSvg('02 · 电磁感应题面（下）', '#b9583f'),
    'preview-loose': previewSvg('待整理 · 化学实验答案', '#557263'),
  }

  return { batches: [batch], detail, previews }
}
