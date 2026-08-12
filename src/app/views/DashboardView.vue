<script setup lang="ts">
import { isTauri } from '@tauri-apps/api/core'
import { inject, onMounted, ref } from 'vue'
import { useRouter } from 'vue-router'
import TrainingDashboard from '@/modules/dashboard/components/TrainingDashboard.vue'
import QuickSessionDialog from '@/modules/review/components/QuickSessionDialog.vue'
import { commands, type DashboardOverview, type ReviewQuickStartInput } from '@/shared/api/bindings'
import { normalizeAppResult } from '@/shared/api/normalize-result'
import { syncControllerKey } from '../sync-controller'

const router = useRouter()
const syncController = inject(syncControllerKey, undefined)
const overview = ref<DashboardOverview | null>(null)
const loading = ref(true)
const errorMessage = ref('')
let requestSequence = 0
const quickOpen = ref(false)
const quickBusy = ref(false)
const quickError = ref('')

async function loadOverview() {
  const sequence = ++requestSequence
  loading.value = true
  errorMessage.value = ''
  try {
    if (!isTauri()) {
      errorMessage.value = '训练台需要从桌面应用的本地加密资料库读取数据。'
      return
    }
    const utcOffsetMinutes = -new Date().getTimezoneOffset()
    const result = normalizeAppResult(await commands.dashboardOverview(utcOffsetMinutes))
    if (sequence !== requestSequence) return
    if (result.ok) overview.value = result.data
    else {
      overview.value = null
      errorMessage.value = result.error.userMessage
    }
  } catch {
    if (sequence === requestSequence) {
      overview.value = null
      errorMessage.value = '本地资料库连接中断，请重新读取；若仍失败，请重新打开应用。'
    }
  } finally {
    if (sequence === requestSequence) loading.value = false
  }
}

onMounted(loadOverview)

function openQuickSession() {
  quickError.value = ''
  quickOpen.value = true
}

async function startQuickSession(input: ReviewQuickStartInput) {
  if (quickBusy.value) return
  quickBusy.value = true
  quickError.value = ''
  try {
    const result = normalizeAppResult(await commands.reviewQuickStart(input))
    if (!result.ok) {
      quickError.value = result.error.userMessage
      return
    }
    syncController?.scheduleMutation()
    quickOpen.value = false
    await router.push({ name: 'review' })
  }
  catch {
    quickError.value = '快速训练没有准备完成，请稍后重试。'
  }
  finally {
    quickBusy.value = false
  }
}
</script>

<template>
  <TrainingDashboard
    :overview="overview"
    :loading="loading"
    :error-message="errorMessage"
    @retry="loadOverview"
    @start-review="router.push({ name: 'review' })"
    @open-quick="openQuickSession"
    @open-inbox="router.push({ name: 'inbox' })"
    @open-library="router.push({ name: 'library' })"
    @open-report="router.push({ name: 'report' })"
  />
  <QuickSessionDialog
    :open="quickOpen"
    :busy="quickBusy"
    :error-message="quickError"
    @close="quickOpen = false"
    @start="startQuickSession"
  />
</template>
