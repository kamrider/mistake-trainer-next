<script setup lang="ts">
import { isTauri } from '@tauri-apps/api/core'
import { Archive, ArchiveRestore, BookOpen, CheckCircle2, CloudOff, Database, FileJson2, FolderCheck, Laptop, LockKeyhole, Plus, RotateCcw, ShieldCheck, Trash2, TriangleAlert, Volume2 } from '@lucide/vue'
import { computed, inject, nextTick, onMounted, ref } from 'vue'
import { routeLocationKey, routerKey } from 'vue-router'
import type { AppResult } from '../../shared/api/app-result'
import { commands, type AuthStatusKind, type BackupRestoreCandidate, type BackupSummary, type CloudAuthState, type CloudBackendKind, type CloudBackendStatus, type DiagnosticExportReceipt, type LibraryAccessStatus, type ReviewFocusPolicy, type ReviewPreferences, type SettingsOverview, type StorageLocationStatus, type StorageMigrationReceipt, type SubjectPreferences, type WindowsCompatibilityStatus } from '../../shared/api/bindings'
import { normalizeAppResult } from '../../shared/api/normalize-result'
import { backendKindLabel, backendStatusLabel, loadSyncBackendStatus, setSyncBackend } from '../../shared/api/sync-backend'
import LegacyImportPanel from '../../modules/legacy/components/LegacyImportPanel.vue'
import OcrCapabilityPanel from '../../modules/ocr/components/OcrCapabilityPanel.vue'
import SyncConflictCenter from '../../modules/sync/components/SyncConflictCenter.vue'
import BackupRestoreDialog from '../BackupRestoreDialog.vue'
import SettingsSectionNav, { type SettingsSectionLink } from '../components/SettingsSectionNav.vue'
import LibraryLockDialog from '../LibraryLockDialog.vue'
import { libraryAccessControllerKey } from '../library-access-controller'
import { syncControllerKey } from '../sync-controller'
import StorageMigrationDialog from '../StorageMigrationDialog.vue'

const overview = ref<SettingsOverview>()
const loading = ref(true)
const errorMessage = ref('')
const createdBackup = ref<BackupSummary>()
const restoreCandidate = ref<BackupRestoreCandidate>()
const creatingBackup = ref(false)
const preparingRestore = ref(false)
const restoreDialogOpen = ref(false)
const restoring = ref(false)
const restoreTrigger = ref<HTMLButtonElement>()
const lockDialogOpen = ref(false)
const lockDialogMode = ref<'lock' | 'sign-out'>('lock')
const lockingLibrary = ref(false)
const lockErrorMessage = ref('')
const lockReturnFocus = ref<HTMLButtonElement>()
const deviceAccessStatus = ref<LibraryAccessStatus>()
const deviceAccessError = ref('')
const libraryAccessController = inject(libraryAccessControllerKey, undefined)
const globalSyncController = inject(syncControllerKey, undefined)
const currentRoute = inject(routeLocationKey, undefined)
const appRouter = inject(routerKey, undefined)
const inboxReturnBatchId = computed(() => {
  if (currentRoute?.query.returnTo !== 'inbox') return ''
  const batchId = currentRoute.query.batchId
  return typeof batchId === 'string' ? batchId : ''
})
const builtinSubjects = ['语文', '数学', '英语', '政治', '历史', '地理', '物理', '化学', '生物']
const subjectPreferences = ref<SubjectPreferences>()
const customSubject = ref('')
const savingSubjects = ref(false)
const subjectMessage = ref('')
const reviewPreferences = ref<ReviewPreferences>()
const savingReviewPreferences = ref(false)
const reviewPreferenceMessage = ref('')
const storageStatus = ref<StorageLocationStatus>()
const storageStatusError = ref('')
const storageMigrationReceipt = ref<StorageMigrationReceipt>()
const storageDialogOpen = ref(false)
const storageMigrating = ref(false)
const storageMigrationError = ref('')
const storageTrigger = ref<HTMLButtonElement>()
const diagnosticsReceipt = ref<DiagnosticExportReceipt>()
const exportingDiagnostics = ref(false)
const diagnosticsMessage = ref('')
const diagnosticsTrigger = ref<HTMLButtonElement>()
const windowsCompatibility = ref<WindowsCompatibilityStatus>()
const backendStatus = ref<AppResult<CloudBackendStatus>>()
const backendBusy = ref(false)
const backendMessage = ref('')
const cloudAuth = ref<CloudAuthState>()
const authEmail = ref('')
const authPassword = ref('')
const authMode = ref<'signIn' | 'signUp'>('signIn')
const authBusy = ref(false)
const authMessage = ref('')
const syncBusy = ref(false)
const syncMessage = ref('')
const conflictCenter = ref<{ reload: () => Promise<void> }>()
const backendOptions: Array<{ kind: CloudBackendKind; title: string; hint: string; available: boolean; badge?: string }> = [
  { kind: 'local-only', title: '仅本地（推荐）', hint: '训练、采集和图片都保存在这台 Windows 设备，不需要网络。', available: true },
  { kind: 'supabase', title: 'Supabase', hint: '适合海外或开发环境；中国大陆网络可能不稳定，必须先配置服务地址和匿名密钥。', available: true },
  { kind: 'tencent', title: '腾讯云', hint: '国内适配器尚未启用；当前版本不会向腾讯云上传任何数据。', available: false, badge: '规划中' },
]
const reviewFocusOptions: Array<{ value: ReviewFocusPolicy, title: string, hint: string }> = [
  { value: 'off', title: '关闭专注插曲', hint: '训练题之间不插入额外环节。' },
  { value: 'session_start', title: '每轮开始前 · 推荐', hint: '进入普通训练时先完成一次 1–25 视线热身，节奏最自然。' },
  { value: 'every_10', title: '每完成 10 题', hint: '保存第 10、20…题后短暂停一下，再继续下一题。' },
]
const settingsSections = computed<SettingsSectionLink[]>(() => [
  { id: 'settings-sync', label: '同步账户', hint: '本地与云端' },
  ...(overview.value
    ? [{ id: 'settings-overview', label: '本机概况', hint: '题库与冲突' }]
    : []),
  ...(subjectPreferences.value
    ? [{ id: 'settings-subjects', label: '科目配置', hint: '采集常用项' }]
    : []),
  ...(reviewPreferences.value
    ? [{ id: 'settings-review', label: '训练节奏', hint: '专注插曲' }]
    : []),
  { id: 'settings-ocr', label: '智能功能', hint: '当前切图与后续识题' },
  { id: 'settings-storage', label: '存储位置', hint: '容量与迁移' },
  { id: 'settings-backup', label: '备份恢复', hint: '完整快照' },
  { id: 'settings-diagnostics', label: '安全诊断', hint: '隐私报告' },
  { id: 'settings-migration', label: '旧版迁移', hint: '安全导入' },
])
const deviceStatusKey = computed(() => [
  deviceAccessStatus.value?.trustedWindowsAccount ? 'trusted' : 'unavailable',
  cloudAuth.value?.status.kind ?? 'checking',
  deviceAccessError.value ? 'error' : 'ready',
].join(':'))

function addCustomSubject() {
  const value = customSubject.value.trim()
  const preferences = subjectPreferences.value
  if (!preferences || !value || value.length > 40 || preferences.customSubjects.includes(value)) return
  if (preferences.customSubjects.length >= 20) {
    subjectMessage.value = '自定义科目最多 20 个。'
    return
  }
  preferences.customSubjects.push(value)
  if (!preferences.enabledSubjects.includes(value)) preferences.enabledSubjects.push(value)
  customSubject.value = ''
  subjectMessage.value = ''
}

function removeCustomSubject(subject: string) {
  const preferences = subjectPreferences.value
  if (!preferences) return
  preferences.customSubjects = preferences.customSubjects.filter(value => value !== subject)
  preferences.enabledSubjects = preferences.enabledSubjects.filter(value => value !== subject)
}

async function saveSubjectPreferences() {
  const preferences = subjectPreferences.value
  if (!preferences || savingSubjects.value) return
  if (!preferences.enabledSubjects.length) {
    subjectMessage.value = '至少保留一个常用科目。'
    return
  }
  savingSubjects.value = true
  subjectMessage.value = ''
  try {
    const result = normalizeAppResult(await commands.subjectPreferencesSave({
      enabledSubjects: preferences.enabledSubjects,
      customSubjects: preferences.customSubjects,
      captureSoundEnabled: preferences.captureSoundEnabled,
    }))
    if (result.ok) {
      subjectPreferences.value = result.data
      subjectMessage.value = '科目配置已保存'
    }
    else subjectMessage.value = result.error.userMessage
  }
  catch {
    subjectMessage.value = '科目配置没有保存成功，原有配置保持不变。'
  }
  finally {
    savingSubjects.value = false
  }
}

