<script setup lang="ts">
import { isTauri } from '@tauri-apps/api/core'
import { CheckCircle2, ShieldAlert, X } from '@lucide/vue'
import { computed, nextTick, onErrorCaptured, onMounted, onUnmounted, provide, ref, watch } from 'vue'
import { RouterView, useRoute, useRouter } from 'vue-router'
import { failure, type AppResult } from '../shared/api/app-result'
import { commands, type BackupRestoreReceipt, type ProfileSummary, type SyncNowReport, type SystemStatus, type WindowsCompatibilityStatus } from '../shared/api/bindings'
import { normalizeAppResult } from '../shared/api/normalize-result'
import { loadSystemStatus } from '../shared/api/system-status'
import AppShell, { type AppPage } from './AppShell.vue'
import StartupUpdateDialog from './components/StartupUpdateDialog.vue'
import { useLibraryAccessLifecycle } from './composables/useLibraryAccessLifecycle'
import { useProfileManagement } from './composables/useProfileManagement'
import { useStartupUpdate } from './composables/useStartupUpdate'
import LibraryAccessScreen from './LibraryAccessScreen.vue'
import { libraryAccessControllerKey } from './library-access-controller'
import { formatSettingsTime } from './settings-formatters'
import { createSyncController, syncControllerKey, syncStatusCopy, type SyncPhase, type SyncTrigger } from './sync-controller'
import { createWorkspaceTransitionGuard, workspaceTransitionGuardKey } from './workspace-transition-guard'

const route = useRoute()
const router = useRouter()
const desktopRuntime = isTauri()
const startupUpdate = useStartupUpdate({
  desktopRuntime,
  operations: {
    status: async () => {
      const invocation = await commands.windowsUpdateStatus()
      if (invocation.status === 'error') throw new Error('update status command rejected')
      return normalizeAppResult(invocation.data)
    },
    check: async () => {
      const invocation = await commands.windowsUpdateCheck()
      if (invocation.status === 'error') throw new Error('update check command rejected')
      return normalizeAppResult(invocation.data)
    },
    install: async (expectedVersion) => {
      const invocation = await commands.windowsUpdateInstall(expectedVersion)
      if (invocation.status === 'error') throw new Error('update install command rejected')
      return normalizeAppResult(invocation.data)
    },
  },
})
const {
  report: startupUpdateReport,
  installing: startupUpdateInstalling,
  message: startupUpdateMessage,
} = startupUpdate
const startupUpdatePublicationLabel = computed(() => {
  const value = startupUpdateReport.value?.publishedAt
  if (!value) return ''
  const timestamp = Date.parse(value)
  return Number.isFinite(timestamp) ? formatSettingsTime(timestamp) : ''
})
const workspaceTransitionGuard = createWorkspaceTransitionGuard()
provide(workspaceTransitionGuardKey, workspaceTransitionGuard)
const {
  phase: libraryAccessPhase,
  errorMessage: libraryAccessError,
  errorReason: libraryAccessErrorReason,
  workspaceInitialized,
  checkLibraryAccess: loadLibraryAccess,
  unlockLibrary,
  enterRestarting,
} = useLibraryAccessLifecycle({
  desktopRuntime,
  checkAccess: async () => normalizeAppResult(await commands.libraryAccessStatus()),
  unlock: async () => normalizeAppResult(await commands.libraryUnlock()),
  initializeWorkspace,
})
provide(libraryAccessControllerKey, {
  enterRestarting,
})
const activePage = computed(() => (route.meta.shellPage ?? route.name ?? 'dashboard') as AppPage)
type PageDirection = 'forward' | 'backward'
const pageOrder: Record<AppPage, number> = {
  dashboard: 0,
  inbox: 1,
  library: 2,
  review: 3,
  report: 4,
  settings: 5,
}
const pageDirection = ref<PageDirection>('forward')
const pageTransitionName = computed(() => `page-${pageDirection.value}`)
const systemStatus = ref<AppResult<SystemStatus>>()
const syncPhase = ref<SyncPhase>('local_only')
const automaticSyncCooldownMs = 15_000
let lastSuccessfulSyncAtUtcMs = 0
let cloudRestoreTask: Promise<void> | undefined
const shellSyncStatus = computed(() => {
  if (systemStatus.value === undefined) {
    return { label: '正在检查资料库', tone: 'neutral' as const }
  }
  if (!systemStatus.value.ok) {
    return { label: '状态检查失败', tone: 'warning' as const }
  }
  if (systemStatus.value.data.storage === 'preview') {
    return { label: '浏览器设计预览', tone: 'neutral' as const }
  }
  return syncStatusCopy(syncPhase.value)
})
const profileEpoch = ref(0)
const restoreNotice = ref<BackupRestoreReceipt>()
const windowsCompatibility = ref<WindowsCompatibilityStatus>()
const compatibilityNoticeDismissed = ref(false)
const routeError = ref('')
const routeErrorDetail = ref('')
const routeRenderEpoch = ref(0)
const routePage = ref<HTMLElement>()
const routePageKey = computed(() => `${route.fullPath}:${profileEpoch.value}:${routeRenderEpoch.value}`)
type RouteFocusRequest = { routePageKey: string; previousActive: Element | null }
const pendingRouteFocus = ref<RouteFocusRequest>()
const mutationSyncPhases = new Set<SyncPhase>([
  'idle',
  'syncing',
  'synced',
  'deferred_capture',
  'retry_waiting',
])
const syncController = createSyncController(performSync, {
  canScheduleMutation: () => mutationSyncPhases.has(syncPhase.value),
})
provide(syncControllerKey, syncController)
const profileManagement = useProfileManagement({
  enabled: desktopRuntime,
  listProfiles: async () => normalizeAppResult(await commands.profileList()),
  scheduleSync: () => syncController.scheduleMutation(),
  refreshWorkspace: async () => {
    profileEpoch.value += 1
    await router.push({ name: 'dashboard' })
  },
})
const {
  profiles,
  activeProfileId,
  busy: profileBusy,
  errorMessage: profileError,
  loadProfiles,
  mutateProfile,
} = profileManagement
const previewProfile: ProfileSummary = {
  id: 'preview-profile',
  name: '本机学习档案',
  createdAtUtcMs: 0,
  updatedAtUtcMs: 0,
  revision: 1,
}
const shellProfiles = computed(() =>
  profiles.value.length || desktopRuntime ? [...profiles.value] : [previewProfile],
)
const shellActiveProfileId = computed(() =>
  activeProfileId.value || (desktopRuntime ? '' : previewProfile.id),
)

