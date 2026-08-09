<script setup lang="ts">
import { isTauri } from '@tauri-apps/api/core'
import { CheckCircle2, RotateCcw } from '@lucide/vue'
import { computed, inject, nextTick, onMounted, ref } from 'vue'
import { routeLocationKey, routerKey } from 'vue-router'
import { commands, type DiagnosticExportReceipt, type LibraryAccessStatus, type ReviewFocusPolicy, type ReviewPreferences, type ReviewPreferencesInput, type SettingsOverview, type StorageLocationStatus, type StorageMigrationReceipt, type SubjectPreferences, type SubjectPreferencesInput, type WindowsCompatibilityStatus, type WindowsUpdateCheckReport, type WindowsUpdateStatus } from '../../shared/api/bindings'
import { normalizeAppResult } from '../../shared/api/normalize-result'
import { backendKindLabel, loadSyncBackendStatus, setSyncBackend } from '../../shared/api/sync-backend'
import LegacyImportPanel from '../../modules/legacy/components/LegacyImportPanel.vue'
import OcrCapabilityPanel from '../../modules/ocr/components/OcrCapabilityPanel.vue'
import { useOcrComponentManagement } from '../../modules/ocr/composables/useOcrComponentManagement'
import SyncConflictCenter from '../../modules/sync/components/SyncConflictCenter.vue'
import ActionConfirmDialog from '../components/ActionConfirmDialog.vue'
import BackupRestoreDialog from '../BackupRestoreDialog.vue'
import SettingsBackupPanel from '../components/SettingsBackupPanel.vue'
import SettingsCloudAuthPanel from '../components/SettingsCloudAuthPanel.vue'
import SettingsDeviceOverviewPanel from '../components/SettingsDeviceOverviewPanel.vue'
import SettingsDiagnosticsPanel from '../components/SettingsDiagnosticsPanel.vue'
import SettingsReviewPanel from '../components/SettingsReviewPanel.vue'
import type { SettingsReviewOption } from '../components/SettingsReviewPanel.vue'
import SettingsSectionNav from '../components/SettingsSectionNav.vue'
import SettingsStoragePanel from '../components/SettingsStoragePanel.vue'
import type { SettingsStorageReceiptCopy } from '../components/SettingsStoragePanel.vue'
import SettingsSubjectPanel from '../components/SettingsSubjectPanel.vue'
import SettingsSyncBackendPanel from '../components/SettingsSyncBackendPanel.vue'
import type { SettingsBackendOption } from '../components/SettingsSyncBackendPanel.vue'
import SettingsUpdatePanel from '../components/SettingsUpdatePanel.vue'
import LibraryLockDialog from '../LibraryLockDialog.vue'
import { libraryAccessControllerKey } from '../library-access-controller'
import { buildSettingsSections } from '../settings-section-catalog'
import { formatSettingsAuthStatus, formatSettingsBytes, formatSettingsTime } from '../settings-formatters'
import { syncControllerKey } from '../sync-controller'
import { workspaceTransitionGuardKey } from '../workspace-transition-guard'
import StorageMigrationDialog from '../StorageMigrationDialog.vue'
import { useQueuedPreferenceSave } from '../composables/useQueuedPreferenceSave'
import { useSettingsBackendSelection } from '../composables/useSettingsBackendSelection'
import { useSettingsBackupOperations } from '../composables/useSettingsBackupOperations'
import { useSettingsCloudSession } from '../composables/useSettingsCloudSession'
import { useSettingsPageLoad } from '../composables/useSettingsPageLoad'
import { useSettingsSyncOperations } from '../composables/useSettingsSyncOperations'
import { useSubjectPreferenceDraft } from '../composables/useSubjectPreferenceDraft'
import { useUnsavedChangesGuard } from '../composables/useUnsavedChangesGuard'

