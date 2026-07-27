<script setup lang="ts">
import { isTauri } from '@tauri-apps/api/core'
import { computed, inject, onBeforeUnmount, onMounted, ref } from 'vue'
import { useRoute, useRouter } from 'vue-router'
import LibraryWorkspace from '../../modules/library/components/LibraryWorkspace.vue'
import ProblemDetailDrawer from '../../modules/library/components/ProblemDetailDrawer.vue'
import { commands, type ProblemDetail, type ProblemStatusFilter, type ProblemSummary } from '../../shared/api/bindings'
import { normalizeAppResult } from '../../shared/api/normalize-result'
import { syncControllerKey } from '../sync-controller'

const syncController = inject(syncControllerKey, undefined)
const router = useRouter()
const route = useRoute()
const profileName = ref('本机学习档案')
const status = ref<ProblemStatusFilter>('active')
const search = ref('')
const problems = ref<ProblemSummary[]>([])
const selectedProblemIds = ref<string[]>([])
const loading = ref(true)
const errorMessage = ref('')
const startingExperience = ref<'review' | 'exam' | null>(null)
let refreshSequence = 0
let searchTimer: number | undefined
let detailSequence = 0
const detailOpen = ref(false)
const detailLoading = ref(false)
const detail = ref<ProblemDetail>()
const detailError = ref('')
const detailSaving = ref(false)
const detailIndex = computed(() => detail.value
  ? problems.value.findIndex(problem => problem.id === detail.value?.id)
  : -1)
const previousProblemId = computed(() => detailIndex.value > 0
  ? problems.value[detailIndex.value - 1]?.id ?? null
  : null)
const nextProblemId = computed(() => detailIndex.value >= 0 && detailIndex.value < problems.value.length - 1
  ? problems.value[detailIndex.value + 1]?.id ?? null
  : null)

function loadDevelopmentPreview() {
  profileName.value = '本机学习档案'
  problems.value = [
    { id: 'preview-math', subject: '数学', note: '圆锥曲线：先确认长轴方向，再写焦点关系。', tags: ['圆锥曲线', '粗心'], status: 'active', questionAssetCount: 2, answerAssetCount: 1, questionPreviewDataUrl: null, updatedAtUtcMs: Date.now() },
    { id: 'preview-physics', subject: '物理', note: '受力分析遗漏了接触面的支持力。', tags: ['力学'], status: 'active', questionAssetCount: 1, answerAssetCount: 2, questionPreviewDataUrl: null, updatedAtUtcMs: Date.now() - 1 },
    { id: 'preview-chemistry', subject: '化学', note: '平衡移动方向与转化率变化需要分开判断。', tags: ['化学平衡'], status: 'active', questionAssetCount: 1, answerAssetCount: 1, questionPreviewDataUrl: null, updatedAtUtcMs: Date.now() - 2 },
    { id: 'preview-english', subject: '英语', note: '长难句先切分从句，再确认非谓语逻辑主语。', tags: ['长难句'], status: 'active', questionAssetCount: 2, answerAssetCount: 2, questionPreviewDataUrl: null, updatedAtUtcMs: Date.now() - 3 },
  ]
}

async function refresh() {
  const sequence = ++refreshSequence
  loading.value = true
  errorMessage.value = ''
  try {
    if (!isTauri()) {
      if (import.meta.env.DEV && route.query.preview === 'library') loadDevelopmentPreview()
      else problems.value = []
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
  await startReview([problemId], true, 'review')
}

async function updateProblem(input: { problemId: string; subject: string; note: string; tags: string[]; timeLimitSeconds: number | null }) {
  detailSaving.value = true
  detailError.value = ''
  try {
    const result = normalizeAppResult(await commands.problemUpdate(input))
    if (!result.ok) {
      detailError.value = result.error.userMessage
      return
    }
    syncController?.scheduleMutation()
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
    syncController?.scheduleMutation()
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
  if (selectedProblemIds.value.includes(problemId)) {
    selectedProblemIds.value = selectedProblemIds.value.filter(id => id !== problemId)
    return
  }
  if (selectedProblemIds.value.length >= 100) {
    errorMessage.value = '一次训练最多选择 100 道题。'
    return
  }
  errorMessage.value = ''
  selectedProblemIds.value = [...selectedProblemIds.value, problemId]
}

function selectAllVisible() {
  const nextIds = problems.value.slice(0, 100).map(problem => problem.id)
  selectedProblemIds.value = nextIds
  errorMessage.value = problems.value.length > 100
    ? '已按当前顺序选择前 100 道题；单次训练最多 100 道。'
    : ''
}

function clearSelection() {
  selectedProblemIds.value = []
  errorMessage.value = ''
}

async function startReview(
  problemIds = selectedProblemIds.value,
  fromDetail = false,
  experience: 'review' | 'exam' = 'review',
) {
  if (problemIds.length === 0 || startingExperience.value) return
  startingExperience.value = experience
  if (fromDetail) detailError.value = ''
  else errorMessage.value = ''
  let sessionCreated = false
  try {
    if (!isTauri() && import.meta.env.DEV && route.query.preview === 'library') {
      await router.push({
        name: 'review',
        query: { preview: experience === 'exam' ? 'exam-answering' : 'manual-review' },
      })
      selectedProblemIds.value = []
      return
    }
    const result = normalizeAppResult(experience === 'exam'
      ? await commands.reviewExamStart({ problemIds })
      : await commands.reviewManualStart({ problemIds }))
    if (!result.ok) {
      if (fromDetail) detailError.value = result.error.userMessage
      else errorMessage.value = result.error.userMessage
      return
    }
    sessionCreated = true
    if (!fromDetail) selectedProblemIds.value = []
    await router.push({ name: 'review' })
  }
  catch {
    const message = sessionCreated
      ? `${experience === 'exam' ? '模拟考试' : '训练卡组'}已安全保存，可从侧边栏“训练室”继续。`
      : `${experience === 'exam' ? '模拟考试' : '训练卡组'}没有创建成功，请保持当前选择并稍后重试。`
    if (fromDetail) detailError.value = message
    else errorMessage.value = message
  }
  finally {
    startingExperience.value = null
  }
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
    syncController?.scheduleMutation()
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
    :starting-experience="startingExperience"
    :error-message="errorMessage"
    @capture="router.push({ name: 'inbox' })"
    @status-change="changeStatus"
    @search-change="changeSearch"
    @open-detail="openDetail"
    @toggle-selection="toggleSelection"
    @batch-status="changeBatchStatus"
    @train-selection="startReview()"
    @start-exam="startReview(selectedProblemIds, false, 'exam')"
    @select-all="selectAllVisible"
    @clear-selection="clearSelection"
  />
  <ProblemDetailDrawer
    v-if="detailOpen"
    :detail="detail"
    :loading="detailLoading"
    :saving="detailSaving || Boolean(startingExperience)"
    :error-message="detailError"
    :previous-problem-id="previousProblemId"
    :next-problem-id="nextProblemId"
    @close="closeDetail"
    @train="trainProblem"
    @update="updateProblem"
    @status="changeProblemStatus"
    @navigate="openDetail"
  />
</template>
