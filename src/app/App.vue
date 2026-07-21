<script setup lang="ts">
import { isTauri } from '@tauri-apps/api/core'
import { CheckCircle2, ShieldAlert, X } from '@lucide/vue'
import { computed, onMounted, ref } from 'vue'
import { RouterView, useRoute, useRouter } from 'vue-router'
import type { AppResult } from '../shared/api/app-result'
import { commands, type BackupRestoreReceipt, type ProfileOverview, type ProfileSummary, type SystemStatus } from '../shared/api/bindings'
import { normalizeAppResult } from '../shared/api/normalize-result'
import { loadSystemStatus, systemStatusLabel } from '../shared/api/system-status'
import AppShell, { type AppPage } from './AppShell.vue'

const route = useRoute()
const router = useRouter()
const activePage = computed(() => (route.meta.shellPage ?? route.name ?? 'dashboard') as AppPage)
const systemStatus = ref<AppResult<SystemStatus>>()
const statusLabel = computed(() => systemStatusLabel(systemStatus.value))
const profiles = ref<ProfileSummary[]>([])
const activeProfileId = ref('')
const profileBusy = ref(false)
const profileError = ref('')
const profileEpoch = ref(0)
const restoreNotice = ref<BackupRestoreReceipt>()
const desktopRuntime = isTauri()
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

onMounted(async () => {
  systemStatus.value = await loadSystemStatus()
  await loadProfiles()
  await loadRestoreReceipt()
})

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

function selectProfile(profileId: string) {
  if (profileId === activeProfileId.value) return
  return mutateProfile(() => commands.profileSelect(profileId), true)
}
</script>

<template>
  <AppShell
    :profiles="shellProfiles"
    :active-profile-id="shellActiveProfileId"
    :profile-busy="profileBusy"
    :profile-error="profileError"
    :active-page="activePage"
    :system-status="statusLabel"
    @navigate="router.push({ name: $event })"
    @profile-select="selectProfile"
    @profile-create="createProfile"
    @profile-rename="renameProfile"
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
        name="page"
        mode="out-in"
      >
        <div
          :key="`${route.fullPath}:${profileEpoch}`"
          class="route-page"
        >
          <Suspense timeout="0">
            <component :is="Component" />
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
.page-enter-active, .page-leave-active { transition: opacity var(--motion-page) var(--ease-standard), transform var(--motion-page) var(--ease-standard); }
.page-enter-from { opacity: 0; transform: translateY(8px); }
.page-leave-to { opacity: 0; transform: translateY(-4px); }
.restore-notice { position: fixed; z-index: 60; top: 20px; right: 24px; display: grid; grid-template-columns: auto minmax(0,1fr) auto; gap: 11px; align-items: center; width: min(440px,calc(100vw - 48px)); padding: 14px 15px; color: #fffdf7; border: 1px solid rgba(255,255,255,.28); border-radius: 15px; background: #365446; box-shadow: 0 18px 46px rgba(26,38,33,.24); }
.restore-notice.warning { background: #874a38; }.restore-notice span { display: grid; gap: 3px; }.restore-notice strong { font-size: 13px; }.restore-notice small { color: rgba(255,253,247,.82); font-size: 11px; line-height: 1.5; }.restore-notice button { display: grid; width: 30px; height: 30px; padding: 0; place-items: center; color: inherit; border: 0; border-radius: 50%; background: rgba(255,255,255,.1); cursor: pointer; }
.restore-notice-enter-active,.restore-notice-leave-active { transition: opacity var(--motion-standard) var(--ease-standard), transform var(--motion-page) var(--ease-standard); }.restore-notice-enter-from,.restore-notice-leave-to { opacity: 0; transform: translateY(-10px) scale(.98); }
@media (max-width: 760px) { .restore-notice { top: 12px; right: 12px; width: calc(100vw - 24px); } }
@media (prefers-reduced-motion: reduce) { .restore-notice-enter-active,.restore-notice-leave-active { transition: none; } }
.route-loading { display: grid; min-height: 50vh; place-items: center; color: var(--ink-muted); font-size: 14px; }
</style>
