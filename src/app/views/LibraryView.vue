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
let refreshSequence = 0

async function refresh() {
  const sequence = ++refreshSequence
  loading.value = true
  errorMessage.value = ''
  try {
    if (!isTauri()) {
      problems.value = []
      return
    }

    const requestedStatus = status.value
    const [contextResult, problemResult] = await Promise.all([
      commands.libraryContext(),
      commands.problemList(requestedStatus),
    ])
    if (sequence !== refreshSequence) return
    const context = normalizeAppResult(contextResult)
    const list = normalizeAppResult(problemResult)
    if (context.ok) profileName.value = context.data.profileName
    if (list.ok) problems.value = list.data
    else errorMessage.value = list.error.userMessage
  }
  catch {
    if (sequence === refreshSequence) {
      errorMessage.value = '题库连接中断，请重新打开应用后再试。'
    }
  }
  finally {
    if (sequence === refreshSequence) loading.value = false
  }
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
