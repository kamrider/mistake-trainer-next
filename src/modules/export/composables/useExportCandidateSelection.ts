import { computed, readonly, ref } from 'vue'
import type { AppResult } from '../../../shared/api/app-result'
import type { ExportCandidate, ExportCandidateSource } from '../../../shared/api/bindings'

interface ExportCandidateSelectionOptions {
  load: (source: ExportCandidateSource) => Promise<AppResult<ExportCandidate[]>>
  initialSource?: ExportCandidateSource
}

export function useExportCandidateSelection(options: ExportCandidateSelectionOptions) {
  const source = ref<ExportCandidateSource>(options.initialSource ?? 'due')
  const candidates = ref<ExportCandidate[]>([])
  const selectedIds = ref<string[]>([])
  const loading = ref(false)
  const error = ref('')
  const candidateItems = computed(() => candidates.value)
  const selectedProblemIds = computed(() => selectedIds.value)
  let loadedSource: ExportCandidateSource | undefined
  let inFlight = false

  async function loadCandidates(nextSource: ExportCandidateSource = source.value): Promise<boolean> {
    if (inFlight) return false

    inFlight = true
    loading.value = true
    error.value = ''
    const replacingSource = loadedSource !== nextSource
    const previousSelection = new Set(selectedIds.value)
    source.value = nextSource
    if (replacingSource) {
      candidates.value = []
      selectedIds.value = []
    }

    try {
      const result = await options.load(nextSource)
      if (!result.ok) {
        error.value = result.error.userMessage
        return false
      }

      candidates.value = result.data
      selectedIds.value = replacingSource
        ? result.data.map(candidate => candidate.id)
        : result.data
            .filter(candidate => previousSelection.has(candidate.id))
            .map(candidate => candidate.id)
      loadedSource = nextSource
      return true
    }
    catch {
      error.value = '可导出的题目没有读取成功，请稍后重试。'
      return false
    }
    finally {
      inFlight = false
      loading.value = false
    }
  }

  function changeSource(nextSource: ExportCandidateSource): Promise<boolean> {
    if (nextSource === source.value || inFlight) return Promise.resolve(false)
    return loadCandidates(nextSource)
  }

  function replacePreview(nextSource: ExportCandidateSource, items: ExportCandidate[]) {
    source.value = nextSource
    candidates.value = [...items]
    selectedIds.value = items.map(candidate => candidate.id)
    loadedSource = nextSource
    error.value = ''
  }

  function toggle(problemId: string) {
    if (!candidates.value.some(candidate => candidate.id === problemId)) return
    const next = new Set(selectedIds.value)
    if (next.has(problemId)) next.delete(problemId)
    else next.add(problemId)
    selectedIds.value = candidates.value
      .filter(candidate => next.has(candidate.id))
      .map(candidate => candidate.id)
  }

  function select(problemIds: string[]) {
    const next = new Set([...selectedIds.value, ...problemIds])
    selectedIds.value = candidates.value
      .filter(candidate => next.has(candidate.id))
      .map(candidate => candidate.id)
  }

  function clear() {
    selectedIds.value = []
  }

  return {
    source: readonly(source),
    candidates: candidateItems,
    selectedIds: selectedProblemIds,
    loading: readonly(loading),
    error: readonly(error),
    loadCandidates,
    changeSource,
    replacePreview,
    toggle,
    select,
    clear,
  }
}