async function saveReviewPreferences() {
  const preferences = reviewPreferences.value
  if (!preferences || savingReviewPreferences.value) return
  savingReviewPreferences.value = true
  reviewPreferenceMessage.value = ''
  try {
    const result = normalizeAppResult(await commands.reviewPreferencesSave({
      focusPolicy: preferences.focusPolicy,
    }))
    if (result.ok) {
      reviewPreferences.value = result.data
      reviewPreferenceMessage.value = '训练节奏已保存，将从下一轮普通训练开始生效。'
    }
    else reviewPreferenceMessage.value = result.error.userMessage
  }
  catch {
    reviewPreferenceMessage.value = '训练节奏没有保存成功，原有配置保持不变。'
  }
  finally {
    savingReviewPreferences.value = false
  }
}

function formatBytes(bytes: number | null) {
  if (bytes === null || !Number.isFinite(bytes) || bytes < 0) return '未知大小'
  if (bytes < 1024) return `${bytes} B`
  if (bytes < 1024 * 1024) return `${(bytes / 1024).toFixed(1)} KB`
  if (bytes < 1024 * 1024 * 1024) return `${(bytes / 1024 / 1024).toFixed(1)} MB`
  return `${(bytes / 1024 / 1024 / 1024).toFixed(2)} GB`
}

const storageTotalBytes = computed(() => {
  const status = storageStatus.value
  if (!status || status.databaseBytes === null || status.assetBytes === null) return null
  const total = status.databaseBytes + status.assetBytes
  return Number.isFinite(total) && total >= 0 ? total : null
})

const storageReceiptCopy = computed(() => {
  const receipt = storageMigrationReceipt.value
  if (!receipt) return undefined
  const summary = `${receipt.destinationLabel} · ${receipt.copiedAssetCount} 个加密资源 · ${formatBytes(receipt.copiedBytes)}`
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

function formatBackupTime(timestamp: number | null) {
  if (
    timestamp === null
    || !Number.isFinite(timestamp)
    || timestamp < 0
    || timestamp > 8_640_000_000_000_000
  ) return '时间未知'
  return new Intl.DateTimeFormat('zh-CN', {
    dateStyle: 'medium',
    timeStyle: 'short',
  }).format(new Date(timestamp))
}

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
    diagnosticsTrigger.value?.focus()
  }
}

async function createBackup() {
  creatingBackup.value = true
  errorMessage.value = ''
  try {
    const invocation = await commands.backupCreate()
    if (invocation.status === 'error') throw new Error('backup command rejected')
    const result = normalizeAppResult(invocation.data)
    if (result.ok) {
      if (result.data) createdBackup.value = result.data
    }
    else {
      createdBackup.value = undefined
      errorMessage.value = result.error.userMessage
    }
  }
  catch {
    createdBackup.value = undefined
    errorMessage.value = '加密备份没有完成，现有资料库未被替换，请检查磁盘空间后重试。'
  }
  finally {
    creatingBackup.value = false
  }
}

async function prepareRestore() {
  preparingRestore.value = true
  errorMessage.value = ''
  try {
    const invocation = await commands.backupPrepareRestore()
    if (invocation.status === 'error') throw new Error('backup command rejected')
    const result = normalizeAppResult(invocation.data)
    if (result.ok) {
      if (result.data) restoreCandidate.value = result.data
    }
    else {
      restoreCandidate.value = undefined
      errorMessage.value = result.error.userMessage
    }
  }
  catch {
    restoreCandidate.value = undefined
    errorMessage.value = '备份包没有验证成功；现有资料库未被修改，请稍后重试。'
  }
  finally {
    preparingRestore.value = false
  }
}

async function confirmRestore() {
  const candidate = restoreCandidate.value
  if (!candidate || restoring.value) return
  restoring.value = true
  errorMessage.value = ''
  try {
    const invocation = await commands.backupRestore(candidate.id)
    if (invocation.status === 'error') throw new Error('restore command rejected')
    const result = normalizeAppResult(invocation.data)
    if (!result.ok) {
      errorMessage.value = result.error.userMessage
      restoring.value = false
      await closeRestoreDialog()
    }
  }
  catch {
    errorMessage.value = '恢复任务没有开始；当前资料库保持不变，请稍后重试。'
    restoring.value = false
    await closeRestoreDialog()
  }
}