watch(() => route.fullPath, () => {
  routeError.value = ''
  routeErrorDetail.value = ''
})

watch(routePageKey, requestRouteFocus)

watch(routeError, (error) => {
  if (error) requestRouteFocus()
})

watch(activePage, (next, previous) => {
  if (next === previous) return
  pageDirection.value = (pageOrder[next] ?? 0) >= (pageOrder[previous] ?? 0)
    ? 'forward'
    : 'backward'
})

function requestRouteFocus() {
  pendingRouteFocus.value = {
    routePageKey: routePageKey.value,
    previousActive: document.activeElement,
  }
  void nextTick(resolveRouteFocus)
}

function resolveRouteFocus({ allowPageFallback = false }: { allowPageFallback?: boolean } = {}) {
  const request = pendingRouteFocus.value
  const page = routePage.value
  if (!request || !page || page.dataset.routePageKey !== request.routePageKey) return

  const active = document.activeElement
  if (
    active !== request.previousActive
    && active !== document.body
    && active !== document.documentElement
  ) {
    pendingRouteFocus.value = undefined
    return
  }

  const heading = page.querySelector<HTMLElement>('h1')
  if (!heading && !allowPageFallback) return
  const target = heading ?? page
  if (heading && !heading.hasAttribute('tabindex')) heading.setAttribute('tabindex', '-1')
  target.focus({ preventScroll: true })
  pendingRouteFocus.value = undefined
}

function handleRoutePageEntered() {
  resolveRouteFocus()
}

function handleRouteContentResolved() {
  resolveRouteFocus({ allowPageFallback: true })
}

function retryCurrentRoute() {
  routeError.value = ''
  routeErrorDetail.value = ''
  routeRenderEpoch.value += 1
}

