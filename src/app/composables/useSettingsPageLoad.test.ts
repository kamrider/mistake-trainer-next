import { ref } from 'vue'
import { describe, expect, it, vi } from 'vitest'
import type { AppResult } from '../../shared/api/app-result'
import { failure, success } from '../../shared/api/app-result'
import type {
  ReviewPreferences,
  SettingsOverview,
  SubjectPreferences,
} from '../../shared/api/bindings'
import { useSettingsPageLoad } from './useSettingsPageLoad'

const overview: SettingsOverview = {
  activeProblemCount: 8,
  archivedProblemCount: 2,
  trashedProblemCount: 1,
  pendingOperationCount: 0,
  failedOperationCount: 0,
  unresolvedConflictCount: 0,
  localEncryptionReady: true,
  cloudSyncConfigured: false,
}
const subjects: SubjectPreferences = {
  enabledSubjects: ['语文', '数学'],
  customSubjects: [],
  captureSoundEnabled: true,
}
const review: ReviewPreferences = { focusPolicy: 'session_start' }

function deferred<T>() {
  let resolve!: (value: T) => void
  const promise = new Promise<T>((finish) => { resolve = finish })
  return { promise, resolve }
}

function harness() {
  let subjectRevision = 0
  let reviewRevision = 0
  const errorMessage = ref('older error')
  const operations = {
    loadBackend: vi.fn().mockResolvedValue(undefined),
    restoreSession: vi.fn().mockResolvedValue(undefined),
    loadOverview: vi.fn().mockResolvedValue(success(overview)),
    loadSubjects: vi.fn().mockResolvedValue(success(subjects)),
    loadReview: vi.fn().mockResolvedValue(success(review)),
    applyOverview: vi.fn(),
    applySubjects: vi.fn(),
    applyReview: vi.fn(),
    loadStorage: vi.fn().mockResolvedValue(undefined),
    loadDevice: vi.fn().mockResolvedValue(undefined),
    loadOcr: vi.fn().mockResolvedValue(undefined),
    onBrowserPreview: vi.fn(),
  }
  let desktop = true
  let blocked: string | undefined
  const controller = useSettingsPageLoad({
    errorMessage,
    blockedMessage: () => blocked,
    isDesktop: () => desktop,
    onBrowserPreview: operations.onBrowserPreview,
    revisions: () => ({ subjects: subjectRevision, review: reviewRevision }),
    loadBackend: operations.loadBackend,
    restoreSession: operations.restoreSession,
    loadOverview: operations.loadOverview,
    loadSubjects: operations.loadSubjects,
    loadReview: operations.loadReview,
    applyOverview: operations.applyOverview,
    applySubjects: operations.applySubjects,
    applyReview: operations.applyReview,
    supplementaryTasks: [
      { label: '存储状态', run: operations.loadStorage },
      { label: '设备状态', run: operations.loadDevice },
      { label: '智能功能', run: operations.loadOcr },
    ],
  })
  return {
    controller,
    errorMessage,
    operations,
    setDesktop(value: boolean) { desktop = value },
    setBlocked(value: string | undefined) { blocked = value },
    reviseSubjects() { subjectRevision += 1 },
    reviseReview() { reviewRevision += 1 },
  }
}

