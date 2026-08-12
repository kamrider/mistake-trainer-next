<script setup lang="ts">
import { isTauri } from '@tauri-apps/api/core'
import { computed, inject, onBeforeUnmount, onMounted, ref, watch } from 'vue'
import { onBeforeRouteLeave, useRoute, useRouter } from 'vue-router'
import LibraryWorkspace from '../../modules/library/components/LibraryWorkspace.vue'
import LibraryBulkMetadataDialog from '../../modules/library/components/LibraryBulkMetadataDialog.vue'
import ProblemDetailDrawer from '../../modules/library/components/ProblemDetailDrawer.vue'
import { useLibraryBatchStatus } from '../../modules/library/composables/useLibraryBatchStatus'
import { useLibraryProblemActions } from '../../modules/library/composables/useLibraryProblemActions'
import { useLibraryReviewLaunch } from '../../modules/library/composables/useLibraryReviewLaunch'
import {
  EMPTY_LIBRARY_FILTERS,
  type LibraryAdvancedFilters,
} from '../../modules/library/domain/libraryFilters'
import { commands, type ProblemDetail, type ProblemStatusFilter, type ProblemSummary } from '../../shared/api/bindings'
import { normalizeAppResult } from '../../shared/api/normalize-result'
import { useDurableActionGuard } from '../composables/useDurableActionGuard'
import type { NavigationAttempt } from '../composables/useUnsavedChangesGuard'
import { syncControllerKey } from '../sync-controller'
import { workspaceTransitionGuardKey } from '../workspace-transition-guard'

const syncController = inject(syncControllerKey, undefined)
const workspaceTransitionGuard = inject(workspaceTransitionGuardKey, undefined)
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
const changingBatchStatus = ref<ProblemStatusFilter | null>(null)
const bulkMetadataOpen = ref(false)
const bulkMetadataBusy = ref(false)
const advancedFilters = ref<LibraryAdvancedFilters>({
  ...EMPTY_LIBRARY_FILTERS,
  subjects: typeof route.query.subject === 'string' ? [route.query.subject] : [],
  tags: typeof route.query.tag === 'string' ? [route.query.tag] : [],
})
const knownSubjects = ref<string[]>([])
const knownTags = ref<string[]>([])
let refreshSequence = 0
let searchTimer: number | undefined
let detailSequence = 0
let viewActive = true
const detailOpen = ref(false)
const detailLoading = ref(false)
const detail = ref<ProblemDetail>()
const detailError = ref('')
const detailSaving = ref(false)
const libraryOperationBusy = computed(() =>
  Boolean(startingExperience.value || changingBatchStatus.value || bulkMetadataBusy.value))
const durableTransitionBlockedMessage = '题库操作正在进行，请等待完成后再离开。'
const { attemptLeave: attemptDurableTransition } = useDurableActionGuard({
  busy: () => libraryOperationBusy.value,
  onBlocked: () => {
    if (detailOpen.value) detailError.value = durableTransitionBlockedMessage
    else errorMessage.value = durableTransitionBlockedMessage
  },
  ...(workspaceTransitionGuard
    ? { registerContextTransition: workspaceTransitionGuard.register }
    : {}),
})
watch(libraryOperationBusy, (busy) => {
  if (busy) return
  if (errorMessage.value === durableTransitionBlockedMessage) errorMessage.value = ''
  if (detailError.value === durableTransitionBlockedMessage) detailError.value = ''
})
onBeforeRouteLeave(attemptDurableTransition)
const detailIndex = computed(() => detail.value
  ? problems.value.findIndex(problem => problem.id === detail.value?.id)
  : -1)
const previousProblemId = computed(() => detailIndex.value > 0
  ? problems.value[detailIndex.value - 1]?.id ?? null
  : null)
const nextProblemId = computed(() => detailIndex.value >= 0 && detailIndex.value < problems.value.length - 1
  ? problems.value[detailIndex.value + 1]?.id ?? null
  : null)

