<script setup lang="ts">
import { isTauri } from '@tauri-apps/api/core'
import { listen, type UnlistenFn } from '@tauri-apps/api/event'
import { onBeforeUnmount, onMounted, reactive, ref } from 'vue'
import CaptureWorkspace from '../../modules/capture/components/CaptureWorkspace.vue'
import {
  commands,
  type CaptureBatchDetail,
  type CaptureBatchSummary,
  type CaptureDraftSummary,
  type CaptureLayoutMode,
} from '../../shared/api/bindings'
import { normalizeAppResult } from '../../shared/api/normalize-result'

const batches = ref<CaptureBatchSummary[]>([])
const detail = ref<CaptureBatchDetail>()
const busy = ref(false)
const errorMessage = ref('')
const previews = reactive<Record<string, string>>({})
const previewOrder: string[] = []
const desktopAvailable = isTauri()
let unlisten: UnlistenFn | undefined
let refreshTimer: ReturnType<typeof setTimeout> | undefined

function showError(message: string) {
  errorMessage.value = message
}

async function loadBatches() {
  if (!desktopAvailable) return
  try {
    const result = normalizeAppResult(await commands.captureBatchList())
    if (result.ok) batches.value = result.data
    else showError(result.error.userMessage)
  }
  catch {
    showError('采集箱连接中断，请重新打开应用后重试。')
  }
}

async function loadDetail(batchId: string) {
  if (!desktopAvailable) return
  try {
    const result = normalizeAppResult(await commands.captureBatchDetail(batchId))
    if (result.ok) detail.value = result.data
    else showError(result.error.userMessage)
  }
  catch {
    showError('没有读取到这个采集批次，请返回后重试。')
  }
}

async function createBatch(subject: string) {
  if (!desktopAvailable || busy.value) return
  busy.value = true
  errorMessage.value = ''
  try {
    const result = normalizeAppResult(await commands.captureBatchCreate({ subject }))
    if (result.ok) {
      await loadBatches()
      await loadDetail(result.data.id)
    }
    else showError(result.error.userMessage)
  }
  catch {
    showError('新批次没有创建成功，请稍后重试。')
  }
  finally {
    busy.value = false
  }
}

async function discardBatch(batchId: string) {
  if (!desktopAvailable || busy.value) return
  busy.value = true
  try {
    const result = normalizeAppResult(await commands.captureBatchDiscard(batchId))
    if (result.ok) {
      if (detail.value?.batch.id === batchId) detail.value = undefined
      await loadBatches()
    }
    else showError(result.error.userMessage)
  }
  catch {
    showError('批次没有删除成功，原有图片仍会保留。')
  }
  finally {
    busy.value = false
  }
}

async function importSelect() {
  const batchId = detail.value?.batch.id
  if (!desktopAvailable || !batchId || busy.value) return
  busy.value = true
  errorMessage.value = ''
  try {
    const result = normalizeAppResult(await commands.captureImportSelect(batchId))
    if (!result.ok) showError(result.error.userMessage)
    await loadDetail(batchId)
  }
  catch {
    showError('图片选择没有完成，请稍后重试。')
  }
  finally {
    busy.value = false
  }
}

async function importFiles(files: File[]) {
  const batchId = detail.value?.batch.id
  if (!desktopAvailable || !batchId || busy.value || !files.length) return
  busy.value = true
  errorMessage.value = ''
  try {
    for (const file of files.slice(0, 150)) {
      const bytes = [...new Uint8Array(await file.arrayBuffer())]
      const result = normalizeAppResult(await commands.captureImportBytes({
        batchId,
        clientUploadId: crypto.randomUUID(),
        sourceName: file.name || 'clipboard-image',
        sourceSequence: null,
        bytes,
      }))
      if (!result.ok) {
        showError(`${file.name || '图片'}：${result.error.userMessage}`)
        break
      }
    }
    await loadDetail(batchId)
  }
  catch {
    showError('拖入或粘贴的图片没有全部保存；已成功的图片仍在批次中。')
    await loadDetail(batchId)
  }
  finally {
    busy.value = false
  }
}

async function finishCollecting(subject: string) {
  const current = detail.value
  if (!desktopAvailable || !current || busy.value) return
  busy.value = true
  try {
    const result = normalizeAppResult(await commands.captureBatchUpdate({
      batchId: current.batch.id,
      expectedRevision: current.batch.revision,
      subject,
      finishCollecting: true,
    }))
    if (result.ok) await loadDetail(current.batch.id)
    else showError(result.error.userMessage)
  }
  catch {
    showError('没有结束采集，请稍后重试。')
  }
  finally {
    busy.value = false
  }
}

async function applyLayout(mode: CaptureLayoutMode, questions: number, answers: number, splitIndex: number | null) {
  const current = detail.value
  if (!desktopAvailable || !current || busy.value) return
  busy.value = true
  try {
    const result = normalizeAppResult(await commands.captureLayoutApply({
      batchId: current.batch.id,
      expectedRevision: current.batch.revision,
      mode,
      questionImagesPerDraft: questions,
      answerImagesPerDraft: answers,
      splitIndex,
    }))
    if (result.ok) detail.value = result.data
    else showError(result.error.userMessage)
  }
  catch {
    showError('整理模板没有应用，原有分组仍会保留。')
  }
  finally {
    busy.value = false
  }
}

