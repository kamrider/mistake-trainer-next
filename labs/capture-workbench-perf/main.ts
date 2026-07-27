import { createApp, h, reactive } from 'vue'
import type {
  CaptureBatchDetail,
  CaptureBatchSummary,
  CaptureItemSummary,
} from '../../src/shared/api/bindings'
import CaptureWorkspace from '../../src/modules/capture/components/CaptureWorkspace.vue'
import '../../src/shared/styles/tokens.css'
import './perf.css'

type PerfEvidence = {
  sampleCount: number
  mountDurationMs: number
  scrollDurationMs: number | null
  longTaskCount: number
  maxLongTaskMs: number
  previewRequestCount: number
  previewCachePeak: number
  previewCacheSize: number
  completedDragCount: number
}

declare global {
  interface Window {
    __capturePerfEvidence: PerfEvidence
  }
}

const batch: CaptureBatchSummary = {
  id: 'perf-batch',
  subject: '数学',
  state: 'organizing',
  itemCount: 150,
  draftCount: 0,
  readyCount: 0,
  updatedAtUtcMs: Date.now(),
  revision: 1,
}

const items = Array.from({ length: 150 }, (_, index): CaptureItemSummary => ({
  id: `perf-item-${String(index + 1).padStart(3, '0')}`,
  sourceName: `性能题图-${String(index + 1).padStart(3, '0')}.png`,
  sourceSequence: index,
  mediaType: 'image/png',
  byteLength: 16_384,
  width: 1600,
  height: 1200,
  stagedRole: index % 4 === 3 ? 'answer' : 'question',
  draftId: null,
  role: null,
  position: null,
  cropDerivationId: null,
  cropSourceItemId: null,
}))

const detail = reactive<CaptureBatchDetail>({
  batch,
  items,
  drafts: [],
  unassignedItemIds: items.map(item => item.id),
})
const previews = reactive<Record<string, string>>({})
const previewOrder: string[] = []
const previewDataUrl = 'data:image/svg+xml;charset=utf-8,' + encodeURIComponent(
  '<svg xmlns="http://www.w3.org/2000/svg" width="320" height="240" viewBox="0 0 320 240">'
  + '<rect width="320" height="240" fill="#fffdf7"/><path d="M24 55h272M24 90h240M24 125h272M24 160h210" stroke="#33463f" stroke-width="10" stroke-linecap="round"/>'
  + '<circle cx="270" cy="190" r="24" fill="none" stroke="#b9583f" stroke-width="9"/></svg>',
)
const longTasks: number[] = []
let previewRequestCount = 0
let previewCachePeak = 0
let completedDragCount = 0

window.__capturePerfEvidence = {
  sampleCount: 150,
  mountDurationMs: 0,
  scrollDurationMs: null,
  longTaskCount: 0,
  maxLongTaskMs: 0,
  previewRequestCount: 0,
  previewCachePeak: 0,
  previewCacheSize: 0,
  completedDragCount: 0,
}

if ('PerformanceObserver' in window) {
  const observer = new PerformanceObserver((list) => {
    for (const entry of list.getEntries()) longTasks.push(entry.duration)
    updateEvidence()
  })
  try {
    observer.observe({ type: 'longtask', buffered: true })
  }
  catch {
    // The summary explicitly reports zero observed entries when Long Tasks is unsupported.
  }
}

function removePreview(itemId: string) {
  delete previews[itemId]
  const index = previewOrder.indexOf(itemId)
  if (index >= 0) previewOrder.splice(index, 1)
}

function loadPreview(itemId: string) {
  if (previews[itemId]) return
  previewRequestCount += 1
  removePreview(itemId)
  previews[itemId] = previewDataUrl
  previewOrder.push(itemId)
  while (previewOrder.length > 40) {
    const expired = previewOrder.shift()
    if (expired) removePreview(expired)
  }
  previewCachePeak = Math.max(previewCachePeak, Object.keys(previews).length)
  updateEvidence()
}