const overview = ref<SettingsOverview>()
const errorMessage = ref('')
const restoreDialogOpen = ref(false)
const backupPanel = ref<{ focusRestoreAction: () => void }>()
const {
  phase: backupPhase,
  busy: backupBusy,
  created: createdBackup,
  candidate: restoreCandidate,
  message: backupMessage,
  createBackup: runCreateBackup,
  prepareRestore: runPrepareRestore,
  restoreBackup: runRestoreBackup,
} = useSettingsBackupOperations({
  create: async () => {
    const invocation = await commands.backupCreate()
    if (invocation.status === 'error') throw new Error('backup command rejected')
    return normalizeAppResult(invocation.data)
  },
  prepareRestore: async () => {
    const invocation = await commands.backupPrepareRestore()
    if (invocation.status === 'error') throw new Error('backup command rejected')
    return normalizeAppResult(invocation.data)
  },
  restore: async (candidateId) => {
    const invocation = await commands.backupRestore(candidateId)
    if (invocation.status === 'error') throw new Error('restore command rejected')
    return normalizeAppResult(invocation.data)
  },
})
const creatingBackup = computed(() => backupPhase.value === 'creating')
const preparingRestore = computed(() => backupPhase.value === 'preparing')
const restoring = computed(() => backupPhase.value === 'restoring')
const deviceAccessStatus = ref<LibraryAccessStatus>()
const deviceAccessError = ref('')
const deviceOverviewPanel = ref<{ focusLockAction: () => void }>()
const cloudAuthPanel = ref<{ focusSignOutAction: () => void }>()
const libraryAccessController = inject(libraryAccessControllerKey, undefined)
const globalSyncController = inject(syncControllerKey, undefined)
const currentRoute = inject(routeLocationKey, undefined)
const appRouter = inject(routerKey, undefined)
const workspaceTransitionGuard = inject(workspaceTransitionGuardKey, undefined)
const authRestoreCommand = (commands as unknown as {
  authRestore?: typeof commands.authRestore
}).authRestore
const authStatusCommand = (commands as unknown as {
  authStatusCommand?: typeof commands.authStatusCommand
}).authStatusCommand
const cloudSession = useSettingsCloudSession({
  operations: {
    ...(typeof authRestoreCommand === 'function'
      ? {
          restore: async () => {
            const invocation = await authRestoreCommand()
            if (invocation.status === 'error') throw new Error('auth restore command rejected')
            return normalizeAppResult(invocation.data)
          },
        }
      : {}),
    ...(typeof authStatusCommand === 'function'
      ? {
          status: async () => normalizeAppResult(await authStatusCommand()),
        }
      : {}),
    signIn: async (credentials) => {
      const invocation = await commands.authSignIn(credentials)
      if (invocation.status === 'error') throw new Error('auth command rejected')
      return normalizeAppResult(invocation.data)
    },
    signUp: async (credentials) => {
      const invocation = await commands.authSignUp(credentials)
      if (invocation.status === 'error') throw new Error('auth command rejected')
      return normalizeAppResult(invocation.data)
    },
    disconnect: async () => {
      const invocation = await commands.authDisconnect()
      if (invocation.status === 'error') throw new Error('auth command rejected')
      return normalizeAppResult(invocation.data)
    },
    lockLibrary: async () => normalizeAppResult(await commands.libraryLock()),
  },
  onConnected: () => globalSyncController?.run('manual'),
  onRestarting: () => libraryAccessController?.enterRestarting(),
})
const {
  auth: cloudAuth,
  email: authEmail,
  password: authPassword,
  mode: authMode,
  authBusy,
  authMessage,
  lockDialogOpen,
  lockDialogMode,
  lockingLibrary,
  lockErrorMessage,
  restoreSession: restoreAuthSession,
  submit: signIn,
  openLibraryLock,
  closeLibraryLock: closeCloudLibraryLock,
  confirmLibraryLock,
} = cloudSession
const inboxReturnBatchId = computed(() => {
  if (currentRoute?.query.returnTo !== 'inbox') return ''
  const batchId = currentRoute.query.batchId
  return typeof batchId === 'string' ? batchId : ''
})
const builtinSubjects = ['语文', '数学', '英语', '政治', '历史', '地理', '物理', '化学', '生物']
const subjectPreferences = ref<SubjectPreferences>()
const reviewPreferences = ref<ReviewPreferences>()
const preferenceRefreshGuardMessage = '请先保存偏好；未保存的科目配置或训练节奏不会被刷新覆盖。'
const preferenceNavigationBusyMessage = '偏好正在保存，请等待完成后再离开设置。'
const backupNavigationBusyMessage = '备份操作正在完成，请等待完成后再离开设置。'
const {
  saving: savingSubjects,
  dirty: subjectPreferencesDirty,
  message: subjectSaveMessage,
  revision: subjectPreferencesRevision,
  markChanged: markSubjectDraftChanged,
  save: runSubjectPreferencesSave,
} = useQueuedPreferenceSave<SubjectPreferencesInput, SubjectPreferences>({
  snapshot: () => {
    const preferences = subjectPreferences.value
    return preferences
      ? {
          enabledSubjects: [...preferences.enabledSubjects],
          customSubjects: [...preferences.customSubjects],
          captureSoundEnabled: preferences.captureSoundEnabled,
        }
      : undefined
  },
  persist: async input => normalizeAppResult(await commands.subjectPreferencesSave(input)),
  applySaved: saved => { subjectPreferences.value = saved },
  validate: input => input.enabledSubjects.length ? undefined : '至少保留一个常用科目。',
  successMessage: '科目配置已保存',
  failureMessage: '科目配置没有保存成功，最新修改会保留在当前页面。',
  queuedMessage: '科目配置有新修改，完成当前保存后会自动继续。',
})
const {
  customSubject,
  message: subjectLocalMessage,
  updateCustomSubject,
  clearMessage: clearSubjectDraftMessage,
  addCustomSubject,
  removeCustomSubject,
  toggleSubject,
  updateCaptureSound,
} = useSubjectPreferenceDraft({
  preferences: subjectPreferences,
  builtinSubjects,
  onChanged: markSubjectDraftChanged,
})
const subjectMessage = computed(() => subjectLocalMessage.value || subjectSaveMessage.value)
const {
  saving: savingReviewPreferences,
  dirty: reviewPreferencesDirty,
  message: reviewPreferenceMessage,
  revision: reviewPreferencesRevision,
  markChanged: markReviewPreferencesChanged,
  save: runReviewPreferencesSave,
} = useQueuedPreferenceSave<ReviewPreferencesInput, ReviewPreferences>({
  snapshot: () => reviewPreferences.value
    ? { focusPolicy: reviewPreferences.value.focusPolicy }
    : undefined,
  persist: async input => normalizeAppResult(await commands.reviewPreferencesSave(input)),
  applySaved: saved => { reviewPreferences.value = saved },
  successMessage: '训练节奏已保存，将从下一轮普通训练开始生效。',
  failureMessage: '训练节奏没有保存成功，最新修改会保留在当前页面。',
  queuedMessage: '训练节奏有新修改，完成当前保存后会自动继续。',
})
const {
  current: preferenceLeaveConfirmation,
  confirm: confirmPreferenceLeave,
  cancel: cancelPreferenceLeave,
} = useUnsavedChangesGuard({
  dirty: () => subjectPreferencesDirty.value || reviewPreferencesDirty.value,
  busy: () => savingSubjects.value || savingReviewPreferences.value || backupBusy.value,
  onBusy: () => {
    errorMessage.value = backupBusy.value
      ? backupNavigationBusyMessage
      : preferenceNavigationBusyMessage
  },
  ...(appRouter
    ? {
        registerNavigation: (attempt: () => boolean | Promise<boolean>) => appRouter.beforeEach((to, from) => {
          if (from.name !== 'settings' || to.name === 'settings') return true
          return attempt()
        }),
    }
    : {}),
  ...(workspaceTransitionGuard
    ? { registerContextTransition: workspaceTransitionGuard.register }
    : {}),
  confirmation: {
    eyebrow: '未保存偏好 · 离开确认',
    title: '放弃设置修改并离开？',
    description: '未保存的科目配置或训练节奏会丢失。你可以继续编辑并先保存，也可以明确放弃这些修改。',
    cancelLabel: '继续编辑',
    confirmLabel: '放弃修改并离开',
    tone: 'danger',
  },
})
const storageStatus = ref<StorageLocationStatus>()
const storageStatusError = ref('')
const storageMigrationReceipt = ref<StorageMigrationReceipt>()
const storageDialogOpen = ref(false)
const storageMigrating = ref(false)
const storageMigrationError = ref('')
const storagePanel = ref<{ focusMigrationAction: () => void }>()
const diagnosticsReceipt = ref<DiagnosticExportReceipt>()
const exportingDiagnostics = ref(false)
const diagnosticsMessage = ref('')
const diagnosticsPanel = ref<{ focusPrimaryAction: () => void }>()
const windowsCompatibility = ref<WindowsCompatibilityStatus>()
const windowsUpdateStatus = ref<WindowsUpdateStatus>()
const windowsUpdateReport = ref<WindowsUpdateCheckReport>()
const checkingWindowsUpdate = ref(false)
const installingWindowsUpdate = ref(false)
const windowsUpdateMessage = ref('')
const windowsUpdatePanel = ref<{ focusPrimaryAction: () => void }>()
const {
  capability: ocrCapability,
  busy: ocrBusy,
  message: ocrMessage,
  refreshCapability: loadOcrCapability,
  install: installOcrComponent,
  remove: removeOcrComponent,
} = useOcrComponentManagement({
  fetchCapability: async () => {
    const invocation = await commands.ocrCapabilityStatus()
    if (invocation.status === 'error') throw new Error('capability command rejected')
    return normalizeAppResult(invocation.data)
  },
  installComponent: async (componentId) => {
    const invocation = await commands.ocrComponentInstall(componentId)
    if (invocation.status === 'error') throw new Error('component command rejected')
    return normalizeAppResult(invocation.data)
  },
  removeComponent: async (componentId) => {
    const invocation = await commands.ocrComponentRemove(componentId)
    if (invocation.status === 'error') throw new Error('component command rejected')
    return normalizeAppResult(invocation.data)
  },
})
const {
  status: backendStatus,
  busy: backendBusy,
  message: backendMessage,
  loadStatus: loadBackendStatus,
  choose: chooseBackend,
} = useSettingsBackendSelection({
  load: loadSyncBackendStatus,
  select: setSyncBackend,
  label: backendKindLabel,
})
const conflictCenter = ref<{ reload: () => Promise<boolean> }>()
const {
  busy: syncBusy,
  message: syncMessage,
  syncNow,
} = useSettingsSyncOperations({
  sync: async () => {
    if (globalSyncController) return globalSyncController.run('manual')
    const invocation = await commands.syncNow()
    if (invocation.status === 'error') throw new Error('sync command rejected')
    return normalizeAppResult(invocation.data)
  },
  refreshOverview: async () => normalizeAppResult(await commands.settingsOverview()),
  applyOverview: value => { overview.value = value },
  refreshConflicts: async () => {
    await nextTick()
    return conflictCenter.value ? conflictCenter.value.reload() : true
  },
})
const backendOptions: SettingsBackendOption[] = [
  { kind: 'local-only', title: '仅本地（推荐）', hint: '训练、采集和图片都保存在这台 Windows 设备，不需要网络。', available: true, badge: undefined },
  { kind: 'supabase', title: 'Supabase', hint: '适合海外或开发环境；中国大陆网络可能不稳定，必须先配置服务地址和匿名密钥。', available: true, badge: undefined },
  { kind: 'tencent', title: '腾讯云', hint: '国内适配器尚未启用；当前版本不会向腾讯云上传任何数据。', available: false, badge: '规划中' },
]
const reviewFocusOptions: SettingsReviewOption[] = [
  { value: 'off', title: '关闭专注插曲', hint: '训练题之间不插入额外环节。' },
  { value: 'session_start', title: '每轮开始前 · 推荐', hint: '进入普通训练时先完成一次 1–25 视线热身，节奏最自然。' },
  { value: 'every_10', title: '每完成 10 题', hint: '保存第 10、20…题后短暂停一下，再继续下一题。' },
]
const settingsSections = computed(() => buildSettingsSections({
  overview: Boolean(overview.value),
  subjects: Boolean(subjectPreferences.value),
  review: Boolean(reviewPreferences.value),
}))

