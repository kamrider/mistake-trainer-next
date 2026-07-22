<script setup lang="ts">
import { isTauri } from '@tauri-apps/api/core'
import { CheckCircle2, ShieldAlert, X } from '@lucide/vue'
import { computed, onErrorCaptured, onMounted, ref, watch } from 'vue'
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
const routeError = ref('')
const routeErrorDetail = ref('')
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

watch(() => route.fullPath, () => {
  routeError.value = ''
  routeErrorDetail.value = ''
})

onErrorCaptured((error, _instance, info) => {
  routeError.value = '这个页面暂时打不开'
  routeErrorDetail.value = desktopRuntime
    ? '本地资料库没有被修改。返回训练台后可以重新打开这个页面。'
    : `浏览器预览遇到异常（${info}），请重新打开页面。`
  if (import.meta.env.DEV) console.error('route render error', error)
  return false
})

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

function deleteProfile(profileId: string, confirmationName: string) {
  return mutateProfile(() => commands.profileDelete({ profileId, confirmationName }), true)
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
        name="page"
        mode="out-in"
      >
        <div
          :key="`${route.fullPath}:${profileEpoch}`"
          class="route-page"
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
.page-enter-active, .page-leave-active { transition: opacity var(--motion-page) var(--ease-standard), transform var(--motion-page) var(--ease-standard); }
.page-enter-from { opacity: 0; transform: translateY(8px); }
.page-leave-to { opacity: 0; transform: translateY(-4px); }
.restore-notice { position: fixed; z-index: 60; top: 20px; right: 24px; display: grid; grid-template-columns: auto minmax(0,1fr) auto; gap: 11px; align-items: center; width: min(440px,calc(100vw - 48px)); padding: 14px 15px; color: #fffdf7; border: 1px solid rgba(255,255,255,.28); border-radius: 15px; background: #365446; box-shadow: 0 18px 46px rgba(26,38,33,.24); }
.restore-notice.warning { background: #874a38; }.restore-notice span { display: grid; gap: 3px; }.restore-notice strong { font-size: 13px; }.restore-notice small { color: rgba(255,253,247,.82); font-size: 11px; line-height: 1.5; }.restore-notice button { display: grid; width: 30px; height: 30px; padding: 0; place-items: center; color: inherit; border: 0; border-radius: 50%; background: rgba(255,255,255,.1); cursor: pointer; }
.restore-notice-enter-active,.restore-notice-leave-active { transition: opacity var(--motion-standard) var(--ease-standard), transform var(--motion-page) var(--ease-standard); }.restore-notice-enter-from,.restore-notice-leave-to { opacity: 0; transform: translateY(-10px) scale(.98); }
@media (max-width: 760px) { .restore-notice { top: 12px; right: 12px; width: calc(100vw - 24px); } }
@media (prefers-reduced-motion: reduce) { .restore-notice-enter-active,.restore-notice-leave-active { transition: none; } }
.route-loading { display: grid; min-height: 50vh; place-items: center; color: var(--ink-muted); font-size: 14px; }
.route-error { display: grid; min-height: 50vh; padding: 52px 24px; place-items: center; align-content: center; gap: 10px; color: var(--ink-muted); text-align: center; }.route-error svg { color: var(--cinnabar); }.route-error h1 { margin: 0; color: var(--ink); font-family: var(--font-serif); font-size: 28px; }.route-error p { max-width: 420px; margin: 0; line-height: 1.7; }.route-error button { min-height: 42px; margin-top: 8px; padding: 0 18px; color: var(--paper); border: 0; border-radius: 999px; background: var(--green-deep); cursor: pointer; }
@media (prefers-reduced-motion: reduce) { .route-error button { transition: none; } }
</style>
