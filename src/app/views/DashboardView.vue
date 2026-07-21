<script setup lang="ts">
import { isTauri } from '@tauri-apps/api/core'
import { onMounted, ref } from 'vue'
import { useRouter } from 'vue-router'
import TrainingDashboard from '@/modules/dashboard/components/TrainingDashboard.vue'
import { commands, type DashboardOverview } from '@/shared/api/bindings'
import { normalizeAppResult } from '@/shared/api/normalize-result'

const router = useRouter()
const overview = ref<DashboardOverview | null>(null)
const loading = ref(true)
const errorMessage = ref('')
let requestSequence = 0

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
</script>

<template>
  <TrainingDashboard
    :overview="overview"
    :loading="loading"
    :error-message="errorMessage"
    @retry="loadOverview"
    @start-review="router.push({ name: 'review' })"
    @open-inbox="router.push({ name: 'inbox' })"
    @open-library="router.push({ name: 'library' })"
    @open-report="router.push({ name: 'report' })"
  />
</template>
