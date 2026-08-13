import { computed, readonly, ref } from 'vue'
import type { AppResult } from '../../shared/api/app-result'
import type {
  StorageLocationStatus,
  StorageMigrationReceipt,
} from '../../shared/api/bindings'
import type { SettingsStorageReceiptCopy } from '../../shared/contracts/settings-storage'
import { formatSettingsBytes } from '../settings-formatters'

export interface SettingsStorageOperations {
  status: () => Promise<AppResult<StorageLocationStatus>>
  receipt: () => Promise<AppResult<StorageMigrationReceipt | null>>
  migrate: () => Promise<AppResult<StorageMigrationReceipt | null>>
}

export interface SettingsStorageLifecycleOptions {
  operations: SettingsStorageOperations
  enterRestarting: () => void
  restoreMigrationFocus: () => Promise<unknown> | unknown
}

export function useSettingsStorageLifecycle(options: SettingsStorageLifecycleOptions) {
  const status = ref<StorageLocationStatus>()
  const statusMessage = ref('')
  const receipt = ref<StorageMigrationReceipt>()
  const dialogOpen = ref(false)
  const busy = ref(false)
  const migrationMessage = ref('')

  const receiptCopy = computed<SettingsStorageReceiptCopy | undefined>(() => {
    const value = receipt.value
    if (!value) return undefined
    const summary = `${value.destinationLabel} · ${value.copiedAssetCount} 个加密资源 · ${formatSettingsBytes(value.copiedBytes)}`
    if (value.outcome === 'moved') {
      return {
        kind: 'success',
        title: '资料库已安全迁移',
        detail: `${summary}。新位置已经过解密与完整性校验。`,
      }
    }
    if (value.outcome === 'cleanup_required') {
      return {
        kind: 'warning',
        title: '新位置已启用，原副本需手动清理',
        detail: `${summary}。资料库可正常使用，但原位置的旧密文副本未能自动删除。`,
      }
    }
    if (value.outcome === 'rolled_back') {
      return {
        kind: 'warning',
        title: '迁移未生效，已自动回到原位置',
        detail: '目标副本没有通过最终启动校验；原资料库保持完整，请更换位置后重试。',
      }
    }
    return {
      kind: 'warning',
      title: '迁移等待安全重启',
      detail: `${summary}。应用重启后会做最后一次解密校验，再决定提交或回滚。`,
    }
  })

  async function loadStatus(): Promise<boolean> {
    statusMessage.value = ''
    try {
      const result = await options.operations.status()
      if (!result.ok) {
        status.value = undefined
        statusMessage.value = result.error.userMessage
        return false
      }
      status.value = result.data
      return true
    }
    catch {
      status.value = undefined
      statusMessage.value = '资料库容量暂时无法读取；迁移入口不会使用猜测数据。'
      return false
    }
  }

  async function loadReceipt(): Promise<boolean> {
    try {
      const result = await options.operations.receipt()
      if (!result.ok || !result.data) return false
      receipt.value = result.data
      return true
    }
    catch {
      return false
    }
  }

  function showBrowserPreview() {
    status.value = undefined
    statusMessage.value = '容量和迁移会在 Windows 桌面应用中显示；浏览器预览不会读取本机资料。'
  }

  function openMigration() {
    if (busy.value) return
    migrationMessage.value = ''
    dialogOpen.value = true
  }

  async function closeMigration(): Promise<boolean> {
    if (busy.value) return false
    dialogOpen.value = false
    await options.restoreMigrationFocus()
    return true
  }

  async function confirmMigration(): Promise<boolean> {
    if (busy.value) return false
    busy.value = true
    migrationMessage.value = ''
    try {
      const result = await options.operations.migrate()
      if (!result.ok) {
        migrationMessage.value = result.error.userMessage
        busy.value = false
        return false
      }
      if (!result.data) {
        busy.value = false
        await closeMigration()
        return false
      }
      options.enterRestarting()
      // Success deliberately stays busy until the root access boundary unmounts this page.
      return true
    }
    catch {
      migrationMessage.value = '迁移没有开始或没有完成，原资料库保持不变，请检查目标磁盘后重试。'
      busy.value = false
      return false
    }
  }

  return {
    status: readonly(status),
    statusMessage: readonly(statusMessage),
    receipt: readonly(receipt),
    receiptCopy,
    dialogOpen: readonly(dialogOpen),
    busy: readonly(busy),
    migrationMessage: readonly(migrationMessage),
    loadStatus,
    loadReceipt,
    showBrowserPreview,
    openMigration,
    closeMigration,
    confirmMigration,
  }
}
