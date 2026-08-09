import { readonly, ref, shallowReadonly, shallowRef } from 'vue'
import type { AppResult } from '../../../shared/api/app-result'
import type {
  ReviewHistoryInput,
  ReviewHistoryItem,
  ReviewHistoryPage,
} from '../../../shared/api/bindings'

export type ReviewHistoryQuery = Omit<ReviewHistoryInput, 'cursor' | 'limit'>

interface ReviewHistoryListOptions {
  listPage: (input: ReviewHistoryInput) => Promise<AppResult<ReviewHistoryPage>>
  initialQuery: ReviewHistoryQuery
  pageSize: number
}

interface FailedHistoryRequest {
  input: ReviewHistoryInput
  append: boolean
}

const fallbackError = '复习历史暂时无法读取，请稍后重试。'

function copyQuery(query: ReviewHistoryQuery): ReviewHistoryQuery {
  return {
    range: query.range,
    rating: query.rating,
    subject: query.subject,
    search: query.search,
  }
}

function copyInput(input: ReviewHistoryInput): ReviewHistoryInput {
  return { ...copyQuery(input), cursor: input.cursor, limit: input.limit }
}

function appendUnique(
  current: ReviewHistoryItem[],
  incoming: ReviewHistoryItem[],
): ReviewHistoryItem[] {
  const seen = new Set(current.map(item => item.eventId))
  return [
    ...current,
    ...incoming.filter((item) => {
      if (seen.has(item.eventId)) return false
      seen.add(item.eventId)
      return true
    }),
  ]
}

export function useReviewHistoryList(options: ReviewHistoryListOptions) {
  const items = shallowRef<ReviewHistoryItem[]>([])
  const subjects = shallowRef<string[]>([])
  const nextCursor = ref<string | null>(null)
  const totalCount = ref(0)
  const loading = ref(false)
  const loadingMore = ref(false)
  const loaded = ref(false)
  const errorMessage = ref('')
  const stale = ref(false)
  let currentQuery = copyQuery(options.initialQuery)
  let failedRequest: FailedHistoryRequest | undefined
  let requestEpoch = 0

  async function request(input: ReviewHistoryInput, append: boolean): Promise<boolean> {
    const requestInput = copyInput(input)
    const epoch = ++requestEpoch
    failedRequest = undefined
    errorMessage.value = ''
    if (append) {
      loadingMore.value = true
    }
    else {
      loadingMore.value = false
      loading.value = true
    }

    try {
      const result = await options.listPage(requestInput)
      if (epoch !== requestEpoch) return false
      if (!result.ok) {
        errorMessage.value = result.error.userMessage
        stale.value = items.value.length > 0
        nextCursor.value = null
        failedRequest = { input: requestInput, append }
        return false
      }

      items.value = append
        ? appendUnique(items.value, result.data.items)
        : result.data.items
      subjects.value = result.data.availableSubjects
      nextCursor.value = result.data.nextCursor
      totalCount.value = result.data.totalCount
      loaded.value = true
      stale.value = false
      failedRequest = undefined
      return true
    }
    catch {
      if (epoch !== requestEpoch) return false
      errorMessage.value = fallbackError
      stale.value = items.value.length > 0
      nextCursor.value = null
      failedRequest = { input: requestInput, append }
      return false
    }
    finally {
      if (epoch === requestEpoch) {
        if (append) loadingMore.value = false
        else loading.value = false
      }
    }
  }

  function replace(query: ReviewHistoryQuery): Promise<boolean> {
    currentQuery = copyQuery(query)
    return request({ ...currentQuery, cursor: null, limit: options.pageSize }, false)
  }

  function loadMore(): Promise<boolean> {
    const cursor = nextCursor.value
    if (loading.value || loadingMore.value || stale.value || !cursor)
      return Promise.resolve(false)
    return request({ ...currentQuery, cursor, limit: options.pageSize }, true)
  }

  function retry(): Promise<boolean> {
    if (loading.value || loadingMore.value) return Promise.resolve(false)
    const failed = failedRequest
    if (!failed) return replace(currentQuery)
    return request(failed.input, failed.append)
  }

  return {
    items: shallowReadonly(items),
    subjects: shallowReadonly(subjects),
    nextCursor: readonly(nextCursor),
    totalCount: readonly(totalCount),
    loading: readonly(loading),
    loadingMore: readonly(loadingMore),
    loaded: readonly(loaded),
    errorMessage: readonly(errorMessage),
    stale: readonly(stale),
    replace,
    loadMore,
    retry,
  }
}
