<script setup lang="ts">
import { isTauri } from '@tauri-apps/api/core'
import { onMounted, ref } from 'vue'
import { useRouter } from 'vue-router'
import CaptureWorkspace from '../../modules/capture/components/CaptureWorkspace.vue'
import { commands, type CaptureCommitInput, type StagedAsset } from '../../shared/api/bindings'
import { normalizeAppResult } from '../../shared/api/normalize-result'

const router = useRouter()
const assets = ref<StagedAsset[]>([])
const saving = ref(false)
const errorMessage = ref('')

async function refresh() {
  if (!isTauri()) return
  try {
    const result = normalizeAppResult(await commands.captureList())
    if (result.ok) assets.value = result.data
    else errorMessage.value = result.error.userMessage
  }
  catch {
    errorMessage.value = '暂存区连接中断，请重新打开应用后再试。'
  }
}

async function select(role: 'question' | 'answer') {
  errorMessage.value = ''
  if (!isTauri()) {
    errorMessage.value = '请在 Windows 桌面应用中选择图片；浏览器预览只展示界面。'
    return
  }
  try {
    const result = normalizeAppResult(await commands.captureSelect(role))
    if (result.ok) await refresh()
    else errorMessage.value = result.error.userMessage
  }
  catch {
    errorMessage.value = '没有完成图片选择，请稍后重试。'
  }
}

async function remove(stagedAssetId: string) {
  errorMessage.value = ''
  if (!isTauri()) return
  try {
    const result = normalizeAppResult(await commands.captureRemove(stagedAssetId))
    if (result.ok) await refresh()
    else errorMessage.value = result.error.userMessage
  }
  catch {
    errorMessage.value = '没有移除这张图片，请稍后重试。'
  }
}

async function commit(input: CaptureCommitInput) {
  if (!isTauri()) return
  saving.value = true
  errorMessage.value = ''
  try {
    const result = normalizeAppResult(await commands.captureCommit(input))
    if (result.ok) await router.push({ name: 'library' })
    else errorMessage.value = result.error.userMessage
  }
  catch {
    errorMessage.value = '保存连接中断，原图片仍在暂存区，可以直接重试。'
  }
  finally {
    saving.value = false
  }
}

onMounted(refresh)
</script>

<template>
  <CaptureWorkspace
    :assets="assets"
    :saving="saving"
    :error-message="errorMessage"
    @select="select"
    @remove="remove"
    @commit="commit"
  />
</template>