function mergeCard(itemIds: string[]) {
  const selected = itemIds
    .map(itemId => detail.items.find(item => item.id === itemId))
    .filter((item): item is CaptureItemSummary => Boolean(item))
  if (!selected.length) return
  const draftId = `perf-draft-${detail.drafts.length + 1}`
  const questionItemIds = selected.filter(item => item.stagedRole === 'question').map(item => item.id)
  const answerItemIds = selected.filter(item => item.stagedRole === 'answer').map(item => item.id)
  for (const item of selected) {
    item.draftId = draftId
    item.role = item.stagedRole
    item.position = item.stagedRole === 'question'
      ? questionItemIds.indexOf(item.id)
      : answerItemIds.indexOf(item.id)
  }
  detail.unassignedItemIds = detail.unassignedItemIds.filter(itemId => !itemIds.includes(itemId))
  detail.drafts.push({
    id: draftId,
    position: detail.drafts.length,
    subject: '数学',
    tags: [],
    note: '',
    questionItemIds,
    answerItemIds,
    ready: questionItemIds.length > 0 && answerItemIds.length > 0,
  })
  detail.batch.draftCount = detail.drafts.length
  completedDragCount += 1
  updateEvidence()
}

function stageItemRole(itemId: string, stagedRole: 'question' | 'answer') {
  const item = detail.items.find(value => value.id === itemId)
  if (item) item.stagedRole = stagedRole
}

function updateEvidence(patch: Partial<PerfEvidence> = {}) {
  Object.assign(window.__capturePerfEvidence, {
    longTaskCount: longTasks.length,
    maxLongTaskMs: longTasks.length ? Math.max(...longTasks) : 0,
    previewRequestCount,
    previewCachePeak,
    previewCacheSize: Object.keys(previews).length,
    completedDragCount,
    ...patch,
  })
  const evidence = window.__capturePerfEvidence
  const summary = document.getElementById('perf-summary')
  if (summary) {
    summary.textContent = [
      `挂载 ${evidence.mountDurationMs.toFixed(1)} ms`,
      `滚动 ${evidence.scrollDurationMs?.toFixed(1) ?? '未运行'} ms`,
      `长任务 ${evidence.longTaskCount} 个 / 最大 ${evidence.maxLongTaskMs.toFixed(1)} ms`,
      `预览缓存 ${evidence.previewCacheSize} / 峰值 ${evidence.previewCachePeak}`,
      `拖拽完成 ${evidence.completedDragCount}`,
    ].join(' · ')
  }
}

const PerfApp = {
  setup() {
    return () => h(CaptureWorkspace, {
      batches: [detail.batch],
      detail,
      previews,
      busy: false,
      errorMessage: '',
      desktopAvailable: true,
      lanAddresses: [],
      lanPreflight: undefined,
      lanPreflightBusy: false,
      lanSession: undefined,
      saveState: 'saved',
      commitMessage: '',
      subjectOptions: ['数学', '物理', '化学'],
      captureSoundEnabled: false,
      recognitionFeature: {
        state: 'evidence_gate_pending',
        requiredComponentId: 'ppocrv6_small',
        detail: '性能实验不运行识别。',
      },
      recognitionBusy: false,
      onPreview: loadPreview,
      onStageItemRole: stageItemRole,
      onMergeCard: mergeCard,
    })
  },
}

const mountStarted = performance.now()
createApp(PerfApp).mount('#app')
updateEvidence({ mountDurationMs: performance.now() - mountStarted })

function frame() {
  return new Promise<void>(resolve => requestAnimationFrame(() => resolve()))
}

async function runScrollEvidence() {
  const strip = document.querySelector<HTMLElement>('.unassigned-strip')
  if (!strip) throw new Error('unassigned strip is missing')
  longTasks.length = 0
  strip.scrollTop = 0
  await frame()
  const started = performance.now()
  const maximum = strip.scrollHeight - strip.clientHeight
  for (let step = 1; step <= 60; step += 1) {
    strip.scrollTop = maximum * (step / 60)
    await frame()
  }
  for (let step = 59; step >= 0; step -= 1) {
    strip.scrollTop = maximum * (step / 60)
    await frame()
  }
  await frame()
  updateEvidence({ scrollDurationMs: performance.now() - started })
}

document.getElementById('run-scroll')?.addEventListener('click', () => {
  void runScrollEvidence()
})