onErrorCaptured((error) => {
  routeError.value = '这个页面暂时打不开'
  routeErrorDetail.value = desktopRuntime
    ? '已保存的本地资料没有被修改。重试会重新打开此页面，未保存的页面输入可能需要重新填写。'
    : '浏览器预览遇到异常。重试会重新打开此页面，未保存的页面输入可能需要重新填写。'
  if (import.meta.env.DEV) console.error('route render error', error)
  return false
})

onMounted(() => {
  window.addEventListener('online', handleOnline)
  document.addEventListener('visibilitychange', handleVisibilityChange)
  startupUpdate.start()
  void loadWindowsCompatibility()
  void loadLibraryAccess()
})

onUnmounted(() => {
  window.removeEventListener('online', handleOnline)
  document.removeEventListener('visibilitychange', handleVisibilityChange)
  startupUpdate.dispose()
  syncController.dispose()
})

async function initializeWorkspace() {
  try {
    systemStatus.value = await loadSystemStatus()
  } catch {
    systemStatus.value = failure(
      'SYSTEM_STATUS_UNAVAILABLE',
      '资料库状态暂时无法读取。',
      true,
      'system-status-startup',
    )
  }
  await Promise.all([loadProfiles(), loadRestoreReceipt()])
  void restoreCloudAndSync('startup')
}

function handleOnline() {
  if (libraryAccessPhase.value !== 'unlocked' || !workspaceInitialized.value) return
  void restoreCloudAndSync('online')
}

function handleVisibilityChange() {
  if (
    document.visibilityState !== 'visible'
    || libraryAccessPhase.value !== 'unlocked'
    || !workspaceInitialized.value
  ) return
  void restoreCloudAndSync('visible')
}

function isNetworkFailure(code: string): boolean {
  return [
    'AUTH_NETWORK',
    'AUTH_TIMEOUT',
    'cloud_network',
    'cloud_timeout',
    'cloud_unavailable',
  ].includes(code)
}

function restoreCloudAndSync(reason: SyncTrigger): Promise<void> {
  if (cloudRestoreTask) return cloudRestoreTask
  if (!desktopRuntime) {
    syncPhase.value = 'local_only'
    return Promise.resolve()
  }
  if (
    reason === 'visible'
    && lastSuccessfulSyncAtUtcMs > 0
    && Date.now() - lastSuccessfulSyncAtUtcMs < automaticSyncCooldownMs
  ) {
    return Promise.resolve()
  }
  if (typeof navigator !== 'undefined' && navigator.onLine === false) {
    syncPhase.value = 'offline'
    return Promise.resolve()
  }

  const job = runCloudRestoreAndSync(reason)
  const tracked = job.finally(() => {
    if (cloudRestoreTask === tracked) cloudRestoreTask = undefined
  })
  cloudRestoreTask = tracked
  return tracked
}

async function runCloudRestoreAndSync(reason: SyncTrigger) {
  try {
    const invocation = await commands.authRestore()
    if (invocation.status === 'error') {
      syncPhase.value = 'retry_waiting'
      return
    }
    const result = normalizeAppResult(invocation.data)
    if (!result.ok) {
      syncPhase.value = isNetworkFailure(result.error.code) ? 'offline' : 'retry_waiting'
      return
    }
    switch (result.data.status.kind) {
      case 'connected':
        syncPhase.value = 'idle'
        await syncController.run(reason)
        return
      case 'offline':
        syncPhase.value = 'offline'
        return
      case 'unconfigured':
        syncPhase.value = 'local_only'
        return
      case 'signed_out':
      case 'verification_required':
        syncPhase.value = 'signed_out'
        return
    }
  }
  catch {
    syncPhase.value = 'offline'
  }
}

