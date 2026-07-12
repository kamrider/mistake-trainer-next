<script setup lang="ts">
import { isTauri } from '@tauri-apps/api/core'
import { onMounted, ref } from 'vue'
import { useRouter } from 'vue-router'
import LibraryWorkspace from '../../modules/library/components/LibraryWorkspace.vue'
import { commands, type ProblemStatusFilter, type ProblemSummary } from '../../shared/api/bindings'
import { normalizeAppResult } from '../../shared/api/normalize-result'

const router = useRouter()
const profileName = ref('本机学习档案')
const status = ref<ProblemStatusFilter>('active')
const problems = ref<ProblemSummary[]>([])
const loading = ref(true)
const errorMessage = ref('')

async function refresh() {
  loading.value = true
  errorMessage.value = ''
  if (!isTauri()) {
    problems.value = []
    loading.value = false
    return
  }

  const [contextResult, problemResult] = await Promise.all([
    commands.libraryContext(),
    commands.problemList(status.value),
  ])
  const context = normalizeAppResult(contextResult)
  const list = normalizeAppResult(problemResult)
  if (context.ok) profileName.value = context.data.profileName
  if (list.ok) problems.value = list.data
  else errorMessage.value = list.error.userMessage
  loading.value = false
}

async function changeStatus(nextStatus: ProblemStatusFilter) {
  status.value = nextStatus
  await refresh()
}

onMounted(refresh)
</script>

<template>
  <LibraryWorkspace
    :profile-name="profileName"
    :status="status"
    :loading="loading"
    :problems="problems"
    :error-message="errorMessage"
    @capture="router.push({ name: 'inbox' })"
    @status-change="changeStatus"
  />
</template>