function updateReviewFocusPolicy(focusPolicy: ReviewFocusPolicy) {
  const preferences = reviewPreferences.value
  if (!preferences) return
  reviewPreferences.value = { ...preferences, focusPolicy }
  markReviewPreferencesChanged()
}

async function saveSubjectPreferences() {
  clearSubjectDraftMessage()
  const saved = await runSubjectPreferencesSave()
  if (saved && !subjectPreferencesDirty.value && !reviewPreferencesDirty.value
    && [preferenceRefreshGuardMessage, preferenceNavigationBusyMessage].includes(errorMessage.value)) {
    errorMessage.value = ''
  }
}

async function saveReviewPreferences() {
  const saved = await runReviewPreferencesSave()
  if (saved && !subjectPreferencesDirty.value && !reviewPreferencesDirty.value
    && [preferenceRefreshGuardMessage, preferenceNavigationBusyMessage].includes(errorMessage.value)) {
    errorMessage.value = ''
  }
}

const storageReceiptCopy = computed<SettingsStorageReceiptCopy | undefined>(() => {
  const receipt = storageMigrationReceipt.value
  if (!receipt) return undefined
  const summary = `${receipt.destinationLabel} · ${receipt.copiedAssetCount} 个加密资源 · ${formatSettingsBytes(receipt.copiedBytes)}`
  if (receipt.outcome === 'moved') {
    return {
      kind: 'success',
      title: '资料库已安全迁移',
      detail: `${summary}。新位置已经过解密与完整性校验。`,
    }
  }
  if (receipt.outcome === 'cleanup_required') {
    return {
      kind: 'warning',
      title: '新位置已启用，原副本需手动清理',
      detail: `${summary}。资料库可正常使用，但原位置的旧密文副本未能自动删除。`,
    }
  }
  if (receipt.outcome === 'rolled_back') {
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

async function exportDiagnostics() {
  if (exportingDiagnostics.value || !isTauri()) return
  exportingDiagnostics.value = true
  diagnosticsMessage.value = ''
  diagnosticsReceipt.value = undefined
  try {
    const invocation = await commands.diagnosticsExport()
    if (invocation.status === 'error') throw new Error('diagnostics command rejected')
    const result = normalizeAppResult(invocation.data)
    if (!result.ok) {
      diagnosticsMessage.value = result.error.userMessage
      return
    }
    if (result.data) diagnosticsReceipt.value = result.data
  }
  catch {
    diagnosticsMessage.value = '诊断报告没有生成，现有资料不会受到影响；请检查磁盘空间和保存位置后重试。'
  }
  finally {
    exportingDiagnostics.value = false
    await nextTick()
    diagnosticsPanel.value?.focusPrimaryAction()
  }
}

async function createBackup() {
  errorMessage.value = ''
  await runCreateBackup()
  if (errorMessage.value === backupNavigationBusyMessage) errorMessage.value = ''
}

async function prepareRestore() {
  errorMessage.value = ''
  await runPrepareRestore()
  if (errorMessage.value === backupNavigationBusyMessage) errorMessage.value = ''
}

function openRestoreDialog() {
  if (backupBusy.value || !restoreCandidate.value) return
  restoreDialogOpen.value = true
}

async function confirmRestore() {
  errorMessage.value = ''
  const started = await runRestoreBackup()
  if (errorMessage.value === backupNavigationBusyMessage) errorMessage.value = ''
  if (!started) await closeRestoreDialog()
}

async function closeRestoreDialog() {
  if (backupBusy.value) return
  restoreDialogOpen.value = false
  await nextTick()
  backupPanel.value?.focusRestoreAction()
}

async function loadStorageStatus() {
  storageStatusError.value = ''
  try {
    const invocation = await commands.storageStatus()
    if (invocation.status === 'error') throw new Error('storage status command rejected')
    const result = normalizeAppResult(invocation.data)
    if (result.ok) storageStatus.value = result.data
    else {
      storageStatus.value = undefined
      storageStatusError.value = result.error.userMessage
    }
  }
  catch {
    storageStatus.value = undefined
    storageStatusError.value = '资料库容量暂时无法读取；迁移入口不会使用猜测数据。'
  }
}

async function loadStorageMigrationReceipt() {
  try {
    const result = normalizeAppResult(await commands.storageMigrationReceipt())
    if (result.ok && result.data) storageMigrationReceipt.value = result.data
  }
  catch {
    // The receipt is supplementary and must not hide otherwise usable settings.
  }
}

async function loadDeviceAccessStatus() {
  deviceAccessError.value = ''
  try {
    const result = normalizeAppResult(await commands.libraryAccessStatus())
    if (result.ok) deviceAccessStatus.value = result.data
    else {
      deviceAccessStatus.value = undefined
      deviceAccessError.value = result.error.userMessage
    }
  }
  catch {
    deviceAccessStatus.value = undefined
    deviceAccessError.value = '当前设备的离线解锁状态暂时无法读取；资料库仍保持本机加密。'
  }
}

function openStorageMigration() {
  if (storageMigrating.value) return
  storageMigrationError.value = ''
  storageDialogOpen.value = true
}

async function closeStorageMigration() {
  if (storageMigrating.value) return
  storageDialogOpen.value = false
  await nextTick()
  storagePanel.value?.focusMigrationAction()
}

async function confirmStorageMigration() {
  if (storageMigrating.value) return
  storageMigrating.value = true
  storageMigrationError.value = ''
  try {
    const invocation = await commands.storageMigrateSelect()
    if (invocation.status === 'error') throw new Error('storage migration command rejected')
    const result = normalizeAppResult(invocation.data)
    if (!result.ok) {
      storageMigrationError.value = result.error.userMessage
      storageMigrating.value = false
      return
    }
    if (!result.data) {
      storageMigrating.value = false
      await closeStorageMigration()
      return
    }
    libraryAccessController?.enterRestarting()
    // Success deliberately stays busy until Rust restarts and validates the new copy.
  }
  catch {
    storageMigrationError.value = '迁移没有开始或没有完成，原资料库保持不变，请检查目标磁盘后重试。'
    storageMigrating.value = false
  }
}

const { loading, load } = useSettingsPageLoad({
  errorMessage,
  blockedMessage: () => (
    subjectPreferencesDirty.value
    || reviewPreferencesDirty.value
    || savingSubjects.value
    || savingReviewPreferences.value
  ) ? preferenceRefreshGuardMessage : undefined,
  isDesktop: isTauri,
  onBrowserPreview: () => {
    storageStatusError.value = '容量和迁移会在 Windows 桌面应用中显示；浏览器预览不会读取本机资料。'
  },
  revisions: () => ({
    subjects: subjectPreferencesRevision.value,
    review: reviewPreferencesRevision.value,
  }),
  loadBackend: loadBackendStatus,
  restoreSession: restoreAuthSession,
  loadOverview: async () => normalizeAppResult(await commands.settingsOverview()),
  loadSubjects: async () => normalizeAppResult(await commands.subjectPreferencesGet()),
  loadReview: async () => normalizeAppResult(await commands.reviewPreferencesGet()),
  applyOverview: value => { overview.value = value },
  applySubjects: value => { subjectPreferences.value = value },
  applyReview: value => { reviewPreferences.value = value },
  supplementaryTasks: [
    { label: '存储状态', run: loadStorageStatus },
    { label: '迁移记录', run: loadStorageMigrationReceipt },
    { label: '设备状态', run: loadDeviceAccessStatus },
    { label: 'Windows 兼容性', run: loadWindowsCompatibility },
    { label: '应用更新', run: loadWindowsUpdateStatus },
    { label: '智能功能', run: loadOcrCapability },
  ],
})

async function loadWindowsCompatibility() {
  try {
    const invocation = await commands.compatibilityStatus()
    if (invocation.status === 'error') return
    const result = normalizeAppResult(invocation.data)
    if (result.ok) windowsCompatibility.value = result.data
  }
  catch {
    // The settings page remains usable when this optional probe is unavailable.
  }
}

async function loadWindowsUpdateStatus() {
  try {
    const invocation = await commands.windowsUpdateStatus()
    if (invocation.status === 'error') return
    const result = normalizeAppResult(invocation.data)
    if (result.ok) windowsUpdateStatus.value = result.data
  }
  catch {
    // Updating is optional. A failed status probe must not affect local workflows.
  }
}

async function checkWindowsUpdate() {
  if (
    checkingWindowsUpdate.value
    || installingWindowsUpdate.value
    || !windowsUpdateStatus.value?.enabled
  ) return
  checkingWindowsUpdate.value = true
  windowsUpdateReport.value = undefined
  windowsUpdateMessage.value = ''
  try {
    const invocation = await commands.windowsUpdateCheck()
    if (invocation.status === 'error') throw new Error('update command rejected')
    const result = normalizeAppResult(invocation.data)
    if (!result.ok) {
      windowsUpdateMessage.value = result.error.userMessage
      return
    }
    windowsUpdateReport.value = result.data
    windowsUpdateMessage.value = result.data.available && result.data.version
      ? `发现已签名版本 ${result.data.version}。下载后仍会再次核对版本和签名。`
      : '当前已经是最新版本。'
  }
  catch {
    windowsUpdateMessage.value = '暂时无法检查更新，请确认网络连接后重试；当前版本可继续离线使用。'
  }
  finally {
    checkingWindowsUpdate.value = false
    await nextTick()
    windowsUpdatePanel.value?.focusPrimaryAction()
  }
}

async function installWindowsUpdate() {
  const version = windowsUpdateReport.value?.version
  if (
    installingWindowsUpdate.value
    || checkingWindowsUpdate.value
    || !windowsUpdateReport.value?.available
    || !version
  ) return
  installingWindowsUpdate.value = true
  windowsUpdateMessage.value = '正在下载并验证更新；安装开始时应用会关闭。'
  try {
    const invocation = await commands.windowsUpdateInstall(version)
    if (invocation.status === 'error') throw new Error('update install command rejected')
    const result = normalizeAppResult(invocation.data)
    if (!result.ok) {
      windowsUpdateReport.value = undefined
      windowsUpdateMessage.value = result.error.userMessage
      return
    }
    windowsUpdateMessage.value = '安装程序已启动；请按系统提示完成更新。'
  }
  catch {
    windowsUpdateReport.value = undefined
    windowsUpdateMessage.value = '更新没有安装，当前版本保持不变；请稍后重新检查。'
  }
  finally {
    installingWindowsUpdate.value = false
    await nextTick()
    windowsUpdatePanel.value?.focusPrimaryAction()
  }
}

function formatUpdatePublication(value: string | null) {
  if (!value) return ''
  const timestamp = Date.parse(value)
  return Number.isFinite(timestamp) ? formatSettingsTime(timestamp) : ''
}

async function closeLibraryLock() {
  const mode = closeCloudLibraryLock()
  if (!mode) return
  await nextTick()
  if (mode === 'sign-out') cloudAuthPanel.value?.focusSignOutAction()
  else deviceOverviewPanel.value?.focusLockAction()
}

async function refreshOverviewAfterConflict() {
  try {
    const result = normalizeAppResult(await commands.settingsOverview())
    if (result.ok) overview.value = result.data
    else errorMessage.value = result.error.userMessage
  }
  catch {
    errorMessage.value = '冲突已经处理，但顶部资料库统计暂时没有刷新。'
  }
}

async function returnToCapture() {
  if (!appRouter || !inboxReturnBatchId.value) return
  await appRouter.push({
    name: 'inbox',
    query: {
      batchId: inboxReturnBatchId.value,
      recognition: 'resume',
    },
  })
}

onMounted(async () => {
  await load()
  if (currentRoute?.query.section === 'ocr') {
    await nextTick()
    document.getElementById('settings-ocr')?.scrollIntoView({ block: 'start' })
  }
})
</script>

<template>
  <main class="settings-page">
    <header>
      <div>
        <p>本地资料库</p>
        <h1>设置</h1>
        <span>数据安静地待在该在的地方；这里展示真实状态，尚未接通的云能力会明确标注。</span>
      </div>
      <button
        class="settings-refresh"
        type="button"
        :disabled="loading || ocrBusy || savingSubjects || savingReviewPreferences || authBusy || lockingLibrary || syncBusy || backendBusy"
        @click="load"
      >
        <RotateCcw :size="16" />刷新
      </button>
    </header>
    <SettingsSectionNav :sections="settingsSections" />
    <p
      v-if="errorMessage"
      class="error-banner"
      role="alert"
    >
      {{ errorMessage }}
    </p>

    <SettingsSyncBackendPanel
      :status="backendStatus"
      :options="backendOptions"
      :busy="backendBusy"
      :message="backendMessage"
      @select="chooseBackend"
    />

    <SettingsCloudAuthPanel
      v-if="cloudAuth"
      ref="cloudAuthPanel"
      :auth="cloudAuth"
      :email="authEmail"
      :password="authPassword"
      :mode="authMode"
      :auth-busy="authBusy"
      :auth-message="authMessage"
      :sync-busy="syncBusy"
      :sync-message="syncMessage"
      :status-label="formatSettingsAuthStatus(cloudAuth.status.kind)"
      @update-email="authEmail = $event"
      @update-password="authPassword = $event"
      @update-mode="authMode = $event"
      @submit="signIn"
      @sync="syncNow"
      @sign-out="openLibraryLock('sign-out')"
    />

    <SettingsDeviceOverviewPanel
      v-if="overview"
      ref="deviceOverviewPanel"
      :overview="overview"
      :access-status="deviceAccessStatus"
      :access-error="deviceAccessError"
      :cloud-auth="cloudAuth"
      :windows-compatibility="windowsCompatibility"
      :loading="loading"
      @request-lock="openLibraryLock('lock')"
    />
    <p
      v-else-if="loading"
      class="state-copy"
      role="status"
    >
      正在检查本地资料库状态…
    </p>
    <p
      v-else
      class="state-copy"
    >
      资料库状态没有读取成功，页面不会用默认数字代替真实状态。
    </p>

    <SyncConflictCenter
      v-if="(overview?.unresolvedConflictCount ?? 0) > 0"
      ref="conflictCenter"
      @changed="refreshOverviewAfterConflict"
    />
    <p
      v-else-if="cloudAuth?.status.kind === 'connected'"
      class="sync-conflict-clear"
      role="status"
    >
      <CheckCircle2 :size="16" />
      云端同步正常，没有需要手动处理的冲突。
    </p>

    <SettingsSubjectPanel
      v-if="subjectPreferences"
      :preferences="subjectPreferences"
      :builtin-subjects="builtinSubjects"
      :custom-subject="customSubject"
      :saving="savingSubjects"
      :message="subjectMessage"
      @toggle-subject="toggleSubject"
      @update-custom-subject="updateCustomSubject"
      @add-custom-subject="addCustomSubject"
      @remove-custom-subject="removeCustomSubject"
      @update-capture-sound="updateCaptureSound"
      @save="saveSubjectPreferences"
    />

    <SettingsReviewPanel
      v-if="reviewPreferences"
      :preferences="reviewPreferences"
      :options="reviewFocusOptions"
      :saving="savingReviewPreferences"
      :message="reviewPreferenceMessage"
      @update-focus-policy="updateReviewFocusPolicy"
      @save="saveReviewPreferences"
    />

    <div
      v-if="inboxReturnBatchId"
      class="settings-return"
    >
      <p>模型准备好后会回到原批次，但不会自动开始识别。</p>
      <button
        type="button"
        @click="returnToCapture"
      >
        返回采集整理
      </button>
    </div>
    <OcrCapabilityPanel
      id="settings-ocr"
      :status="ocrCapability"
      :busy="ocrBusy"
      :message="ocrMessage"
      @install="installOcrComponent"
      @remove="removeOcrComponent"
    />

    <SettingsStoragePanel
      ref="storagePanel"
      :status="storageStatus"
      :status-message="storageStatusError"
      :receipt="storageReceiptCopy"
      :migrating="storageMigrating"
      @migrate="openStorageMigration"
    />

    <SettingsBackupPanel
      ref="backupPanel"
      :created="createdBackup"
      :candidate="restoreCandidate"
      :creating="creatingBackup"
      :preparing="preparingRestore"
      :restoring="restoring"
      :message="backupMessage"
      @create="createBackup"
      @prepare="prepareRestore"
      @open-restore="openRestoreDialog"
    />

    <SettingsUpdatePanel
      ref="windowsUpdatePanel"
      :status="windowsUpdateStatus"
      :report="windowsUpdateReport"
      :checking="checkingWindowsUpdate"
      :installing="installingWindowsUpdate"
      :message="windowsUpdateMessage"
      :publication-label="formatUpdatePublication(windowsUpdateReport?.publishedAt ?? null)"
      @check="checkWindowsUpdate"
      @install="installWindowsUpdate"
    />

    <SettingsDiagnosticsPanel
      ref="diagnosticsPanel"
      :receipt="diagnosticsReceipt"
      :exporting="exportingDiagnostics"
      :message="diagnosticsMessage"
      :native-available="isTauri()"
      :generated-at-label="formatSettingsTime(diagnosticsReceipt?.generatedAtUtcMs ?? null)"
      @export="exportDiagnostics"
    />

    <div
      id="settings-migration"
      class="settings-migration-anchor"
    >
      <LegacyImportPanel @changed="load" />
    </div>

    <section class="roadmap-panel">
      <div><p>安全底座</p><h2>当前设备已经可控，跨设备撤销需要设备级密钥</h2></div>
      <ol>
        <li><strong>已接通 · 恢复与回滚</strong><span>备份经两次校验后自动重启切换；新资料库打不开时恢复原资料。</span></li>
        <li><strong>已接通 · 当前设备保护</strong><span>可立即锁定本机，退出云端只注销这台电脑；其他设备保持登录。</span></li>
        <li><strong>后续 · 其他设备撤销</strong><span>只有完成每台设备独立密钥封装后，才会提供远程撤销；当前版本不会用假按钮冒充安全能力。</span></li>
      </ol>
    </section>

    <BackupRestoreDialog
      v-if="restoreDialogOpen && restoreCandidate"
      :candidate="restoreCandidate"
      :busy="restoring"
      @cancel="closeRestoreDialog"
      @confirm="confirmRestore"
    />
    <LibraryLockDialog
      v-if="lockDialogOpen"
      :mode="lockDialogMode"
      :busy="lockingLibrary"
      :error-message="lockErrorMessage"
      @cancel="closeLibraryLock"
      @confirm="confirmLibraryLock"
    />
    <StorageMigrationDialog
      v-if="storageDialogOpen"
      :busy="storageMigrating"
      :error-message="storageMigrationError"
      @cancel="closeStorageMigration"
      @confirm="confirmStorageMigration"
    />
    <ActionConfirmDialog
      v-if="preferenceLeaveConfirmation"
      :request="preferenceLeaveConfirmation"
      @cancel="cancelPreferenceLeave"
      @confirm="confirmPreferenceLeave"
    />
  </main>
</template>

<style scoped>
.settings-page { min-height: 100vh; padding: 42px clamp(24px,5vw,72px) 72px; background: radial-gradient(circle at 85% 0,rgba(33,51,45,.08),transparent 32%); }
.settings-return { display:flex; justify-content:space-between; gap:16px; align-items:center; margin-top:24px; padding:14px 18px; border:1px solid rgba(79,128,110,.25); border-radius:14px; background:rgba(229,239,233,.66); }.settings-return p { margin:0; color:var(--ink-muted); font-size:12px; }.settings-return button { flex:0 0 auto; }
#settings-sync,#settings-overview,#settings-subjects,#settings-review,#settings-ocr,#settings-storage,#settings-backup,#settings-updates,#settings-diagnostics,.settings-migration-anchor { scroll-margin-top: 118px; }
header { display: flex; justify-content: space-between; gap: 24px; align-items: flex-start; margin-bottom: 28px; } header p, .roadmap-panel p, .migration-panel header p { margin: 0 0 8px; color: var(--cinnabar); font-size: 12px; font-weight: 800; letter-spacing: .14em; text-transform: uppercase; }
h1, h2 { margin: 0; font-family: Georgia,'Microsoft YaHei',serif; color: var(--green-deep); } h1 { font-size: clamp(28px,4vw,42px); } h2 { font-size: 21px; } header span { display: block; margin-top: 9px; color: var(--ink-muted); }
button { display: inline-flex; min-height: 44px; gap: 7px; align-items: center; padding: 10px 14px; border: 1px solid var(--line); border-radius: 10px; background: var(--paper-raised); cursor: pointer; } button:disabled { opacity: .5; }
.settings-refresh { flex: 0 0 auto; white-space: nowrap; }
.error-banner { padding: 12px; border-radius: 10px; background: rgba(185,88,63,.08); color: #843d2c; }
.state-copy { padding: 28px; border: 1px solid var(--line); border-radius: 14px; background: rgba(255,253,247,.7); color: var(--ink-muted); text-align: center; }
.roadmap-panel, .migration-panel { border: 1px solid var(--line); border-radius: 17px; background: rgba(255,253,247,.78); box-shadow: 0 16px 48px rgba(34,48,43,.05); }
.roadmap-panel { margin-top: 16px; padding: 26px; }.roadmap-panel ol { display: grid; grid-template-columns: repeat(3,1fr); gap: 12px; margin: 22px 0 0; padding: 0; list-style: none; }.roadmap-panel li { padding: 17px; border-radius: 12px; background: rgba(232,221,199,.34); }.roadmap-panel strong, .roadmap-panel span { display: block; }.roadmap-panel span { margin-top: 7px; color: var(--ink-muted); font-size: 12px; line-height: 1.65; }
.migration-panel { margin-top: 16px; padding: 26px; }.migration-panel header { margin-bottom: 20px; }.migration-panel header span { max-width: 680px; }.migration-panel header button { flex: 0 0 auto; }.migration-stats { display: grid; grid-template-columns: repeat(6,minmax(0,1fr)); gap: 10px; margin: 0; }.migration-stats div { padding: 15px; border-radius: 12px; background: rgba(232,221,199,.34); }.migration-stats dt { color: var(--ink-muted); font-size: 12px; }.migration-stats dd { margin: 6px 0 0; color: var(--green-deep); font-family: Georgia,serif; font-size: 25px; font-weight: 700; }.preflight-note { margin: 16px 0 0; color: var(--ink-muted); font-size: 12px; }.preflight-note.ready { color: #557263; }.issue-list { display: grid; gap: 8px; max-height: 330px; margin: 16px 0 0; padding: 0; overflow: auto; list-style: none; }.issue-list li { display: grid; grid-template-columns: 108px minmax(120px,.6fr) 1fr; gap: 12px; align-items: start; padding: 12px 14px; border-radius: 10px; background: rgba(185,88,63,.06); }.issue-list strong { color: #843d2c; font-size: 12px; }.issue-list span, .issue-list small { color: var(--ink-muted); font-size: 12px; overflow-wrap: anywhere; }
.preflight-note.warning { color: #843d2c; font-weight: 700; }
@media (max-width: 980px) { .migration-stats { grid-template-columns: repeat(3,minmax(0,1fr)); } }
@media (max-width: 760px) { .settings-page small,.settings-page label,.settings-page dt { font-size: 12px; }.settings-page button,.settings-page input,.settings-page select { min-height: 44px; } }
@media (max-width: 760px) { .settings-page { padding: 24px 16px 92px; } .roadmap-panel ol, .migration-stats { grid-template-columns: 1fr; } .migration-panel header { flex-direction: column; }.issue-list li { grid-template-columns: 1fr; gap: 4px; } }
.sync-conflict-clear { display: flex; gap: 8px; align-items: center; margin: 16px 0 0; padding: 11px 14px; color: #557263; border: 1px solid rgba(85,114,99,.16); border-radius: 11px; background: rgba(85,114,99,.07); font-size: 12px; }
</style>