async function performSync(reason: SyncTrigger): Promise<AppResult<SyncNowReport>> {
  syncPhase.value = 'syncing'
  try {
    const invocation = await commands.syncNow()
    if (invocation.status === 'error') {
      const result = failure(
        'SYNC_COMMAND_UNAVAILABLE',
        '同步请求没有启动，本地内容已经保存。',
        true,
        'sync-command-unavailable',
      )
      syncPhase.value = 'retry_waiting'
      return result
    }
    const result = normalizeAppResult(invocation.data)
    if (!result.ok) {
      if (result.error.code === 'SYNC_CAPTURE_ACTIVE') {
        syncPhase.value = 'deferred_capture'
      }
      else if (result.error.code === 'SYNC_ALREADY_RUNNING') {
        syncPhase.value = 'syncing'
      }
      else {
        syncPhase.value = isNetworkFailure(result.error.code) ? 'offline' : 'retry_waiting'
      }
      return result
    }

    syncPhase.value = 'synced'
    lastSuccessfulSyncAtUtcMs = Date.now()
    await loadProfiles()
    if (
      result.data.pulledChangeCount > 0
      && reason !== 'mutation'
      && activePage.value !== 'review'
    ) {
      profileEpoch.value += 1
    }
    return result
  }
  catch {
    syncPhase.value = 'offline'
    return failure(
      'SYNC_REQUEST_FAILED',
      '暂时无法连接云端，本地内容已经保存并会等待重试。',
      true,
      'sync-request-failed',
    )
  }
}

const restoreNoticeCopy = computed(() => {
  const receipt = restoreNotice.value
  if (!receipt) return undefined
  if (receipt.status === 'succeeded') {
    return { kind: 'success', title: '资料库恢复成功', detail: `已安全恢复“${receipt.label}”，加密数据库和图片资源均已重新校验。` }
  }
  if (receipt.status === 'rolled_back') {
    return { kind: 'warning', title: '已自动换回原资料库', detail: `“${receipt.label}”未能正常启动，原有资料没有丢失。` }
  }
  return { kind: 'warning', title: '恢复包未通过最终校验', detail: `“${receipt.label}”没有替换当前资料库，请重新选择备份后再试。` }
})

const compatibilityNoticeCopy = computed(() => {
  const status = windowsCompatibility.value
  if (!status || status.supportLevel === 'supported' || compatibilityNoticeDismissed.value) return undefined
  if (status.supportLevel === 'unsupported') {
    return {
      title: '当前 Windows 环境不在支持范围',
      detail: `${status.summary} 建议先导出备份，再迁移到受支持的 Windows 11 x64 设备。`,
    }
  }
  return {
    title: '当前设备使用扩展兼容模式',
    detail: `${status.summary} 核心功能可继续使用，但发布前会优先在 Windows 11 x64 上完整验证。`,
  }
})

async function loadWindowsCompatibility() {
  if (!desktopRuntime) return
  try {
    const invocation = await commands.compatibilityStatus()
    if (invocation.status === 'error') return
    const result = normalizeAppResult(invocation.data)
    if (result.ok) windowsCompatibility.value = result.data
  }
  catch {
    // Compatibility guidance is supplementary and must never block library access.
  }
}

async function loadRestoreReceipt() {
  if (!desktopRuntime) return
  try {
    const result = normalizeAppResult(await commands.backupRestoreStatus())
    if (result.ok && result.data) restoreNotice.value = result.data
  }
  catch {
    // Recovery status is supplementary; profile and library access remain usable.
  }
}

async function createProfile(name: string): Promise<boolean> {
  if (!await workspaceTransitionGuard.attempt()) return false
  return mutateProfile(
    async () => normalizeAppResult(await commands.profileCreate({ name })),
    { refreshWorkspace: true, scheduleSync: true },
  )
}

function renameProfile(profileId: string, name: string) {
  return mutateProfile(
    async () => normalizeAppResult(await commands.profileRename({ profileId, name })),
    { refreshWorkspace: false, scheduleSync: true },
  )
}

async function deleteProfile(profileId: string, confirmationName: string): Promise<boolean> {
  const deletesActiveProfile = profileId === activeProfileId.value
  if (deletesActiveProfile && !await workspaceTransitionGuard.attempt()) return false
  return mutateProfile(
    async () => normalizeAppResult(await commands.profileDelete({ profileId, confirmationName })),
    { refreshWorkspace: deletesActiveProfile, scheduleSync: true },
  )
}

async function selectProfile(profileId: string): Promise<boolean> {
  if (profileId === activeProfileId.value) return false
  if (!await workspaceTransitionGuard.attempt()) return false
  return mutateProfile(
    async () => normalizeAppResult(await commands.profileSelect(profileId)),
    { refreshWorkspace: true, scheduleSync: false },
  )
}
</script>

