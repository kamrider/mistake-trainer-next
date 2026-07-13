<script setup lang="ts">
import { isTauri } from '@tauri-apps/api/core'
import { onBeforeUnmount, onMounted, ref } from 'vue'
import { useRouter } from 'vue-router'
import LibraryWorkspace from '../../modules/library/components/LibraryWorkspace.vue'
import ProblemDetailDrawer from '../../modules/library/components/ProblemDetailDrawer.vue'
import { commands, type ProblemDetail, type ProblemStatusFilter, type ProblemSummary } from '../../shared/api/bindings'
import { normalizeAppResult } from '../../shared/api/normalize-result'

const router = useRouter()
const profileName = ref('本机学习档案')
const status = ref<ProblemStatusFilter>('active')
const search = ref('')
const problems = ref<ProblemSummary[]>([])
const selectedProblemIds = ref<string[]>([])
const loading = ref(true)
const errorMessage = ref('')
let refreshSequence = 0
let searchTimer: number | undefined
let detailSequence = 0
const detailOpen = ref(false)
const detailLoading = ref(false)
const detail = ref<ProblemDetail>()
const detailError = ref('')
const detailSaving = ref(false)

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
    const requestedSearch = search.value.trim()
    const [contextResult, problemResult] = await Promise.all([
      commands.libraryContext(),
      commands.problemList(requestedStatus, requestedSearch || null),
    ])
    if (sequence !== refreshSequence) return
    const context = normalizeAppResult(contextResult)
    const list = normalizeAppResult(problemResult)
    if (context.ok) profileName.value = context.data.profileName
    if (list.ok) {
      problems.value = list.data
      const visible = new Set(list.data.map(problem => problem.id))
      selectedProblemIds.value = selectedProblemIds.value.filter(id => visible.has(id))
    }
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

function changeSearch(nextSearch: string) {
  search.value = nextSearch
  selectedProblemIds.value = []
  if (searchTimer !== undefined) window.clearTimeout(searchTimer)
  searchTimer = window.setTimeout(() => void refresh(), 180)
}

async function openDetail(problemId: string) {
  const sequence = ++detailSequence
  detailOpen.value = true
  detailLoading.value = true
  detail.value = undefined
  detailError.value = ''
  try {
    if (!isTauri()) return
    const result = normalizeAppResult(await commands.problemDetail(problemId))
    if (sequence !== detailSequence) return
    if (result.ok) detail.value = result.data
    else detailError.value = result.error.userMessage
  }
  catch {
    if (sequence === detailSequence) detailError.value = '题目图片解密失败，请关闭详情后重试。'
  }
  finally {
    if (sequence === detailSequence) detailLoading.value = false
  }
}

function closeDetail() {
  detailSequence += 1
  detailOpen.value = false
  detail.value = undefined
  detailError.value = ''
}

async function trainProblem(problemId: string) {
  try {
    await router.push({ name: 'review', query: { problemId } })
  }
  catch {
    detailError.value = '训练页面暂时无法打开，请稍后重试。'
  }
}

async function updateProblem(input: { problemId: string; subject: string; note: string }) {
  detailSaving.value = true
  detailError.value = ''
  try {
    const result = normalizeAppResult(await commands.problemUpdate(input))
    if (!result.ok) {
      detailError.value = result.error.userMessage
      return
    }
    await refresh()
    await openDetail(input.problemId)
  }
  catch {
    detailError.value = '修改没有保存，请稍后重试。'
  }
  finally {
    detailSaving.value = false
  }
}

async function changeProblemStatus(problemId: string, targetStatus: ProblemStatusFilter) {
  detailSaving.value = true
  detailError.value = ''
  try {
    const result = normalizeAppResult(await commands.problemChangeStatus({
      problemIds: [problemId],
      targetStatus,
    }))
    if (!result.ok) {
      detailError.value = result.error.userMessage
      return
    }
    closeDetail()
    await refresh()
  }
  catch {
    detailError.value = '题目状态没有改变，请稍后重试。'
  }
  finally {
    detailSaving.value = false
  }
}

async function changeStatus(nextStatus: ProblemStatusFilter) {
  status.value = nextStatus
  selectedProblemIds.value = []
  await refresh()
}

function toggleSelection(problemId: string) {
  selectedProblemIds.value = selectedProblemIds.value.includes(problemId)
    ? selectedProblemIds.value.filter(id => id !== problemId)
    : [...selectedProblemIds.value, problemId]
}

async function changeBatchStatus(targetStatus: ProblemStatusFilter) {
  if (selectedProblemIds.value.length === 0) return
  errorMessage.value = ''
  try {
    const result = normalizeAppResult(await commands.problemChangeStatus({
      problemIds: selectedProblemIds.value,
      targetStatus,
    }))
    if (!result.ok) {
      errorMessage.value = result.error.userMessage
      return
    }
    selectedProblemIds.value = []
    await refresh()
  }
  catch {
    errorMessage.value = '批量操作没有完成，请稍后重试。'
  }
}

onMounted(refresh)
onBeforeUnmount(() => {
  if (searchTimer !== undefined) window.clearTimeout(searchTimer)
})
</script>

<template>
  <LibraryWorkspace
    :profile-name="profileName"
    :status="status"
    :search="search"
    :loading="loading"
    :problems="problems"
    :selected-problem-ids="selectedProblemIds"
    :error-message="errorMessage"
    @capture="router.push({ name: 'inbox' })"
    @status-change="changeStatus"
    @search-change="changeSearch"
    @open-detail="openDetail"
    @toggle-selection="toggleSelection"
    @batch-status="changeBatchStatus"
  />
  <ProblemDetailDrawer
    v-if="detailOpen"
    :detail="detail"
    :loading="detailLoading"
    :saving="detailSaving"
    :error-message="detailError"
    @close="closeDetail"
    @train="trainProblem"
    @update="updateProblem"
    @status="changeProblemStatus"
  />
</template>