async function closeRestoreDialog() {
  if (restoring.value) return
  restoreDialogOpen.value = false
  await nextTick()
  restoreTrigger.value?.focus()
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

function openStorageMigration(event: Event) {
  if (storageMigrating.value) return
  storageMigrationError.value = ''
  storageTrigger.value = event.currentTarget as HTMLButtonElement
  storageDialogOpen.value = true
}

async function closeStorageMigration() {
  if (storageMigrating.value) return
  storageDialogOpen.value = false
  await nextTick()
  storageTrigger.value?.focus()
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

async function load() {
  loading.value = true
  errorMessage.value = ''
  try {
    backendStatus.value = await loadSyncBackendStatus()
    if (!isTauri()) {
      storageStatusError.value = '容量和迁移会在 Windows 桌面应用中显示；浏览器预览不会读取本机资料。'
      return
    }
    const authStatusCommand = (commands as unknown as { authStatusCommand?: () => Promise<unknown> }).authStatusCommand
    const authRestoreCommand = (commands as unknown as { authRestore?: () => Promise<unknown> }).authRestore
    if (typeof authRestoreCommand === 'function') {
      const invocation = await authRestoreCommand() as Awaited<ReturnType<typeof commands.authRestore>>
      if (invocation.status === 'ok') {
        const restored = normalizeAppResult(invocation.data)
        if (restored.ok) cloudAuth.value = restored.data
      }
    }
    if (!cloudAuth.value && typeof authStatusCommand === 'function') {
      const invocation = await authStatusCommand() as Awaited<ReturnType<typeof commands.authStatusCommand>>
      const authResult = normalizeAppResult(invocation)
      if (authResult.ok) cloudAuth.value = authResult.data
    }
    const result = normalizeAppResult(await commands.settingsOverview())
    if (result.ok) overview.value = result.data
    else errorMessage.value = result.error.userMessage
    const preferenceResult = normalizeAppResult(await commands.subjectPreferencesGet())
    if (preferenceResult.ok) subjectPreferences.value = preferenceResult.data
    else errorMessage.value = preferenceResult.error.userMessage
    const reviewPreferenceResult = normalizeAppResult(await commands.reviewPreferencesGet())
    if (reviewPreferenceResult.ok) reviewPreferences.value = reviewPreferenceResult.data
    else errorMessage.value = reviewPreferenceResult.error.userMessage
    await Promise.all([
      loadStorageStatus(),
      loadStorageMigrationReceipt(),
      loadDeviceAccessStatus(),
      loadWindowsCompatibility(),
    ])
  }
  catch {
    errorMessage.value = '设置状态暂时无法读取，请重新打开应用后再试。'
  }
  finally {
    loading.value = false
  }
}

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

function authStatusLabel(kind: AuthStatusKind) {
  return {
    unconfigured: '未配置云端',
    signed_out: '未登录',
    verification_required: '等待邮箱验证',
    connected: '已连接',
    offline: '离线模式',
  }[kind]
}

async function signIn() {
  if (authBusy.value || !authEmail.value.trim() || !authPassword.value) return
  authBusy.value = true
  authMessage.value = ''
  try {
    const invocation = authMode.value === 'signUp'
      ? await commands.authSignUp({ email: authEmail.value.trim(), password: authPassword.value })
      : await commands.authSignIn({ email: authEmail.value.trim(), password: authPassword.value })
    if (invocation.status === 'error') throw new Error('auth command rejected')
    const result = normalizeAppResult(invocation.data)
    if (result.ok) {
      cloudAuth.value = result.data
      authMessage.value = result.data.status.kind === 'verification_required'
        ? '注册成功，请先完成邮箱验证，再回来登录。'
        : result.data.status.kind === 'connected' ? '已连接，正在开始第一次安全同步。' : '登录状态已更新。'
      if (result.data.status.kind === 'connected') {
        void globalSyncController?.run('manual')
      }
    }
    else authMessage.value = result.error.userMessage
  }
  catch {
    authMessage.value = '登录请求没有完成，本地数据不受影响。'
  }
  finally {
    authBusy.value = false
  }
}

async function disconnectCloud(): Promise<boolean> {
  if (authBusy.value) return false
  authBusy.value = true
  authMessage.value = ''
  try {
    const invocation = await commands.authDisconnect()
    if (invocation.status === 'error') throw new Error('auth command rejected')
    const result = normalizeAppResult(invocation.data)
    if (result.ok) {
      cloudAuth.value = result.data
      return true
    }
    authMessage.value = result.error.userMessage
    return false
  }
  catch {
    authMessage.value = '退出云端账户没有完成。'
    return false
  }
  finally {
    authBusy.value = false
  }
}

function openLibraryLock(mode: 'lock' | 'sign-out', event: Event) {
  lockDialogMode.value = mode
  lockErrorMessage.value = ''
  lockReturnFocus.value = event.currentTarget as HTMLButtonElement
  lockDialogOpen.value = true
}

async function closeLibraryLock() {
  if (lockingLibrary.value) return
  lockDialogOpen.value = false
  await nextTick()
  lockReturnFocus.value?.focus()
}

async function confirmLibraryLock() {
  if (lockingLibrary.value) return
  lockingLibrary.value = true
  lockErrorMessage.value = ''
  try {
    if (lockDialogMode.value === 'sign-out' && !await disconnectCloud()) {
      lockErrorMessage.value = authMessage.value || '云端账户没有退出，本地资料库保持打开。'
      lockingLibrary.value = false
      return
    }
    const result = normalizeAppResult(await commands.libraryLock())
    if (!result.ok) {
      lockErrorMessage.value = result.error.userMessage
      lockingLibrary.value = false
      return
    }
    libraryAccessController?.enterRestarting()
    // Success deliberately stays busy: Rust restarts into the locked boundary.
  }
  catch {
    lockErrorMessage.value = '本地资料库没有完成锁定，当前资料保持打开，请稍后再试。'
    lockingLibrary.value = false
  }
}

async function syncNow() {
  if (syncBusy.value) return
  syncBusy.value = true
  syncMessage.value = ''
  try {
    const result = globalSyncController
      ? await globalSyncController.run('manual')
      : await (async () => {
          const invocation = await commands.syncNow()
          if (invocation.status === 'error') throw new Error('sync command rejected')
          return normalizeAppResult(invocation.data)
        })()
    if (result.ok) {
      syncMessage.value = `同步完成：上传 ${result.data.pushedOperationCount} 项，拉取 ${result.data.pulledChangeCount} 项。`
      const overviewResult = normalizeAppResult(await commands.settingsOverview())
      if (overviewResult.ok) overview.value = overviewResult.data
      await nextTick()
      await conflictCenter.value?.reload()
    }
    else syncMessage.value = result.error.userMessage
  }
  catch {
    syncMessage.value = '同步请求没有完成，待同步变更会保留并等待下次重试。'
  }
  finally {
    syncBusy.value = false
  }
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

async function chooseBackend(kind: CloudBackendKind) {
  if (kind === 'tencent') return
  const current = backendStatus.value
  if (backendBusy.value || (current?.ok && current.data.kind === kind)) return
  backendBusy.value = true
  backendMessage.value = ''
  try {
    const result = await setSyncBackend(kind)
    if (result.ok) {
      backendStatus.value = result
      backendMessage.value = `已选择 ${backendKindLabel(kind)}`
    }
    else {
      backendMessage.value = result.error.userMessage
    }
  }
  catch {
    backendMessage.value = '同步后端设置暂时不可用，本地数据不会受到影响。'
  }
  finally {
    backendBusy.value = false
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
      <div><p>设置 · 本地资料库</p><h1>数据安静地待在该在的地方</h1><span>这里展示真实状态；尚未接通的云能力会明确标注。</span></div>
      <button
        type="button"
        :disabled="loading"
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

    <section
      id="settings-sync"
      class="backend-panel"
      aria-labelledby="backend-settings-title"
      :aria-busy="backendBusy"
    >
      <header>
        <div>
          <p>同步后端 · 可替换的云服务</p>
          <h2 id="backend-settings-title">
            数据始终先保存在本地
          </h2>
          <span>选择只影响未来的同步尝试；未配置或未启用的远程服务不会上传任何数据。</span>
        </div>
        <CloudOff :size="24" />
      </header>
      <div
        v-if="backendStatus?.ok"
        class="backend-current"
        role="status"
        aria-live="polite"
      >
        <span
          class="backend-status-dot"
          :class="{ ready: backendStatus.data.ready }"
          aria-hidden="true"
        />
        <span>
          <strong>{{ backendStatusLabel(backendStatus.data) }}</strong>
          <small>{{ backendStatus.data.syncEnabled ? '云端同步已启用' : '云端同步未启用，待同步变更会安全保留' }}</small>
        </span>
      </div>
      <p
        v-else-if="backendStatus && !backendStatus.ok"
        class="backend-message warning"
        role="alert"
      >
        {{ backendStatus.error.userMessage }}
      </p>
      <div class="backend-options">
        <button
          v-for="option in backendOptions"
          :key="option.kind"
          type="button"
          :class="['backend-option', { selected: option.available && backendStatus?.ok && backendStatus.data.kind === option.kind, unavailable: !option.available }]"
          :disabled="backendBusy || !option.available"
          @click="chooseBackend(option.kind)"
        >
          <span
            class="backend-option-mark"
            aria-hidden="true"
          />
          <span>
            <strong>
              {{ option.title }}
              <em
                v-if="option.badge"
                class="capability-badge"
              >{{ option.badge }}</em>
            </strong>
            <small>{{ option.hint }}</small>
          </span>
        </button>
      </div>
      <p
        v-if="backendMessage"
        class="backend-message"
        role="status"
      >
        {{ backendMessage }}
      </p>
    </section>

    <section
      v-if="cloudAuth"
      class="cloud-auth-panel"
      aria-labelledby="cloud-auth-title"
    >
      <header>
        <div>
          <p>账户与跨设备</p>
          <h2 id="cloud-auth-title">
            云端同步账户
          </h2>
          <span>登录只用于同步；题库仍先写入本机加密库。刷新、退出或网络中断都不会清空本地资料。</span>
        </div>
        <CloudOff :size="24" />
      </header>
      <aside
        class="regional-sync-note"
        role="note"
      >
        <strong>国内网络提示</strong>
        <span>Supabase 在中国大陆可能出现连接超时或验证邮件延迟。遇到这种情况请保持“仅本地”继续使用；待同步变更会保留，之后网络恢复再手动同步。</span>
      </aside>
      <div
        class="cloud-auth-status"
        role="status"
        aria-live="polite"
      >
        <span
          class="backend-status-dot"
          :class="{ ready: cloudAuth.status.kind === 'connected', offline: cloudAuth.status.kind === 'offline' }"
          aria-hidden="true"
        />
        <strong>{{ authStatusLabel(cloudAuth.status.kind) }}</strong>
        <small v-if="cloudAuth.status.emailHint">{{ cloudAuth.status.emailHint }}</small>
      </div>
      <form
        v-if="cloudAuth.configured && cloudAuth.status.kind !== 'connected'"
        class="cloud-auth-form"
        @submit.prevent="signIn"
      >
        <label>邮箱<input
          v-model="authEmail"
          type="email"
          autocomplete="username"
          placeholder="you@example.com"
          required
        ></label>
        <label>密码<input
          v-model="authPassword"
          type="password"
          autocomplete="current-password"
          minlength="8"
          required
        ></label>
        <button
          type="submit"
          class="primary-action"
          :disabled="authBusy"
        >
          {{ authBusy ? '连接中…' : authMode === 'signUp' ? '注册并连接' : '登录并连接' }}
        </button>
      </form>
      <button
        v-if="cloudAuth.configured && cloudAuth.status.kind !== 'connected'"
        type="button"
        class="auth-mode-toggle"
        @click="authMode = authMode === 'signIn' ? 'signUp' : 'signIn'"
      >
        {{ authMode === 'signIn' ? '还没有账户？注册' : '已有账户？返回登录' }}
      </button>
      <div
        v-else-if="cloudAuth.status.kind === 'connected'"
        class="cloud-auth-actions"
      >
        <button
          type="button"
          class="primary-action"
          :disabled="syncBusy"
          @click="syncNow"
        >
          {{ syncBusy ? '同步中…' : '立即同步' }}
        </button>
        <button
          type="button"
          :disabled="authBusy"
          @click="openLibraryLock('sign-out', $event)"
        >
          退出云端并锁定
        </button>
      </div>
      <p
        v-else
        class="backend-message warning"
      >
        当前构建没有云端地址，仍可正常使用全部本地功能。
      </p>
      <p
        v-if="authMessage"
        class="backend-message"
        role="status"
      >
        {{ authMessage }}
      </p>
      <p
        v-if="syncMessage"
        class="backend-message"
        role="status"
      >
        {{ syncMessage }}
      </p>
    </section>

    <section
      v-if="overview"
      id="settings-overview"
      class="settings-grid"
      :aria-busy="loading"
    >
      <article class="setting-card encryption-card device-security-card">
        <div class="icon">
          <LockKeyhole :size="22" />
        </div>
        <div class="device-security-copy">
          <p>当前设备保护</p>
          <h2>这台 Windows 电脑</h2>
          <span>这里只显示当前设备的真实保护状态，不暴露设备编号、密钥或本机路径。</span>
          <Transition
            name="device-status"
            mode="out-in"
          >
            <dl
              :key="deviceStatusKey"
              class="device-security-list"
            >
              <div>
                <dt>本地资料库</dt>
                <dd>{{ overview?.localEncryptionReady ? 'SQLCipher 与原图独立加密' : '正在检查加密状态' }}</dd>
              </div>
              <div>
                <dt>离线解锁</dt>
                <dd>{{ deviceAccessStatus?.trustedWindowsAccount ? '当前 Windows 账户可解锁' : '状态暂不可用' }}</dd>
              </div>
              <div>
                <dt>云端会话</dt>
                <dd>{{ cloudAuth ? authStatusLabel(cloudAuth.status.kind) : '正在检查' }}</dd>
              </div>
            </dl>
          </Transition>
          <span
            v-if="cloudAuth?.status.kind === 'connected' || cloudAuth?.status.kind === 'offline'"
            class="device-scope-note"
          >退出云端只影响这台电脑，其他设备保持登录。</span>
          <span
            v-if="deviceAccessError"
            class="device-access-error"
            role="status"
            aria-label="当前设备保护状态"
          >{{ deviceAccessError }}</span>
          <button
            type="button"
            class="lock-now"
            @click="openLibraryLock('lock', $event)"
          >
            <LockKeyhole :size="15" />立即锁定资料库
          </button>
        </div>
        <strong><ShieldCheck :size="15" />本机安全边界</strong>
      </article>
      <article class="setting-card">
        <div class="icon">
          <Database :size="22" />
        </div><div><p>题库状态</p><h2>{{ overview?.activeProblemCount ?? 0 }} 道活动题</h2><span><Archive :size="13" /> {{ overview?.archivedProblemCount ?? 0 }} 道归档 · <Trash2 :size="13" /> {{ overview?.trashedProblemCount ?? 0 }} 道回收站</span></div>
      </article>
      <article
        v-if="windowsCompatibility"
        class="setting-card windows-compatibility-card"
        aria-label="Windows 兼容性"
      >
        <div class="icon">
          <Laptop :size="22" />
        </div>
        <div>
          <p>Windows 兼容性</p>
          <h2>
            {{ windowsCompatibility.supportLevel === 'supported'
              ? '完整支持'
              : windowsCompatibility.supportLevel === 'extended' ? '扩展兼容' : '不受支持' }}
          </h2>
          <span>
            {{ windowsCompatibility.osName }}
            · Build {{ windowsCompatibility.buildNumber }}.{{ windowsCompatibility.updateBuildRevision }}
            · {{ windowsCompatibility.nativeArchitecture }}
          </span>
          <small>
            WebView2 {{ windowsCompatibility.webview2Version ?? '未检测到' }}
            · 最低 Build {{ windowsCompatibility.minimumWindowsBuild }}
          </small>
        </div>
      </article>
      <article class="setting-card">
        <div class="icon">
          <CloudOff :size="22" />
        </div><div><p>云端同步</p><h2>{{ overview?.cloudSyncConfigured ? '已配置' : '尚未配置' }}</h2><span>待同步变更 {{ overview?.pendingOperationCount ?? 0 }} 项；配置账户后可从上方手动同步。</span></div>
      </article>
      <article class="setting-card">
        <div class="icon">
          <TriangleAlert :size="22" />
        </div><div><p>需要关注</p><h2>{{ (overview?.failedOperationCount ?? 0) + (overview?.unresolvedConflictCount ?? 0) }} 项</h2><span>{{ overview?.failedOperationCount ?? 0 }} 项失败操作 · {{ overview?.unresolvedConflictCount ?? 0 }} 项未解决冲突</span></div>
      </article>
    </section>
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

    <section
      v-if="subjectPreferences"
      id="settings-subjects"
      class="subject-panel"
      aria-labelledby="subject-settings-title"
    >
      <header>
        <div>
          <p>采集偏好 · 当前学习档案</p>
          <h2 id="subject-settings-title">
            常用科目
          </h2>
          <span>勾选会出现在采集台顶部；一键即可把整批题统一设置为同一科目。</span>
        </div>
        <BookOpen :size="24" />
      </header>
      <div class="builtin-subjects">
        <label
          v-for="subject in builtinSubjects"
          :key="subject"
          :class="{ selected: subjectPreferences.enabledSubjects.includes(subject) }"
        >
          <input
            v-model="subjectPreferences.enabledSubjects"
            type="checkbox"
            :value="subject"
            :disabled="subjectPreferences.enabledSubjects.length === 1 && subjectPreferences.enabledSubjects.includes(subject)"
          >
          <span>{{ subject }}</span>
        </label>
      </div>
      <div class="custom-subjects">
        <span
          v-for="subject in subjectPreferences.customSubjects"
          :key="subject"
        >{{ subject }}<button
          type="button"
          :aria-label="`删除自定义科目 ${subject}`"
          @click="removeCustomSubject(subject)"
        ><Trash2 :size="13" /></button></span>
      </div>
      <div class="subject-controls">
        <form @submit.prevent="addCustomSubject">
          <input
            v-model="customSubject"
            maxlength="40"
            placeholder="例如：编程、竞赛数学"
          >
          <button
            type="submit"
            aria-label="添加自定义科目"
          >
            <Plus :size="15" />添加
          </button>
        </form>
        <label class="sound-toggle">
          <input
            v-model="subjectPreferences.captureSoundEnabled"
            type="checkbox"
          >
          <Volume2 :size="16" /><span><strong>拖放成功音效</strong><small>仅在题卡融合成功后播放短提示音</small></span>
        </label>
        <button
          type="button"
          class="save-subjects"
          :disabled="savingSubjects"
          @click="saveSubjectPreferences"
        >
          {{ savingSubjects ? '保存中…' : '保存科目配置' }}
        </button>
      </div>
      <p
        v-if="subjectMessage"
        class="subject-message"
        role="status"
      >
        {{ subjectMessage }}
      </p>
    </section>

    <section
      v-if="reviewPreferences"
      id="settings-review"
      class="review-rhythm-panel"
      aria-labelledby="review-rhythm-title"
    >
      <header>
        <div>
          <p>训练偏好 · 当前学习档案</p>
          <h2 id="review-rhythm-title">
            训练间的专注插曲
          </h2>
          <span>舒尔特 5×5 只改变普通训练节奏；每轮都能跳过，模拟考试不会插入专注环节。</span>
        </div>
      </header>
      <div class="rhythm-options">
        <label
          v-for="option in reviewFocusOptions"
          :key="option.value"
          :class="{ selected: reviewPreferences.focusPolicy === option.value }"
        >
          <input
            v-model="reviewPreferences.focusPolicy"
            type="radio"
            name="review-focus-policy"
            :value="option.value"
          >
          <span
            class="rhythm-mark"
            aria-hidden="true"
          />
          <span><strong>{{ option.title }}</strong><small>{{ option.hint }}</small></span>
        </label>
      </div>
      <footer class="rhythm-actions">
        <p
          v-if="reviewPreferenceMessage"
          role="status"
        >
          {{ reviewPreferenceMessage }}
        </p>
        <span v-else>正在进行的训练保持原节奏，避免中途突然改变。</span>
        <button
          type="button"
          :disabled="savingReviewPreferences"
          @click="saveReviewPreferences"
        >
          {{ savingReviewPreferences ? '保存中…' : '保存训练节奏' }}
        </button>
      </footer>
    </section>

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
    <OcrCapabilityPanel id="settings-ocr" />

    <section
      id="settings-storage"
      class="storage-panel"
      aria-labelledby="storage-settings-title"
    >
      <header>
        <div>
          <p>本机加密资料 · 可迁移</p>
          <h2 id="storage-settings-title">
            资料库存储位置
          </h2>
          <span>显示经过边界处理的位置名称和真实密文容量；页面不会读取、保存或传递任意文件路径。</span>
        </div>
        <Database :size="24" />
      </header>

      <aside
        v-if="storageReceiptCopy"
        :class="['storage-receipt', storageReceiptCopy.kind]"
        :role="storageReceiptCopy.kind === 'success' ? 'status' : 'alert'"
        aria-label="存储迁移结果"
        :aria-live="storageReceiptCopy.kind === 'success' ? 'polite' : 'assertive'"
      >
        <component
          :is="storageReceiptCopy.kind === 'success' ? CheckCircle2 : TriangleAlert"
          :size="19"
        />
        <span><strong>{{ storageReceiptCopy.title }}</strong><small>{{ storageReceiptCopy.detail }}</small></span>
      </aside>

      <div
        v-if="storageStatus"
        class="storage-summary"
      >
        <article class="storage-location">
          <span class="storage-location-icon"><FolderCheck :size="21" /></span>
          <span>
            <small>{{ storageStatus.kind === 'custom' ? '当前使用自定义位置' : '当前使用默认位置' }}</small>
            <strong>{{ storageStatus.locationLabel }}</strong>
          </span>
          <span
            v-if="storageStatus.migrationPending"
            class="storage-pending"
          >等待重启校验</span>
        </article>
        <dl class="storage-usage">
          <div><dt>加密数据库</dt><dd>{{ formatBytes(storageStatus.databaseBytes) }}</dd></div>
          <div><dt>加密图片</dt><dd>{{ formatBytes(storageStatus.assetBytes) }}</dd></div>
          <div><dt>合计占用</dt><dd>{{ formatBytes(storageTotalBytes) }}</dd></div>
        </dl>
        <footer class="storage-actions">
          <span>选择新的本机磁盘或文件夹；校验失败会自动保留原位置。</span>
          <button
            ref="storageTrigger"
            type="button"
            :disabled="storageMigrating || storageStatus.migrationPending"
            @click="openStorageMigration"
          >
            <FolderCheck :size="16" />
            {{ storageStatus.migrationPending ? '等待安全重启' : '迁移资料库' }}
          </button>
        </footer>
      </div>
      <p
        v-else
        class="storage-unavailable"
        role="status"
      >
        {{ storageStatusError || '正在读取资料库容量…' }}
      </p>
    </section>

    <section
      id="settings-backup"
      class="backup-panel"
    >
      <header>
        <div><p>备份与恢复 · 本机加密</p><h2>完整快照、双重校验与自动回滚</h2><span>数据库和原图密文会一起备份；清单仅记录密文哈希与大小，不包含题图明文、本机绝对路径或原始账户标识。</span></div>
        <div class="backup-actions">
          <button
            type="button"
            :disabled="creatingBackup || preparingRestore || restoring"
            @click="createBackup"
          >
            <ArchiveRestore :size="16" />{{ creatingBackup ? '正在创建…' : '创建加密备份' }}
          </button>
          <button
            type="button"
            :disabled="creatingBackup || preparingRestore || restoring"
            @click="prepareRestore"
          >
            <FolderCheck :size="16" />{{ preparingRestore ? '正在校验并暂存…' : '选择备份并准备恢复' }}
          </button>
        </div>
      </header>
      <p class="backup-boundary">
        当前备份依赖这台可信 Windows 设备保存的加密凭据；跨设备恢复将在账户同步阶段使用正式密钥封装接入，不使用弱口令派生方案。
      </p>
      <div
        v-if="createdBackup || restoreCandidate"
        class="backup-results"
      >
        <article v-if="createdBackup">
          <strong><ShieldCheck :size="16" />加密备份已创建</strong>
          <span>{{ createdBackup.label }}</span>
          <small>{{ formatBackupTime(createdBackup.createdAtUtcMs) }} · {{ createdBackup.assetCount }} 个资源 · {{ formatBytes(createdBackup.encryptedBytes) }}</small>
        </article>
        <article
          v-if="restoreCandidate"
          class="restore-ready-card"
        >
          <strong><FolderCheck :size="16" />安全恢复包已就绪</strong>
          <span>{{ restoreCandidate.summary.label }}</span>
          <small>{{ restoreCandidate.summary.assetCount }} 个资源 · {{ formatBytes(restoreCandidate.summary.encryptedBytes) }} · 已复制到隔离区并再次校验，当前资料库尚未改变</small>
          <button
            ref="restoreTrigger"
            type="button"
            :disabled="restoring"
            @click="restoreDialogOpen = true"
          >
            查看风险并确认恢复
          </button>
        </article>
      </div>
    </section>

    <section
      id="settings-diagnostics"
      class="diagnostic-panel"
      aria-labelledby="diagnostic-settings-title"
    >
      <header>
        <div>
          <p>问题排查 · 隐私优先</p>
          <h2 id="diagnostic-settings-title">
            安全诊断报告
          </h2>
          <span>当启动、存储、同步或采集出现问题时，生成一份可交给支持人员的固定格式报告。</span>
        </div>
        <FileJson2 :size="24" />
      </header>

      <div class="diagnostic-contract">
        <span class="diagnostic-contract-icon"><ShieldCheck :size="21" /></span>
        <div>
          <strong>先看清内容，再决定是否发送</strong>
          <p>不会包含题图、答案、笔记、账户信息或本机路径</p>
          <ul>
            <li>仅包含应用版本、Windows 架构和数据库结构版本</li>
            <li>只统计题目、资源、采集批次、训练、导出和同步数量</li>
            <li>完整性检查只写“通过 / 未通过”，不写数据库原始错误</li>
          </ul>
        </div>
        <button
          ref="diagnosticsTrigger"
          type="button"
          :disabled="exportingDiagnostics || !isTauri()"
          @click="exportDiagnostics"
        >
          <FileJson2 :size="16" />
          {{ exportingDiagnostics ? '正在检查并生成…' : '生成安全诊断报告' }}
        </button>
      </div>

      <Transition name="diagnostic-receipt">
        <article
          v-if="diagnosticsReceipt"
          class="diagnostic-receipt"
          role="status"
          aria-label="诊断报告已生成"
          aria-live="polite"
        >
          <span class="diagnostic-receipt-seal"><CheckCircle2 :size="20" /></span>
          <div>
            <strong>诊断报告已安全生成</strong>
            <span>{{ diagnosticsReceipt.fileLabel }}</span>
            <small>
              {{ formatBackupTime(diagnosticsReceipt.generatedAtUtcMs) }}
              · 报告编号 {{ diagnosticsReceipt.reportId }}
              · {{ diagnosticsReceipt.warningCount === 0 ? '所有检查通过' : `${diagnosticsReceipt.warningCount} 项需留意` }}
            </small>
          </div>
        </article>
      </Transition>
      <p
        v-if="diagnosticsMessage"
        class="diagnostic-error"
        role="alert"
        aria-label="诊断报告未生成"
      >
        <TriangleAlert :size="16" />
        {{ diagnosticsMessage }}
      </p>
    </section>

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
  </main>
</template>

<style scoped>
.settings-page { min-height: 100vh; padding: 42px clamp(24px,5vw,72px) 72px; background: radial-gradient(circle at 85% 0,rgba(33,51,45,.08),transparent 32%); }
.settings-return { display:flex; justify-content:space-between; gap:16px; align-items:center; margin-top:24px; padding:14px 18px; border:1px solid rgba(79,128,110,.25); border-radius:14px; background:rgba(229,239,233,.66); }.settings-return p { margin:0; color:var(--ink-muted); font-size:11px; }.settings-return button { flex:0 0 auto; }
#settings-sync,#settings-overview,#settings-subjects,#settings-review,#settings-ocr,#settings-storage,#settings-backup,#settings-diagnostics,.settings-migration-anchor { scroll-margin-top: 118px; }
header { display: flex; justify-content: space-between; gap: 24px; align-items: flex-start; margin-bottom: 28px; } header p, .roadmap-panel p, .migration-panel header p { margin: 0 0 8px; color: var(--cinnabar); font-size: 12px; font-weight: 800; letter-spacing: .14em; text-transform: uppercase; }
h1, h2 { margin: 0; font-family: Georgia,'Microsoft YaHei',serif; color: var(--green-deep); } h1 { font-size: clamp(28px,4vw,42px); } h2 { font-size: 21px; } header span { display: block; margin-top: 9px; color: var(--ink-muted); }
button { display: inline-flex; gap: 7px; align-items: center; padding: 10px 14px; border: 1px solid var(--line); border-radius: 10px; background: var(--paper-raised); cursor: pointer; } button:disabled { opacity: .5; }
.error-banner { padding: 12px; border-radius: 10px; background: rgba(185,88,63,.08); color: #843d2c; }
.state-copy { padding: 28px; border: 1px solid var(--line); border-radius: 14px; background: rgba(255,253,247,.7); color: var(--ink-muted); text-align: center; }
.cloud-auth-panel { margin-bottom: 16px; padding: 24px 26px; border: 1px solid rgba(185,88,63,.22); border-radius: 17px; background: linear-gradient(135deg,rgba(255,253,247,.96),rgba(247,235,220,.42)); box-shadow: 0 16px 48px rgba(34,48,43,.05); }.cloud-auth-panel>header { align-items: center; margin-bottom: 16px; }.cloud-auth-panel>header p { margin: 0 0 8px; color: var(--cinnabar); font-size: 12px; font-weight: 800; letter-spacing: .14em; }.cloud-auth-panel>header span { display: block; max-width: 680px; margin-top: 8px; color: var(--ink-muted); font-size: 12px; }.regional-sync-note { display: grid; gap: 4px; margin: 0 0 14px; padding: 11px 13px; color: #74594d; border: 1px solid rgba(185,88,63,.18); border-radius: 11px; background: rgba(247,225,216,.48); font-size: 11px; line-height: 1.55; }.regional-sync-note strong { color: #8d4635; font-size: 11px; }.cloud-auth-status { display: flex; gap: 9px; align-items: center; margin-bottom: 13px; padding: 11px 13px; border-radius: 11px; background: rgba(33,51,45,.07); }.cloud-auth-status small { color: var(--ink-muted); }.cloud-auth-form { display: grid; grid-template-columns: 1fr 1fr auto; gap: 10px; align-items: end; }.cloud-auth-form label { display: grid; gap: 5px; color: var(--ink-muted); font-size: 11px; }.cloud-auth-form input { min-width: 0; padding: 10px 12px; border: 1px solid var(--line); border-radius: 10px; background: var(--paper); }.primary-action { color: var(--paper); border-color: var(--green-deep); background: var(--green-deep); }.cloud-auth-actions { display: flex; gap: 9px; }.auth-mode-toggle { margin-top: 11px; padding: 0; border: 0; color: var(--green-deep); background: transparent; font-size: 11px; }.cloud-auth-panel .backend-message { margin-top: 12px; }
.backend-panel { margin-bottom: 16px; padding: 24px 26px; border: 1px solid rgba(33,51,45,.22); border-radius: 17px; background: linear-gradient(135deg,rgba(255,253,247,.94),rgba(220,228,220,.34)); box-shadow: 0 16px 48px rgba(34,48,43,.05); }.backend-panel>header { align-items: center; margin-bottom: 16px; }.backend-panel>header>svg { color: var(--green-deep); }.backend-panel>header p { margin: 0 0 8px; color: var(--cinnabar); font-size: 12px; font-weight: 800; letter-spacing: .14em; }.backend-panel>header span { display: block; margin-top: 8px; color: var(--ink-muted); font-size: 12px; }.backend-current { display: flex; gap: 9px; align-items: center; margin-bottom: 13px; padding: 11px 13px; border-radius: 11px; background: rgba(33,51,45,.07); }.backend-current>span:last-child { display: grid; gap: 3px; }.backend-current strong { color: var(--green-deep); font-size: 13px; }.backend-current small { color: var(--ink-muted); font-size: 11px; }.backend-status-dot { width: 9px; height: 9px; border-radius: 50%; background: var(--cinnabar); box-shadow: 0 0 0 4px rgba(185,88,63,.13); }.backend-status-dot.ready { background: #557263; box-shadow: 0 0 0 4px rgba(85,114,99,.14); }.backend-options { display: grid; grid-template-columns: repeat(3,minmax(0,1fr)); gap: 10px; }.backend-option { display: grid; grid-template-columns: 17px 1fr; gap: 10px; align-items: start; min-height: 92px; padding: 14px; border: 1px solid var(--line); border-radius: 12px; background: rgba(255,253,247,.65); cursor: pointer; text-align: left; transition: transform var(--motion-feedback) var(--ease-standard), border-color var(--motion-standard) var(--ease-standard), background var(--motion-standard) var(--ease-standard), box-shadow var(--motion-standard) var(--ease-standard); }.backend-option:hover:not(:disabled) { transform: translateY(-2px); border-color: rgba(33,51,45,.38); box-shadow: 0 10px 24px rgba(34,48,43,.08); }.backend-option.selected { border-color: var(--green-deep); background: var(--green-soft); box-shadow: inset 0 0 0 1px var(--green-deep); }.backend-option:disabled { cursor: wait; }.backend-option.unavailable { opacity: .62; cursor: not-allowed; }.backend-option.unavailable .backend-option-mark { border-style: dashed; background: transparent; box-shadow: none; }.backend-option-mark { width: 15px; height: 15px; margin-top: 2px; border: 1px solid var(--sand-deep); border-radius: 50%; background: var(--paper-raised); box-shadow: inset 0 0 0 4px var(--paper-raised); }.backend-option.selected .backend-option-mark { border-color: var(--green-deep); background: var(--green-deep); }.backend-option>span:last-child { display: grid; gap: 5px; }.backend-option strong { color: var(--green-deep); font-size: 13px; }.capability-badge { display: inline-flex; margin-left: 5px; padding: 2px 6px; color: #7d5d45; border-radius: 999px; background: rgba(232,221,199,.7); font-size: 9px; font-style: normal; vertical-align: middle; }.backend-option small { color: var(--ink-muted); font-size: 10px; line-height: 1.55; }.backend-message { margin: 12px 0 0; color: #557263; font-size: 11px; }.backend-message.warning { color: #843d2c; font-weight: 700; }.settings-grid { display: grid; grid-template-columns: repeat(2,minmax(0,1fr)); gap: 15px; }.setting-card, .roadmap-panel, .migration-panel, .backup-panel, .subject-panel, .review-rhythm-panel, .storage-panel, .diagnostic-panel { border: 1px solid var(--line); border-radius: 17px; background: rgba(255,253,247,.78); box-shadow: 0 16px 48px rgba(34,48,43,.05); }
.setting-card { position: relative; display: grid; grid-template-columns: 48px 1fr; gap: 14px; min-height: 150px; padding: 22px; }.setting-card .icon { display: grid; width: 44px; height: 44px; place-items: center; border-radius: 13px; background: var(--green-soft); color: var(--green-deep); }.setting-card p { margin: 1px 0 8px; color: var(--ink-muted); font-size: 11px; font-weight: 800; letter-spacing: .08em; text-transform: uppercase; }.setting-card span { display: flex; gap: 5px; align-items: center; margin-top: 10px; color: var(--ink-muted); font-size: 12px; line-height: 1.7; }.setting-card > strong { position: absolute; right: 18px; bottom: 16px; display: flex; gap: 5px; align-items: center; color: #557263; font-size: 10px; }
.encryption-card { min-height: 192px; padding-bottom: 46px; border-color: rgba(33,51,45,.28); }.device-security-card { grid-column: 1 / -1; grid-template-columns: 48px minmax(0,1fr); }.device-security-copy { min-width: 0; }.device-security-list { display: grid; grid-template-columns: repeat(3,minmax(0,1fr)); gap: 9px; margin: 15px 0 0; }.device-security-list div { min-width: 0; padding: 11px 12px; border: 1px solid rgba(33,51,45,.12); border-radius: 11px; background: rgba(232,221,199,.24); }.device-security-list dt { color: var(--ink-muted); font-size: 10px; font-weight: 800; letter-spacing: .06em; }.device-security-list dd { margin: 5px 0 0; overflow-wrap: anywhere; color: var(--green-deep); font-size: 12px; font-weight: 700; }.device-status-enter-active,.device-status-leave-active { transition: opacity var(--motion-standard) var(--ease-standard),transform var(--motion-standard) var(--ease-standard); }.device-status-enter-from { opacity: 0; transform: translateY(5px); }.device-status-leave-to { opacity: 0; transform: translateY(-3px); }.device-security-copy .device-scope-note,.device-security-copy .device-access-error { display: block; margin-top: 10px; }.device-security-copy .device-scope-note { color: #557263; }.device-security-copy .device-access-error { color: #843d2c; }.lock-now { width: fit-content; margin-top: 13px; padding: 8px 11px; color: var(--green-deep); border-color: rgba(33,51,45,.25); background: var(--green-soft); font-size: 11px; font-weight: 700; transition: transform var(--motion-feedback) var(--ease-standard); }.lock-now:hover { transform: translateY(-1px); box-shadow: 0 8px 18px rgba(33,51,45,.1); }.roadmap-panel { margin-top: 16px; padding: 26px; }.roadmap-panel ol { display: grid; grid-template-columns: repeat(3,1fr); gap: 12px; margin: 22px 0 0; padding: 0; list-style: none; }.roadmap-panel li { padding: 17px; border-radius: 12px; background: rgba(232,221,199,.34); }.roadmap-panel strong, .roadmap-panel span { display: block; }.roadmap-panel span { margin-top: 7px; color: var(--ink-muted); font-size: 12px; line-height: 1.65; }
.subject-panel { margin-top: 16px; padding: 26px; }.subject-panel>header { align-items: center; margin-bottom: 18px; }.subject-panel>header p { margin: 0 0 8px; color: var(--cinnabar); font-size: 12px; font-weight: 800; letter-spacing: .14em; }.builtin-subjects { display: flex; gap: 9px; flex-wrap: wrap; }.builtin-subjects label { position: relative; cursor: pointer; }.builtin-subjects input { position: absolute; opacity: 0; pointer-events: none; }.builtin-subjects span { display: grid; min-width: 58px; min-height: 38px; padding: 0 13px; place-items: center; color: var(--ink-muted); border: 1px solid var(--line); border-radius: 999px; background: var(--paper); transition: transform var(--motion-feedback), color var(--motion-feedback), background var(--motion-feedback); }.builtin-subjects label.selected span { color: var(--paper); border-color: var(--green-deep); background: var(--green-deep); }.builtin-subjects label:hover span { transform: translateY(-1px); }.builtin-subjects input:focus-visible+span { outline: 3px solid rgba(185,88,63,.24); outline-offset: 2px; }.custom-subjects { display: flex; gap: 8px; flex-wrap: wrap; min-height: 8px; margin-top: 12px; }.custom-subjects>span { display: inline-flex; gap: 5px; align-items: center; padding: 6px 7px 6px 11px; color: #7e412f; border-radius: 999px; background: rgba(185,88,63,.1); font-size: 11px; }.custom-subjects button { display: grid; width: 25px; height: 25px; padding: 0; place-items: center; border: 0; border-radius: 50%; background: transparent; }.subject-controls { display: grid; grid-template-columns: minmax(280px,1fr) minmax(250px,1fr) auto; gap: 12px; align-items: center; margin-top: 17px; }.subject-controls form { display: flex; gap: 8px; }.subject-controls form input { flex: 1; min-width: 0; padding: 10px 12px; border: 1px solid var(--line); border-radius: 10px; background: var(--paper); }.sound-toggle { display: flex; gap: 9px; align-items: center; padding: 8px 11px; border: 1px solid var(--line); border-radius: 11px; cursor: pointer; }.sound-toggle>span { display: grid; }.sound-toggle small { color: var(--ink-muted); font-size: 9px; }.save-subjects { color: var(--paper); border-color: var(--green-deep); background: var(--green-deep); }.subject-message { margin: 12px 0 0; color: #557263; font-size: 12px; }
.review-rhythm-panel { margin-top: 16px; padding: 26px; }.review-rhythm-panel>header { margin-bottom: 18px; }.review-rhythm-panel>header p { margin: 0 0 8px; color: var(--cinnabar); font-size: 12px; font-weight: 800; letter-spacing: .14em; }.review-rhythm-panel>header span { max-width: 720px; }.rhythm-options { display: grid; grid-template-columns: repeat(3,minmax(0,1fr)); gap: 10px; }.rhythm-options label { position: relative; display: grid; min-height: 104px; grid-template-columns: 18px 1fr; gap: 10px; align-content: start; padding: 16px; border: 1px solid var(--line); border-radius: 7px 17px 17px; background: rgba(255,253,247,.68); cursor: pointer; transition: transform var(--motion-feedback) var(--ease-standard), border-color var(--motion-standard), background var(--motion-standard), box-shadow var(--motion-standard); }.rhythm-options label:hover { transform: translateY(-2px); }.rhythm-options label.selected { border-color: rgba(33,51,45,.4); background: var(--green-soft); box-shadow: 0 12px 28px rgba(33,51,45,.08); }.rhythm-options input { position: absolute; width: 1px; height: 1px; opacity: 0; }.rhythm-options input:focus-visible~.rhythm-mark { outline: 3px solid rgba(185,88,63,.35); outline-offset: 3px; }.rhythm-mark { width: 16px; height: 16px; margin-top: 2px; border: 1px solid var(--sand-deep); border-radius: 50%; background: var(--paper-raised); box-shadow: inset 0 0 0 4px var(--paper-raised); }.selected .rhythm-mark { border-color: var(--green-deep); background: var(--green-deep); }.rhythm-options label>span:last-child { display: grid; }.rhythm-options strong { color: var(--green-deep); font-size: 13px; }.rhythm-options small { margin-top: 7px; color: var(--ink-muted); font-size: 10px; line-height: 1.6; }.rhythm-actions { display: flex; gap: 16px; align-items: center; justify-content: space-between; margin-top: 16px; }.rhythm-actions p,.rhythm-actions span { margin: 0; color: #557263; font-size: 11px; }.rhythm-actions button { color: var(--paper); border-color: var(--green-deep); background: var(--green-deep); }
.storage-panel { margin-top: 16px; padding: 26px; }
.storage-panel > header { align-items: center; margin-bottom: 18px; }
.storage-panel > header p { margin: 0 0 8px; color: var(--cinnabar); font-size: 12px; font-weight: 800; letter-spacing: .14em; text-transform: uppercase; }
.storage-panel > header span { max-width: 720px; }
.storage-panel > header > svg { color: var(--green-deep); }
.storage-receipt { display: grid; grid-template-columns: 28px 1fr; gap: 10px; align-items: start; margin-bottom: 14px; padding: 13px 15px; border: 1px solid rgba(85,114,99,.18); border-radius: 12px; background: rgba(85,114,99,.08); color: #557263; }
.storage-receipt.warning { border-color: rgba(185,88,63,.2); background: rgba(185,88,63,.07); color: #843d2c; }
.storage-receipt span { display: grid; gap: 4px; }
.storage-receipt small { color: var(--ink-muted); line-height: 1.6; }
.storage-summary { display: grid; gap: 13px; }
.storage-location { display: grid; grid-template-columns: 46px 1fr auto; gap: 13px; align-items: center; padding: 15px; border: 1px solid rgba(33,51,45,.12); border-radius: 13px; background: linear-gradient(135deg,rgba(220,228,220,.4),rgba(255,253,247,.7)); }
.storage-location-icon { display: grid; width: 42px; height: 42px; place-items: center; color: var(--green-deep); border-radius: 12px; background: rgba(33,51,45,.1); }
.storage-location > span:nth-child(2) { display: grid; gap: 4px; min-width: 0; }
.storage-location small { color: var(--ink-muted); font-size: 10px; }
.storage-location strong { color: var(--green-deep); overflow-wrap: anywhere; }
.storage-pending { padding: 6px 9px; color: #8d4635; border-radius: 999px; background: rgba(185,88,63,.1); font-size: 10px; font-weight: 800; }
.storage-usage { display: grid; grid-template-columns: repeat(3,minmax(0,1fr)); gap: 10px; margin: 0; }
.storage-usage div { padding: 14px 15px; border-radius: 12px; background: rgba(232,221,199,.3); }
.storage-usage dt { color: var(--ink-muted); font-size: 10px; }
.storage-usage dd { margin: 7px 0 0; color: var(--green-deep); font-family: Georgia,serif; font-size: 22px; font-weight: 700; }
.storage-actions { display: flex; gap: 16px; align-items: center; justify-content: space-between; }
.storage-actions > span { color: var(--ink-muted); font-size: 11px; line-height: 1.6; }
.storage-actions button { flex: 0 0 auto; color: var(--paper); border-color: var(--green-deep); background: var(--green-deep); transition: transform var(--motion-feedback) var(--ease-standard), box-shadow var(--motion-feedback) var(--ease-standard); }
.storage-actions button:hover:not(:disabled) { transform: translateY(-1px); box-shadow: 0 8px 20px rgba(33,51,45,.14); }
.storage-unavailable { margin: 0; padding: 18px; color: var(--ink-muted); border: 1px dashed var(--line); border-radius: 12px; background: rgba(232,221,199,.2); text-align: center; }
.migration-panel { margin-top: 16px; padding: 26px; }.migration-panel header { margin-bottom: 20px; }.migration-panel header span { max-width: 680px; }.migration-panel header button { flex: 0 0 auto; }.migration-stats { display: grid; grid-template-columns: repeat(6,minmax(0,1fr)); gap: 10px; margin: 0; }.migration-stats div { padding: 15px; border-radius: 12px; background: rgba(232,221,199,.34); }.migration-stats dt { color: var(--ink-muted); font-size: 11px; }.migration-stats dd { margin: 6px 0 0; color: var(--green-deep); font-family: Georgia,serif; font-size: 25px; font-weight: 700; }.preflight-note { margin: 16px 0 0; color: var(--ink-muted); font-size: 12px; }.preflight-note.ready { color: #557263; }.issue-list { display: grid; gap: 8px; max-height: 330px; margin: 16px 0 0; padding: 0; overflow: auto; list-style: none; }.issue-list li { display: grid; grid-template-columns: 108px minmax(120px,.6fr) 1fr; gap: 12px; align-items: start; padding: 12px 14px; border-radius: 10px; background: rgba(185,88,63,.06); }.issue-list strong { color: #843d2c; font-size: 12px; }.issue-list span, .issue-list small { color: var(--ink-muted); font-size: 11px; overflow-wrap: anywhere; }
.backup-panel { margin-top: 16px; padding: 26px; }.backup-panel header { margin-bottom: 16px; }.backup-panel header p { margin: 0 0 8px; color: var(--cinnabar); font-size: 12px; font-weight: 800; letter-spacing: .14em; text-transform: uppercase; }.backup-panel header span { max-width: 690px; }.backup-actions { display: flex; flex: 0 0 auto; gap: 8px; }.backup-boundary { margin: 0; padding: 13px 15px; border-left: 3px solid var(--cinnabar); border-radius: 7px; background: rgba(232,221,199,.28); color: var(--ink-muted); font-size: 12px; line-height: 1.7; }.backup-results { display: grid; grid-template-columns: repeat(2,minmax(0,1fr)); gap: 10px; margin-top: 14px; }.backup-results article { display: grid; gap: 6px; padding: 15px; border-radius: 12px; background: rgba(33,51,45,.055); }.backup-results strong { display: flex; gap: 6px; align-items: center; color: #557263; font-size: 13px; }.backup-results span { color: var(--green-deep); font-size: 12px; overflow-wrap: anywhere; }.backup-results small { color: var(--ink-muted); line-height: 1.6; }
.diagnostic-panel { margin-top: 16px; padding: 26px; overflow: hidden; }.diagnostic-panel > header { align-items: center; margin-bottom: 16px; }.diagnostic-panel > header p { margin: 0 0 8px; color: var(--cinnabar); font-size: 12px; font-weight: 800; letter-spacing: .14em; text-transform: uppercase; }.diagnostic-panel > header span { max-width: 690px; }.diagnostic-panel > header > svg { flex: 0 0 auto; color: var(--green-deep); }.diagnostic-contract { display: grid; grid-template-columns: 44px minmax(0,1fr) auto; gap: 14px; align-items: start; padding: 17px; border: 1px solid rgba(33,51,45,.15); border-radius: 14px; background: linear-gradient(135deg,rgba(220,228,220,.4),rgba(255,253,247,.62)); }.diagnostic-contract-icon { display: grid; width: 42px; height: 42px; place-items: center; color: var(--green-deep); border-radius: 13px 4px 13px 13px; background: rgba(33,51,45,.1); }.diagnostic-contract strong { display: block; color: var(--green-deep); font-size: 13px; }.diagnostic-contract p { margin: 5px 0 8px; color: #557263; font-size: 12px; font-weight: 750; }.diagnostic-contract ul { display: grid; gap: 4px; margin: 0; padding-left: 17px; color: var(--ink-muted); font-size: 10px; line-height: 1.55; }.diagnostic-contract button { align-self: center; justify-content: center; min-width: 176px; color: var(--paper); border-color: var(--green-deep); background: var(--green-deep); transition: transform var(--motion-feedback) var(--ease-standard), opacity var(--motion-feedback) var(--ease-standard); }.diagnostic-contract button:hover:not(:disabled) { transform: translateY(-1px); }.diagnostic-receipt { display: grid; grid-template-columns: 42px minmax(0,1fr); gap: 13px; align-items: center; margin-top: 13px; padding: 15px 17px; border: 1px solid rgba(85,114,99,.23); border-radius: 13px; background: rgba(85,114,99,.08); }.diagnostic-receipt-seal { display: grid; width: 38px; height: 38px; place-items: center; color: var(--paper); border-radius: 12px 4px 12px 12px; background: #557263; }.diagnostic-receipt div { display: grid; min-width: 0; gap: 4px; }.diagnostic-receipt strong { color: var(--green-deep); font-size: 13px; }.diagnostic-receipt div > span { color: #557263; font-size: 12px; font-weight: 750; overflow-wrap: anywhere; }.diagnostic-receipt small { color: var(--ink-muted); font-size: 10px; line-height: 1.55; overflow-wrap: anywhere; }.diagnostic-error { display: flex; gap: 8px; align-items: center; margin: 12px 0 0; padding: 11px 13px; color: #843d2c; border: 1px solid rgba(185,88,63,.23); border-radius: 11px; background: rgba(185,88,63,.08); font-size: 11px; }.diagnostic-error svg { flex: 0 0 auto; }.diagnostic-receipt-enter-active,.diagnostic-receipt-leave-active { transition: opacity var(--motion-standard) var(--ease-standard),transform var(--motion-standard) var(--ease-standard); }.diagnostic-receipt-enter-from,.diagnostic-receipt-leave-to { opacity: 0; transform: translateY(8px); }
.backup-results .restore-ready-card button { width: fit-content; margin-top: 5px; color: #fffdf7; border-color: var(--cinnabar); background: var(--cinnabar); transition: transform var(--motion-feedback), box-shadow var(--motion-feedback); }.backup-results .restore-ready-card button:hover { transform: translateY(-1px); box-shadow: 0 8px 20px rgba(185,88,63,.18); }
.preflight-note.warning { color: #843d2c; font-weight: 700; }
.builtin-subjects input { width: 1px; height: 1px; pointer-events: auto; }
@media (max-width: 980px) { .migration-stats { grid-template-columns: repeat(3,minmax(0,1fr)); }.subject-controls { grid-template-columns: 1fr; } }
@media (max-width: 760px) { .settings-page { padding: 24px 16px 92px; } .settings-grid { grid-template-columns: 1fr; } .backend-options { grid-template-columns: 1fr; } .cloud-auth-form { grid-template-columns: 1fr; }.cloud-auth-actions { flex-direction: column; }.cloud-auth-actions button { justify-content: center; }.device-security-list,.roadmap-panel ol, .migration-stats, .backup-results, .rhythm-options, .storage-usage { grid-template-columns: 1fr; } .migration-panel header, .backup-panel header, .storage-panel > header { flex-direction: column; }.backup-actions { width: 100%; flex-direction: column; }.backup-actions button { justify-content: center; }.issue-list li { grid-template-columns: 1fr; gap: 4px; }.rhythm-actions, .storage-actions { align-items: stretch; flex-direction: column; }.rhythm-actions button, .storage-actions button { justify-content: center; }.storage-location { grid-template-columns: 46px 1fr; }.storage-pending { grid-column: 1 / -1; width: fit-content; }.diagnostic-contract { grid-template-columns: 42px 1fr; }.diagnostic-contract button { grid-column: 1 / -1; width: 100%; } }
@media (prefers-reduced-motion: reduce) { .backend-option,.lock-now,.storage-actions button,.diagnostic-contract button,.diagnostic-receipt-enter-active,.diagnostic-receipt-leave-active,.device-status-enter-active,.device-status-leave-active { transition: none; }.backend-option:hover:not(:disabled),.lock-now:hover,.storage-actions button:hover:not(:disabled),.diagnostic-contract button:hover:not(:disabled),.diagnostic-receipt-enter-from,.diagnostic-receipt-leave-to,.device-status-enter-from,.device-status-leave-to { transform: none; } }
.backend-status-dot.offline { background: #b07a42; box-shadow: 0 0 0 4px rgba(176,122,66,.14); }
.sync-conflict-clear { display: flex; gap: 8px; align-items: center; margin: 16px 0 0; padding: 11px 14px; color: #557263; border: 1px solid rgba(85,114,99,.16); border-radius: 11px; background: rgba(85,114,99,.07); font-size: 12px; }
</style>