<template>
  <LibraryAccessScreen
    v-if="libraryAccessPhase !== 'unlocked'"
    :phase="libraryAccessPhase"
    :message="libraryAccessError"
    :reason="libraryAccessErrorReason"
    @unlock="unlockLibrary"
    @retry="loadLibraryAccess"
  />
  <AppShell
    v-else
    :profiles="shellProfiles"
    :active-profile-id="shellActiveProfileId"
    :profile-busy="profileBusy"
    :profile-error="profileError"
    :active-page="activePage"
    :sync-status="shellSyncStatus"
    @navigate="router.push({ name: $event })"
    @profile-select="selectProfile"
    @profile-create="createProfile"
    @profile-rename="renameProfile"
    @profile-delete="deleteProfile"
    @profile-retry="loadProfiles"
  >
    <div class="global-notice-stack">
      <Transition name="restore-notice">
        <aside
          v-if="restoreNoticeCopy"
          :class="['restore-notice', restoreNoticeCopy.kind]"
          :role="restoreNoticeCopy.kind === 'success' ? 'status' : 'alert'"
          :aria-live="restoreNoticeCopy.kind === 'success' ? 'polite' : 'assertive'"
        >
          <component
            :is="restoreNoticeCopy.kind === 'success' ? CheckCircle2 : ShieldAlert"
            :size="21"
          />
          <span><strong>{{ restoreNoticeCopy.title }}</strong><small>{{ restoreNoticeCopy.detail }}</small></span>
          <button
            type="button"
            aria-label="关闭恢复结果通知"
            @click="restoreNotice = undefined"
          >
            <X :size="16" />
          </button>
        </aside>
      </Transition>
      <Transition name="restore-notice">
        <aside
          v-if="compatibilityNoticeCopy"
          class="restore-notice compatibility-notice warning"
          role="alert"
          aria-live="polite"
        >
          <ShieldAlert :size="21" />
          <span><strong>{{ compatibilityNoticeCopy.title }}</strong><small>{{ compatibilityNoticeCopy.detail }}</small></span>
          <button
            type="button"
            aria-label="关闭 Windows 兼容性通知"
            @click="compatibilityNoticeDismissed = true"
          >
            <X :size="16" />
          </button>
        </aside>
      </Transition>
    </div>
    <RouterView v-slot="{ Component }">
      <Transition
        :name="pageTransitionName"
        mode="out-in"
        @after-enter="handleRoutePageEntered"
      >
        <div
          :key="routePageKey"
          ref="routePage"
          class="route-page"
          :data-direction="pageDirection"
          :data-route-page-key="routePageKey"
          role="region"
          aria-label="页面内容"
          tabindex="-1"
        >
          <section
            v-if="routeError"
            class="route-error"
            role="alert"
          >
            <ShieldAlert :size="28" />
            <h1>{{ routeError }}</h1>
            <p>{{ routeErrorDetail }}</p>
            <div class="route-error-actions">
              <button
                type="button"
                @click="retryCurrentRoute"
              >
                重试当前页面
              </button>
              <button
                class="secondary"
                type="button"
                @click="router.push({ name: 'dashboard' })"
              >
                回到训练台
              </button>
            </div>
          </section>
          <Suspense
            v-else
            timeout="0"
            @resolve="handleRouteContentResolved"
          >
            <template v-if="Component">
              <component :is="Component" />
            </template>
            <section
              v-else
              class="route-error"
              role="alert"
            >
              <ShieldAlert :size="28" />
              <h1>页面组件暂时不可用</h1>
              <p>这个页面没有留下空白状态。返回训练台后可以继续使用本地资料库。</p>
              <button
                type="button"
                @click="router.push({ name: 'dashboard' })"
              >
                回到训练台
              </button>
            </section>
            <template #fallback>
              <div
                class="route-loading"
                role="status"
                aria-live="polite"
              >
                正在打开页面…
              </div>
            </template>
          </Suspense>
        </div>
      </Transition>
    </RouterView>
  </AppShell>
  <StartupUpdateDialog
    v-if="startupUpdateReport?.available && startupUpdateReport.version"
    :report="startupUpdateReport"
    :installing="startupUpdateInstalling"
    :message="startupUpdateMessage"
    :publication-label="startupUpdatePublicationLabel"
    @dismiss="startupUpdate.dismiss"
    @install="startupUpdate.install"
  />
