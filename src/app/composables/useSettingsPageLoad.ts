import { readonly, ref, type Ref } from 'vue'
import type { AppResult } from '../../shared/api/app-result'
import type {
  ReviewPreferences,
  SettingsOverview,
  SubjectPreferences,
} from '../../shared/api/bindings'

interface SettingsLoadRevisions {
  subjects: number
  review: number
}

interface SettingsSupplementaryTask {
  label: string
  run: () => Promise<unknown>
}

interface SettingsPageLoadOptions {
  errorMessage: Ref<string>
  blockedMessage: () => string | undefined
  isDesktop: () => boolean
  onBrowserPreview: () => void
  revisions: () => SettingsLoadRevisions
  loadBackend: () => Promise<unknown>
  restoreSession: () => Promise<unknown>
  loadOverview: () => Promise<AppResult<SettingsOverview>>
  loadSubjects: () => Promise<AppResult<SubjectPreferences>>
  loadReview: () => Promise<AppResult<ReviewPreferences>>
  applyOverview: (value: SettingsOverview) => void
  applySubjects: (value: SubjectPreferences) => void
  applyReview: (value: ReviewPreferences) => void
  supplementaryTasks: readonly SettingsSupplementaryTask[]
}

interface SettingsLoadFailure {
  label: string
  userMessage?: string
}

function failureMessage(failures: SettingsLoadFailure[]): string {
  if (failures.length === 1 && failures[0]?.userMessage) return failures[0].userMessage
  const labels = [...new Set(failures.map(failure => failure.label))]
  return `部分设置暂时无法读取：${labels.join('、')}。其他设置仍可使用，可点击刷新重试。`
}

export function useSettingsPageLoad(options: SettingsPageLoadOptions) {
  const loading = ref(true)
  let inFlight = false

  async function runNamed(
    label: string,
    operation: () => Promise<unknown>,
  ): Promise<SettingsLoadFailure | undefined> {
    try {
      await operation()
      return undefined
    }
    catch {
      return { label }
    }
  }

  async function runResult<T>(
    label: string,
    operation: () => Promise<AppResult<T>>,
    apply: (value: T) => void,
  ): Promise<SettingsLoadFailure | undefined> {
    try {
      const result = await operation()
      if (!result.ok) return { label, userMessage: result.error.userMessage }
      apply(result.data)
      return undefined
    }
    catch {
      return { label }
    }
  }

  async function load(): Promise<boolean> {
    if (inFlight) return false
    const blocked = options.blockedMessage()
    if (blocked) {
      loading.value = false
      options.errorMessage.value = blocked
      return false
    }

    inFlight = true
    loading.value = true
    options.errorMessage.value = ''
    const revisionAtStart = options.revisions()
    try {
      const backendTask = runNamed('同步设置', options.loadBackend)
      if (!options.isDesktop()) {
        options.onBrowserPreview()
        const backendFailure = await backendTask
        if (backendFailure) options.errorMessage.value = failureMessage([backendFailure])
        return !backendFailure
      }

      const outcomes = await Promise.all([
        backendTask,
        runNamed('云端账户', options.restoreSession),
        runResult('资料库概览', options.loadOverview, options.applyOverview),
        runResult('科目配置', options.loadSubjects, (value) => {
          if (options.revisions().subjects === revisionAtStart.subjects) {
            options.applySubjects(value)
          }
        }),
        runResult('训练节奏', options.loadReview, (value) => {
          if (options.revisions().review === revisionAtStart.review) {
            options.applyReview(value)
          }
        }),
        ...options.supplementaryTasks.map(task => runNamed(task.label, task.run)),
      ])
      const failures = outcomes.filter(
        (outcome): outcome is SettingsLoadFailure => Boolean(outcome),
      )
      if (failures.length) options.errorMessage.value = failureMessage(failures)
      return failures.length === 0
    }
    finally {
      inFlight = false
      loading.value = false
    }
  }

  return {
    loading: readonly(loading),
    load,
  }
}
