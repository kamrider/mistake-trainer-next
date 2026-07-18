<script setup lang="ts">
import { isTauri } from '@tauri-apps/api/core'
import { computed, onMounted, ref } from 'vue'
import { RouterView, useRoute, useRouter } from 'vue-router'
import type { AppResult } from '../shared/api/app-result'
import { commands, type ProfileOverview, type ProfileSummary, type SystemStatus } from '../shared/api/bindings'
import { normalizeAppResult } from '../shared/api/normalize-result'
import { loadSystemStatus, systemStatusLabel } from '../shared/api/system-status'
import AppShell, { type AppPage } from './AppShell.vue'

const route = useRoute()
const router = useRouter()
const activePage = computed(() => (route.name ?? 'dashboard') as AppPage)
const systemStatus = ref<AppResult<SystemStatus>>()
const statusLabel = computed(() => systemStatusLabel(systemStatus.value))
const profiles = ref<ProfileSummary[]>([])
const activeProfileId = ref('')
const profileBusy = ref(false)
const profileError = ref('')
const profileEpoch = ref(0)
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
})

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
    <RouterView v-slot="{ Component }">
      <Transition
        name="page"
        mode="out-in"
      >
        <div
          :key="`${route.fullPath}:${profileEpoch}`"
          class="route-page"
        >
          <component :is="Component" />
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
</style>