async function createDraft() {
  const current = detail.value
  if (!desktopAvailable || !current || busy.value) return
  busy.value = true
  try {
    const result = normalizeAppResult(await commands.captureDraftCreate(current.batch.id, current.batch.revision))
    if (result.ok) detail.value = result.data
    else showError(result.error.userMessage)
  }
  catch {
    showError('空白草稿没有创建成功。')
  }
  finally {
    busy.value = false
  }
}

async function moveItem(target: { itemId: string, targetDraftId: string | null, targetRole: 'question' | 'answer' | null, targetPosition: number }) {
  const current = detail.value
  if (!desktopAvailable || !current || busy.value) return
  busy.value = true
  try {
    const result = normalizeAppResult(await commands.captureItemMove({
      batchId: current.batch.id,
      expectedRevision: current.batch.revision,
      ...target,
    }))
    if (result.ok) detail.value = result.data
    else {
      showError(result.error.userMessage)
      if (result.error.code === 'capture_revision_conflict') await loadDetail(current.batch.id)
    }
  }
  catch {
    showError('图片没有移动成功，请刷新批次后重试。')
  }
  finally {
    busy.value = false
  }
}

async function updateDraft(draft: CaptureDraftSummary, subject: string, tags: string[], note: string) {
  const current = detail.value
  if (!desktopAvailable || !current || busy.value) return
  busy.value = true
  try {
    const result = normalizeAppResult(await commands.captureDraftUpdate({
      batchId: current.batch.id,
      expectedRevision: current.batch.revision,
      draftId: draft.id,
      subject,
      tags,
      note,
    }))
    if (result.ok) detail.value = result.data
    else showError(result.error.userMessage)
  }
  catch {
    showError('草稿文字没有保存成功，请重试。')
  }
  finally {
    busy.value = false
  }
}

async function removeItem(itemId: string) {
  const current = detail.value
  if (!desktopAvailable || !current || busy.value) return
  if (!window.confirm('删除这张采集图片？如果没有其他引用，对应的加密资产也会被清理。')) return
  busy.value = true
  try {
    const result = normalizeAppResult(await commands.captureItemRemove(current.batch.id, current.batch.revision, itemId))
    if (result.ok) {
      detail.value = result.data
      delete previews[itemId]
    }
    else showError(result.error.userMessage)
  }
  catch {
    showError('图片没有删除成功。')
  }
  finally {
    busy.value = false
  }
}

async function commitReady() {
  const current = detail.value
  if (!desktopAvailable || !current || busy.value) return
  busy.value = true
  try {
    const result = normalizeAppResult(await commands.captureCommitReady(current.batch.id, current.batch.revision))
    if (result.ok) {
      await loadDetail(current.batch.id)
      await loadBatches()
    }
    else showError(result.error.userMessage)
  }
  catch {
    showError('批量入库没有完成，所有草稿仍保持原样，可以直接重试。')
  }
  finally {
    busy.value = false
  }
}

async function loadPreview(itemId: string) {
  const batchId = detail.value?.batch.id
  if (!desktopAvailable || !batchId || previews[itemId]) return
  try {
    const result = normalizeAppResult(await commands.captureItemPreview(batchId, itemId))
    if (!result.ok) return
    previews[itemId] = result.data.dataUrl
    previewOrder.push(itemId)
    while (previewOrder.length > 40) {
      const expired = previewOrder.shift()
      if (expired && expired !== itemId) delete previews[expired]
    }
  }
  catch {
    // A failed thumbnail must not interrupt organizing the rest of the batch.
  }
}

function handlePaste(event: ClipboardEvent) {
  if (!detail.value || detail.value.batch.state === 'completed') return
  if (event.target instanceof HTMLInputElement || event.target instanceof HTMLTextAreaElement) return
  const files = [...(event.clipboardData?.items ?? [])]
    .filter(item => item.kind === 'file' && item.type.startsWith('image/'))
    .map(item => item.getAsFile())
    .filter((file): file is File => Boolean(file))
  if (!files.length) return
  event.preventDefault()
  void importFiles(files)
}

function scheduleRefresh(batchId: string) {
  if (refreshTimer) clearTimeout(refreshTimer)
  refreshTimer = setTimeout(() => {
    if (detail.value?.batch.id === batchId) void loadDetail(batchId)
    else void loadBatches()
  }, 120)
}

onMounted(async () => {
  window.addEventListener('paste', handlePaste)
  if (!desktopAvailable) {
    showError('浏览器预览只展示界面；请在 Windows 桌面应用中使用加密采集箱。')
    return
  }
  await loadBatches()
  unlisten = await listen<{ batchId: string }>('capture_batch_changed', event => scheduleRefresh(event.payload.batchId))
})

onBeforeUnmount(() => {
  window.removeEventListener('paste', handlePaste)
  if (refreshTimer) clearTimeout(refreshTimer)
  unlisten?.()
})
</script>

<template>
  <CaptureWorkspace
    :batches="batches"
    :detail="detail"
    :previews="previews"
    :busy="busy"
    :error-message="errorMessage"
    :desktop-available="desktopAvailable"
    @create-batch="createBatch"
    @open-batch="loadDetail"
    @back="detail = undefined; loadBatches()"
    @discard-batch="discardBatch"
    @import-select="importSelect"
    @import-files="importFiles"
    @finish-collecting="finishCollecting"
    @apply-layout="applyLayout"
    @create-draft="createDraft"
    @move-item="moveItem"
    @update-draft="updateDraft"
    @remove-item="removeItem"
    @commit-ready="commitReady"
    @preview="loadPreview"
    @mobile-capture="showError('手机扫码服务将在下一阶段接入；当前可先用电脑批量选择、拖入或 Ctrl + V。')"
  />
</template>