describe('useSettingsPageLoad', () => {
  it('isolates a rejected overview and still applies every healthy sibling', async () => {
    const h = harness()
    h.operations.loadOverview.mockRejectedValueOnce(new Error('overview offline'))

    await expect(h.controller.load()).resolves.toBe(false)

    expect(h.operations.applyOverview).not.toHaveBeenCalled()
    expect(h.operations.applySubjects).toHaveBeenCalledWith(subjects)
    expect(h.operations.applyReview).toHaveBeenCalledWith(review)
    expect(h.operations.restoreSession).toHaveBeenCalledOnce()
    expect(h.operations.loadStorage).toHaveBeenCalledOnce()
    expect(h.operations.loadDevice).toHaveBeenCalledOnce()
    expect(h.operations.loadOcr).toHaveBeenCalledOnce()
    expect(h.errorMessage.value).toBe(
      '部分设置暂时无法读取：资料库概览。其他设置仍可使用，可点击刷新重试。',
    )
    expect(h.controller.loading.value).toBe(false)
  })

  it('preserves the exact copy from one typed section failure', async () => {
    const h = harness()
    h.operations.loadSubjects.mockResolvedValueOnce(failure(
      'SUBJECTS_UNAVAILABLE',
      '科目配置暂时无法读取，请刷新重试。',
      true,
      'subjects-load',
    ))

    await expect(h.controller.load()).resolves.toBe(false)

    expect(h.errorMessage.value).toBe('科目配置暂时无法读取，请刷新重试。')
    expect(h.operations.applyOverview).toHaveBeenCalledWith(overview)
    expect(h.operations.applyReview).toHaveBeenCalledWith(review)
  })

  it('aggregates multiple failures without claiming the whole page failed', async () => {
    const h = harness()
    h.operations.loadOverview.mockRejectedValueOnce(new Error('overview offline'))
    h.operations.loadSubjects.mockResolvedValueOnce(failure(
      'SUBJECTS_UNAVAILABLE',
      '科目配置暂时无法读取。',
      true,
      'subjects-load',
    ))

    await expect(h.controller.load()).resolves.toBe(false)

    expect(h.errorMessage.value).toBe(
      '部分设置暂时无法读取：资料库概览、科目配置。其他设置仍可使用，可点击刷新重试。',
    )
    expect(h.errorMessage.value).not.toContain('设置状态暂时无法读取，请重新打开应用')
  })

  it('does not apply preference data after the corresponding draft revision changes', async () => {
    const subjectGate = deferred<AppResult<SubjectPreferences>>()
    const h = harness()
    h.operations.loadSubjects.mockReturnValueOnce(subjectGate.promise)

    const loading = h.controller.load()
    await vi.waitFor(() => expect(h.operations.loadSubjects).toHaveBeenCalledOnce())
    h.reviseSubjects()
    subjectGate.resolve(success(subjects))
    await expect(loading).resolves.toBe(true)

    expect(h.operations.applySubjects).not.toHaveBeenCalled()
    expect(h.operations.applyReview).toHaveBeenCalledWith(review)
    expect(h.errorMessage.value).toBe('')
  })

  it('rejects duplicate and guarded refresh attempts', async () => {
    const backendGate = deferred<void>()
    const h = harness()
    h.operations.loadBackend.mockReturnValueOnce(backendGate.promise)

    const first = h.controller.load()
    await vi.waitFor(() => expect(h.operations.loadBackend).toHaveBeenCalledOnce())
    await expect(h.controller.load()).resolves.toBe(false)
    backendGate.resolve()
    await expect(first).resolves.toBe(true)
    expect(h.operations.loadBackend).toHaveBeenCalledOnce()

    h.setBlocked('请先保存偏好；未保存内容不会被刷新覆盖。')
    await expect(h.controller.load()).resolves.toBe(false)
    expect(h.errorMessage.value).toBe('请先保存偏好；未保存内容不会被刷新覆盖。')
    expect(h.operations.loadBackend).toHaveBeenCalledOnce()
  })

  it('loads only browser-safe state outside the desktop runtime', async () => {
    const h = harness()
    h.setDesktop(false)

    await expect(h.controller.load()).resolves.toBe(true)

    expect(h.operations.loadBackend).toHaveBeenCalledOnce()
    expect(h.operations.onBrowserPreview).toHaveBeenCalledOnce()
    expect(h.operations.restoreSession).not.toHaveBeenCalled()
    expect(h.operations.loadOverview).not.toHaveBeenCalled()
    expect(h.operations.loadSubjects).not.toHaveBeenCalled()
    expect(h.operations.loadStorage).not.toHaveBeenCalled()
    expect(h.errorMessage.value).toBe('')
  })
})
