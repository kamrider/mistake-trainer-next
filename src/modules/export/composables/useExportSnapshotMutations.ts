import { computed, readonly, ref } from 'vue'
import type { AppResult } from '../../../shared/api/app-result'
import type {
  DeletedExportSnapshotSummary,
  ExportCreateInput,
  ExportSnapshotSummary,
  GeneratedExportSummary,
} from '../../../shared/api/bindings'

interface ExportSnapshotMutationOptions {
  snapshots: () => ExportSnapshotSummary[]
  deletedSnapshots: () => DeletedExportSnapshotSummary[]
  onSnapshotsChange: (snapshots: ExportSnapshotSummary[]) => void
  onDeletedSnapshotsChange: (snapshots: DeletedExportSnapshotSummary[]) => void
  onTrashLoadedChange: (loaded: boolean) => void
  onError: (message: string) => void
  onStatus: (message: string) => void
  scheduleSync: () => void
  blocked: () => boolean
  confirmDelete: (snapshot: ExportSnapshotSummary) => Promise<boolean>
  createOperation: (input: ExportCreateInput) => Promise<AppResult<ExportSnapshotSummary>>
  generateOperation: (snapshotId: string) => Promise<AppResult<GeneratedExportSummary | null>>
  deleteOperation: (snapshotId: string) => Promise<AppResult<boolean>>
  restoreOperation: (snapshotId: string) => Promise<AppResult<boolean>>
  listTrash: () => Promise<AppResult<DeletedExportSnapshotSummary[]>>
}

function runNotification(effect: () => void) {
  try {
    effect()
  }
  catch {
    // The durable export result is authoritative even if a UI or sync notification fails.
  }
}

export function useExportSnapshotMutations(options: ExportSnapshotMutationOptions) {
  const saving = ref(false)
  const generatingId = ref('')
  const deletingId = ref('')
  const restoringId = ref('')
  const operationBusy = computed(() => Boolean(
    saving.value || generatingId.value || deletingId.value || restoringId.value,
  ))

  function unavailable() {
    return options.blocked() || operationBusy.value
  }

  async function createSnapshot(input: ExportCreateInput): Promise<boolean> {
    const title = input.title.trim()
    if (unavailable() || !title || input.problemIds.length === 0) return false

    const request: ExportCreateInput = {
      title,
      problemIds: [...input.problemIds],
      layout: input.layout,
    }
    saving.value = true
    options.onError('')
    runNotification(() => options.onStatus(''))

    let result: AppResult<ExportSnapshotSummary>
    try {
      result = await options.createOperation(request)
    }
    catch {
      options.onError('导出快照没有保存，请稍后重试。')
      saving.value = false
      return false
    }

    try {
      if (!result.ok) {
        options.onError(result.error.userMessage)
        return false
      }

      options.onSnapshotsChange([
        result.data,
        ...options.snapshots().filter(snapshot => snapshot.id !== result.data.id),
      ])
      runNotification(() => options.onStatus(
        `已保存“${result.data.title}”，随时可以从下方重新生成。`,
      ))
      runNotification(options.scheduleSync)
      return true
    }
    finally {
      saving.value = false
    }
  }

  async function generateSnapshot(snapshot: ExportSnapshotSummary): Promise<boolean> {
    if (unavailable() || !options.snapshots().some(item => item.id === snapshot.id)) return false

    generatingId.value = snapshot.id
    options.onError('')
    runNotification(() => options.onStatus(''))

    let result: AppResult<GeneratedExportSummary | null>
    try {
      result = await options.generateOperation(snapshot.id)
    }
    catch {
      options.onError('文件没有生成，请检查目标目录空间与权限后重试。')
      generatingId.value = ''
      return false
    }

    try {
      if (!result.ok) {
        options.onError(result.error.userMessage)
        return false
      }
      const generated = result.data
      if (!generated) return false

      runNotification(() => options.onStatus(
        `已生成 ${generated.outputName}，共 ${generated.problemCount} 题。`,
      ))
      return true
    }
    finally {
      generatingId.value = ''
    }
  }

  async function deleteSnapshot(snapshotId: string): Promise<boolean> {
    if (unavailable()) return false
    const snapshot = options.snapshots().find(value => value.id === snapshotId)
    if (!snapshot) return false

    deletingId.value = snapshotId
    let confirmed: boolean
    try {
      confirmed = await options.confirmDelete(snapshot)
    }
    catch {
      options.onError('导出快照没有删除，请稍后重试。')
      deletingId.value = ''
      return false
    }

    if (!confirmed || !options.snapshots().some(value => value.id === snapshotId)) {
      deletingId.value = ''
      return false
    }

    options.onError('')
    runNotification(() => options.onStatus(''))
    let result: AppResult<boolean>
    try {
      result = await options.deleteOperation(snapshotId)
    }
    catch {
      options.onError('导出快照没有删除，请稍后重试。')
      deletingId.value = ''
      return false
    }

    try {
      if (!result.ok) {
        options.onError(result.error.userMessage)
        return false
      }

      options.onSnapshotsChange(options.snapshots().filter(value => value.id !== snapshotId))
      runNotification(options.scheduleSync)
      runNotification(() => options.onStatus(`“${snapshot.title}”已移入回收区。`))
      try {
        const trashResult = await options.listTrash()
        if (!trashResult.ok) {
          options.onError('快照已删除，但回收区暂时没有刷新成功。')
          return true
        }
        options.onDeletedSnapshotsChange(trashResult.data)
        options.onTrashLoadedChange(true)
      }
      catch {
        options.onError('快照已删除，但回收区暂时没有刷新成功。')
      }
      return true
    }
    finally {
      deletingId.value = ''
    }
  }

  async function restoreSnapshot(deleted: DeletedExportSnapshotSummary): Promise<boolean> {
    if (unavailable()) return false
    const snapshotId = deleted.snapshot.id
    if (!options.deletedSnapshots().some(item => item.snapshot.id === snapshotId)) return false

    restoringId.value = snapshotId
    options.onError('')
    runNotification(() => options.onStatus(''))
    let result: AppResult<boolean>
    try {
      result = await options.restoreOperation(snapshotId)
    }
    catch {
      options.onError('导出快照没有恢复，请稍后重试。')
      restoringId.value = ''
      return false
    }

    try {
      if (!result.ok) {
        options.onError(result.error.userMessage)
        return false
      }

      options.onSnapshotsChange([
        deleted.snapshot,
        ...options.snapshots().filter(snapshot => snapshot.id !== snapshotId),
      ])
      options.onDeletedSnapshotsChange(
        options.deletedSnapshots().filter(item => item.snapshot.id !== snapshotId),
      )
      runNotification(options.scheduleSync)
      runNotification(() => options.onStatus(`已恢复“${deleted.snapshot.title}”。`))
      return true
    }
    finally {
      restoringId.value = ''
    }
  }

  return {
    saving: readonly(saving),
    generatingId: readonly(generatingId),
    deletingId: readonly(deletingId),
    restoringId: readonly(restoringId),
    operationBusy,
    createSnapshot,
    generateSnapshot,
    deleteSnapshot,
    restoreSnapshot,
  }
}
