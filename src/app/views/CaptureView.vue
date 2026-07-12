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
  const result = normalizeAppResult(await commands.captureList())
  if (result.ok) assets.value = result.data
  else errorMessage.value = result.error.userMessage
}

async function select(role: 'question' | 'answer') {
  errorMessage.value = ''
  if (!isTauri()) {
    errorMessage.value = '请在 Windows 桌面应用中选择图片；浏览器预览只展示界面。'
    return
  }
  const result = normalizeAppResult(await commands.captureSelect(role))
  if (result.ok) await refresh()
  else errorMessage.value = result.error.userMessage
}

async function remove(stagedAssetId: string) {
  errorMessage.value = ''
  if (!isTauri()) return
  const result = normalizeAppResult(await commands.captureRemove(stagedAssetId))
  if (result.ok) await refresh()
  else errorMessage.value = result.error.userMessage
}

async function commit(input: CaptureCommitInput) {
  if (!isTauri()) return
  saving.value = true
  errorMessage.value = ''
  const result = normalizeAppResult(await commands.captureCommit(input))
  saving.value = false
  if (result.ok) await router.push({ name: 'library' })
  else errorMessage.value = result.error.userMessage
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