function registerDetailNavigation(attempt: NavigationAttempt) {
  return router.beforeEach((to, from) => {
    if (from.name !== 'library' || to.name === 'library') return true
    return attempt()
  })
}

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
      commands.problemList({
        status: requestedStatus,
        search: requestedSearch || null,
        subjects: advancedFilters.value.subjects,
        tags: advancedFilters.value.tags,
        reviewState: advancedFilters.value.reviewState,
        answerState: advancedFilters.value.answerState,
      }),
    ])
    if (sequence !== refreshSequence) return
    const context = normalizeAppResult(contextResult)
    const list = normalizeAppResult(problemResult)
    if (context.ok) profileName.value = context.data.profileName
    if (list.ok) {
      problems.value = list.data
      knownSubjects.value = [...new Set([
        ...knownSubjects.value,
        ...list.data.map(problem => problem.subject).filter(Boolean),
      ])].sort((left, right) => left.localeCompare(right, 'zh-CN'))
      knownTags.value = [...new Set([
        ...knownTags.value,
        ...list.data.flatMap(problem => problem.tags ?? []),
      ])].sort((left, right) => left.localeCompare(right, 'zh-CN'))
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

async function changeAdvancedFilters(filters: LibraryAdvancedFilters) {
  advancedFilters.value = {
    ...filters,
    subjects: [...filters.subjects],
    tags: [...filters.tags],
  }
  selectedProblemIds.value = []
  await refresh()
}

async function submitBulkMetadata(change: {
  subject: string | null
  addTags: string[]
  removeTags: string[]
}) {
  if (bulkMetadataBusy.value || selectedProblemIds.value.length === 0) return
  bulkMetadataBusy.value = true
  errorMessage.value = ''
  try {
    const result = normalizeAppResult(await commands.problemBulkMetadata({
      problemIds: [...selectedProblemIds.value],
      subject: change.subject,
      addTags: change.addTags,
      removeTags: change.removeTags,
    }))
    if (!result.ok) {
      errorMessage.value = result.error.userMessage
      return
    }
    bulkMetadataOpen.value = false
    selectedProblemIds.value = []
    syncController?.scheduleMutation()
    await refresh()
  }
  catch {
    errorMessage.value = '批量修改未完成，请稍后重试。'
  }
  finally {
    bulkMetadataBusy.value = false
  }
}

async function openDetail(problemId: string) {
  const sequence = ++detailSequence
  detailOpen.value = true
  detailLoading.value = true
  if (detail.value?.id !== problemId) detail.value = undefined
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

const problemActions = useLibraryProblemActions({
  activeProblemId: () => viewActive && detailOpen.value ? detail.value?.id : undefined,
  isSaving: () => detailSaving.value,
  onSavingChange: value => { detailSaving.value = value },
  onError: message => { detailError.value = message },
  onUpdateSuccess: (input) => {
    if (detail.value?.id !== input.problemId) return
    detail.value = {
      ...detail.value,
      subject: input.subject,
      note: input.note,
      tags: [...input.tags],
      timeLimitSeconds: input.timeLimitSeconds,
    }
  },
  refresh,
  reloadDetail: openDetail,
  closeDetail: (problemId) => {
    if (detailOpen.value && detail.value?.id === problemId) closeDetail()
  },
  scheduleSync: () => syncController?.scheduleMutation(),
  operations: {
    update: async input => normalizeAppResult(await commands.problemUpdate(input)),
    changeStatus: async input => normalizeAppResult(await commands.problemChangeStatus(input)),
  },
})
const { updateProblem, changeProblemStatus } = problemActions

const batchStatus = useLibraryBatchStatus({
  selectedProblemIds: () => selectedProblemIds.value,
  onSelectionChange: problemIds => { selectedProblemIds.value = problemIds },
  changingStatus: () => changingBatchStatus.value,
  onChangingStatusChange: nextStatus => { changingBatchStatus.value = nextStatus },
  onError: message => { errorMessage.value = message },
  scheduleSync: () => syncController?.scheduleMutation(),
  refresh,
  operation: async input => normalizeAppResult(await commands.problemChangeStatus(input)),
})
const { changeBatchStatus } = batchStatus

const isLibraryPreview = () => !isTauri()
  && import.meta.env.DEV
  && route.query.preview === 'library'
const reviewLaunch = useLibraryReviewLaunch({
  selectedProblemIds: () => selectedProblemIds.value,
  startingExperience: () => startingExperience.value,
  ownsRoute: () => viewActive && route.name === 'library',
  onSelectionChange: problemIds => { selectedProblemIds.value = problemIds },
  onStartingExperienceChange: experience => { startingExperience.value = experience },
  onListError: message => { errorMessage.value = message },
  onDetailError: message => { detailError.value = message },
  startManual: async input => isLibraryPreview()
    ? { ok: true, data: undefined }
    : normalizeAppResult(await commands.reviewManualStart(input)),
  startExam: async input => isLibraryPreview()
    ? { ok: true, data: undefined }
    : normalizeAppResult(await commands.reviewExamStart(input)),
  navigate: async (experience) => {
    await router.push(isLibraryPreview()
      ? {
          name: 'review',
          query: { preview: experience === 'exam' ? 'exam-answering' : 'manual-review' },
        }
      : { name: 'review' })
  },
})
const { startReview } = reviewLaunch

async function trainProblem(problemId: string) {
  await startReview([problemId], true, 'review')
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

onMounted(refresh)
onBeforeUnmount(() => {
  viewActive = false
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
    :changing-batch-status="changingBatchStatus"
    :bulk-metadata-busy="bulkMetadataBusy"
    :advanced-filters="advancedFilters"
    :subject-options="knownSubjects"
    :tag-options="knownTags"
    :error-message="errorMessage"
    @capture="router.push({ name: 'inbox' })"
    @status-change="changeStatus"
    @search-change="changeSearch"
    @filters-change="changeAdvancedFilters"
    @open-detail="openDetail"
    @toggle-selection="toggleSelection"
    @batch-status="changeBatchStatus"
    @train-selection="startReview()"
    @start-exam="startReview(selectedProblemIds, false, 'exam')"
    @select-all="selectAllVisible"
    @clear-selection="clearSelection"
    @bulk-metadata="bulkMetadataOpen = true"
  />
  <LibraryBulkMetadataDialog
    :open="bulkMetadataOpen"
    :selected-count="selectedProblemIds.length"
    :busy="bulkMetadataBusy"
    @close="bulkMetadataOpen = false"
    @submit="submitBulkMetadata"
  />
  <ProblemDetailDrawer
    v-if="detailOpen"
    :detail="detail"
    :loading="detailLoading && !detail"
    :saving="detailSaving || Boolean(startingExperience)"
    :error-message="detailError"
    :previous-problem-id="previousProblemId"
    :next-problem-id="nextProblemId"
    :register-navigation="registerDetailNavigation"
    :register-context-transition="workspaceTransitionGuard?.register"
    @close="closeDetail"
    @train="trainProblem"
    @update="updateProblem"
    @status="changeProblemStatus"
    @navigate="openDetail"
  />
</template>
