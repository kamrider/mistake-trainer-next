<script setup lang="ts">
import { isTauri } from '@tauri-apps/api/core'
import { CheckCircle2, ShieldAlert, X } from '@lucide/vue'
import { computed, onErrorCaptured, onMounted, onUnmounted, provide, ref, watch } from 'vue'
import { RouterView, useRoute, useRouter } from 'vue-router'
import { failure, type AppResult } from '../shared/api/app-result'
import { commands, type BackupRestoreReceipt, type ProfileOverview, type ProfileSummary, type SyncNowReport, type SystemStatus } from '../shared/api/bindings'
import { normalizeAppResult } from '../shared/api/normalize-result'
import { loadSystemStatus } from '../shared/api/system-status'
import AppShell, { type AppPage } from './AppShell.vue'
import LibraryAccessScreen from './LibraryAccessScreen.vue'
import { libraryAccessControllerKey } from './library-access-controller'
import { createSyncController, syncControllerKey, syncStatusCopy, type SyncPhase, type SyncTrigger } from './sync-controller'

const route = useRoute()
const router = useRouter()
const desktopRuntime = isTauri()
type LibraryAccessPhase = 'checking' | 'unlocked' | 'locked' | 'error' | 'unlocking' | 'restarting'
type LibraryAccessErrorReason = 'credentials' | 'storage'
const libraryAccessPhase = ref<LibraryAccessPhase>(desktopRuntime ? 'checking' : 'unlocked')
const libraryAccessError = ref('')
const libraryAccessErrorReason = ref<LibraryAccessErrorReason>('credentials')
let workspaceInitialized = false
provide(libraryAccessControllerKey, {
  enterRestarting: () => {
    libraryAccessPhase.value = 'restarting'
  },
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
const syncPhase = ref<SyncPhase>(desktopRuntime ? 'idle' : 'local_only')
const automaticSyncCooldownMs = 15_000
let lastSuccessfulSyncAtUtcMs = 0
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
const profiles = ref<ProfileSummary[]>([])
const activeProfileId = ref('')
const profileBusy = ref(false)
const profileError = ref('')
const profileEpoch = ref(0)
const restoreNotice = ref<BackupRestoreReceipt>()
const routeError = ref('')
const routeErrorDetail = ref('')
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
const previewProfile: ProfileSummary = {
  id: 'preview-profile',
  name: '本机学习档案',
  createdAtUtcMs: 0,
  updatedAtUtcMs: 0,
  revision: 1,
}
const shellProfiles = computed(() =>
  profiles.value.length || desktopRuntime ? profiles.value : [previewProfile],
)
const shellActiveProfileId = computed(() =>
  activeProfileId.value || (desktopRuntime ? '' : previewProfile.id),
)

watch(() => route.fullPath, () => {
  routeError.value = ''
  routeErrorDetail.value = ''
})

watch(activePage, (next, previous) => {
  if (next === previous) return
  pageDirection.value = (pageOrder[next] ?? 0) >= (pageOrder[previous] ?? 0)
    ? 'forward'
    : 'backward'
})

onErrorCaptured((error, _instance, info) => {
  routeError.value = '这个页面暂时打不开'
  routeErrorDetail.value = desktopRuntime
    ? '本地资料库没有被修改。返回训练台后可以重新打开这个页面。'
    : `浏览器预览遇到异常（${info}），请重新打开页面。`
  if (import.meta.env.DEV) console.error('route render error', error)
  return false
})

onMounted(() => {
  window.addEventListener('online', handleOnline)
  document.addEventListener('visibilitychange', handleVisibilityChange)
  void loadLibraryAccess()
})

onUnmounted(() => {
  window.removeEventListener('online', handleOnline)
  document.removeEventListener('visibilitychange', handleVisibilityChange)
  syncController.dispose()
})

async function initializeWorkspace() {
  if (workspaceInitialized) return
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
  workspaceInitialized = true
  void restoreCloudAndSync('startup')
}

function handleOnline() {
  if (libraryAccessPhase.value !== 'unlocked' || !workspaceInitialized) return
  void restoreCloudAndSync('online')
}

function handleVisibilityChange() {
  if (
    document.visibilityState !== 'visible'
    || libraryAccessPhase.value !== 'unlocked'
    || !workspaceInitialized
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

async function restoreCloudAndSync(reason: SyncTrigger) {
  if (!desktopRuntime) {
    syncPhase.value = 'local_only'
    return
  }
  if (
    reason === 'visible'
    && lastSuccessfulSyncAtUtcMs > 0
    && Date.now() - lastSuccessfulSyncAtUtcMs < automaticSyncCooldownMs
  ) {
    return
  }
  if (typeof navigator !== 'undefined' && navigator.onLine === false) {
    syncPhase.value = 'offline'
    return
  }

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

async function performSync(): Promise<AppResult<SyncNowReport>> {
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
    profileEpoch.value += 1
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

async function loadLibraryAccess() {
  if (!desktopRuntime) {
    libraryAccessPhase.value = 'unlocked'
    await initializeWorkspace()
    return
  }

  libraryAccessPhase.value = 'checking'
  libraryAccessError.value = ''
  libraryAccessErrorReason.value = 'credentials'
  try {
    const result = normalizeAppResult(await commands.libraryAccessStatus())
    if (!result.ok) {
      libraryAccessPhase.value = 'error'
      libraryAccessError.value = result.error.userMessage
      libraryAccessErrorReason.value = result.error.code === 'LIBRARY_STORAGE_UNAVAILABLE'
        ? 'storage'
        : 'credentials'
      return
    }
    if (result.data.locked) {
      libraryAccessPhase.value = 'locked'
      return
    }
    libraryAccessPhase.value = 'unlocked'
    await initializeWorkspace()
  } catch {
    libraryAccessPhase.value = 'error'
    libraryAccessError.value = 'Windows 凭据管理器没有响应，请重新检查或使用当前账户解锁。'
  }
}

async function unlockLibrary() {
  if (!desktopRuntime || libraryAccessPhase.value === 'unlocking' || libraryAccessPhase.value === 'restarting') return
  libraryAccessPhase.value = 'unlocking'
  libraryAccessError.value = ''
  try {
    const result = normalizeAppResult(await commands.libraryUnlock())
    if (!result.ok) {
      libraryAccessPhase.value = 'error'
      libraryAccessError.value = result.error.userMessage
      return
    }
    libraryAccessPhase.value = 'restarting'
  } catch {
    libraryAccessPhase.value = 'error'
    libraryAccessError.value = '当前 Windows 账户未能完成解锁，请稍后再试。'
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

async function loadProfiles() {
  if (!desktopRuntime) return
  profileBusy.value = true
  profileError.value = ''
  try {
    const result = normalizeAppResult(await commands.profileList())
    if (!result.ok) {
      profileError.value = result.error.userMessage
      return
    }
    applyOverview(result.data)
  } catch {
    profileError.value = '学习档案没有读取成功，请重新打开应用后重试。'
  } finally {
    profileBusy.value = false
  }
}

function applyOverview(overview: ProfileOverview) {
  profiles.value = overview.profiles
  activeProfileId.value = overview.activeProfileId
}

async function mutateProfile(
  invoke: () => ReturnType<typeof commands.profileList>,
  refreshWorkspace: boolean,
) {
  if (!desktopRuntime || profileBusy.value) return
  profileBusy.value = true
  profileError.value = ''
  try {
    const result = normalizeAppResult(await invoke())
    if (!result.ok) {
      profileError.value = result.error.userMessage
      return
    }
    applyOverview(result.data)
    if (refreshWorkspace) {
      profileEpoch.value += 1
      await router.push({ name: 'dashboard' })
    }
  } catch {
    profileError.value = '学习档案没有完成这次操作，请稍后重试。'
  } finally {
    profileBusy.value = false
  }
}

function createProfile(name: string) {
  return mutateProfile(() => commands.profileCreate({ name }), true)
}

function renameProfile(profileId: string, name: string) {
  return mutateProfile(() => commands.profileRename({ profileId, name }), false)
}

function deleteProfile(profileId: string, confirmationName: string) {
  return mutateProfile(() => commands.profileDelete({ profileId, confirmationName }), true)
}

function selectProfile(profileId: string) {
  if (profileId === activeProfileId.value) return
  return mutateProfile(() => commands.profileSelect(profileId), true)
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
    <RouterView v-slot="{ Component }">
      <Transition
        :name="pageTransitionName"
        mode="out-in"
      >
        <div
          :key="`${route.fullPath}:${profileEpoch}`"
          class="route-page"
          :data-direction="pageDirection"
        >
          <section
            v-if="routeError"
            class="route-error"
            role="alert"
          >
            <ShieldAlert :size="28" />
            <h1>{{ routeError }}</h1>
            <p>{{ routeErrorDetail }}</p>
            <button
              type="button"
              @click="router.push({ name: 'dashboard' })"
            >
              回到训练台
            </button>
          </section>
          <Suspense
            v-else
            timeout="0"
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
.restore-notice { position: fixed; z-index: 60; top: 20px; right: 24px; display: grid; grid-template-columns: auto minmax(0,1fr) auto; gap: 11px; align-items: center; width: min(440px,calc(100vw - 48px)); padding: 14px 15px; color: #fffdf7; border: 1px solid rgba(255,255,255,.28); border-radius: 15px; background: #365446; box-shadow: 0 18px 46px rgba(26,38,33,.24); }
.restore-notice.warning { background: #874a38; }.restore-notice span { display: grid; gap: 3px; }.restore-notice strong { font-size: 13px; }.restore-notice small { color: rgba(255,253,247,.82); font-size: 11px; line-height: 1.5; }.restore-notice button { display: grid; width: 30px; height: 30px; padding: 0; place-items: center; color: inherit; border: 0; border-radius: 50%; background: rgba(255,255,255,.1); cursor: pointer; }
.restore-notice-enter-active,.restore-notice-leave-active { transition: opacity var(--motion-standard) var(--ease-standard), transform var(--motion-page) var(--ease-standard); }.restore-notice-enter-from,.restore-notice-leave-to { opacity: 0; transform: translateY(-10px) scale(.98); }
@media (max-width: 760px) { .restore-notice { top: 12px; right: 12px; width: calc(100vw - 24px); } }
@media (prefers-reduced-motion: reduce) { .restore-notice-enter-active,.restore-notice-leave-active { transition: none; } }
.route-loading { display: grid; min-height: 50vh; place-items: center; color: var(--ink-muted); font-size: 14px; }
.route-error { display: grid; min-height: 50vh; padding: 52px 24px; place-items: center; align-content: center; gap: 10px; color: var(--ink-muted); text-align: center; }.route-error svg { color: var(--cinnabar); }.route-error h1 { margin: 0; color: var(--ink); font-family: var(--font-serif); font-size: 28px; }.route-error p { max-width: 420px; margin: 0; line-height: 1.7; }.route-error button { min-height: 42px; margin-top: 8px; padding: 0 18px; color: var(--paper); border: 0; border-radius: 999px; background: var(--green-deep); cursor: pointer; }
@media (prefers-reduced-motion: reduce) { .route-error button { transition: none; } }
</style>