</template>

<style>
.route-page { min-width: 0; min-height: 100vh; }
.page-forward-enter-active, .page-backward-enter-active { transition: opacity 180ms var(--ease-standard), transform 180ms var(--ease-standard); }
.page-forward-leave-active, .page-backward-leave-active { transition: opacity 100ms var(--ease-standard), transform 100ms var(--ease-standard); }
.page-forward-enter-from { opacity: 0; transform: translate3d(14px,0,0); }
.page-forward-leave-to { opacity: 0; transform: translate3d(-6px,0,0); }
.page-backward-enter-from { opacity: 0; transform: translate3d(-14px,0,0); }
.page-backward-leave-to { opacity: 0; transform: translate3d(6px,0,0); }
@media (prefers-reduced-motion: reduce) {
  .page-forward-enter-active, .page-forward-leave-active, .page-backward-enter-active, .page-backward-leave-active { transition: none; }
  .page-forward-enter-from, .page-forward-leave-to, .page-backward-enter-from, .page-backward-leave-to { opacity: 1; transform: none; }
}
.global-notice-stack { position: fixed; z-index: 60; top: 20px; right: 24px; display: grid; gap: 10px; width: min(440px,calc(100vw - 48px)); max-height: calc(100vh - 40px); overflow: auto; overscroll-behavior: contain; }
.restore-notice { position: relative; display: grid; grid-template-columns: auto minmax(0,1fr) auto; gap: 11px; align-items: center; padding: 14px 15px; color: #fffdf7; border: 1px solid rgba(255,255,255,.28); border-radius: 15px; background: #365446; box-shadow: 0 18px 46px rgba(26,38,33,.24); }
.restore-notice.warning { background: #874a38; }.restore-notice span { display: grid; gap: 3px; }.restore-notice strong { font-size: 13px; }.restore-notice small { color: rgba(255,253,247,.82); font-size: 12px; line-height: 1.5; }.restore-notice button { display: grid; width: 44px; height: 44px; padding: 0; place-items: center; color: inherit; border: 0; border-radius: 50%; background: rgba(255,255,255,.1); cursor: pointer; }
.restore-notice-enter-active,.restore-notice-leave-active { transition: opacity var(--motion-standard) var(--ease-standard), transform var(--motion-page) var(--ease-standard); }.restore-notice-enter-from,.restore-notice-leave-to { opacity: 0; transform: translateY(-10px) scale(.98); }
@media (max-width: 760px) { .global-notice-stack { top: 12px; right: 12px; width: calc(100vw - 24px); max-height: calc(100vh - 24px); } }
@media (prefers-reduced-motion: reduce) { .restore-notice-enter-active,.restore-notice-leave-active { transition: none; } }
.route-page:focus,.route-page h1[tabindex="-1"]:focus { outline: none; }
.route-loading { display: grid; min-height: 50vh; place-items: center; color: var(--ink-muted); font-size: 14px; }
.route-error { display: grid; min-height: 50vh; padding: 52px 24px; place-items: center; align-content: center; gap: 10px; color: var(--ink-muted); text-align: center; }.route-error svg { color: var(--cinnabar); }.route-error h1 { margin: 0; color: var(--ink); font-family: var(--font-serif); font-size: 28px; }.route-error p { max-width: 460px; margin: 0; line-height: 1.7; }.route-error button { min-height: 44px; padding: 0 18px; color: var(--paper); border: 0; border-radius: 999px; background: var(--green-deep); font-weight: 740; cursor: pointer; }.route-error>button,.route-error-actions { margin-top: 8px; }.route-error-actions { display: flex; gap: 9px; flex-wrap: wrap; justify-content: center; }.route-error-actions .secondary { color: var(--green-deep); border: 1px solid rgba(33,51,45,.22); background: var(--paper); }
@media (prefers-reduced-motion: reduce) { .route-error button { transition: none; } }
</style>
