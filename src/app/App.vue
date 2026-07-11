<script setup lang="ts">
import { computed, onMounted, ref } from 'vue'
import { RouterView, useRoute, useRouter } from 'vue-router'
import type { AppResult } from '../shared/api/app-result'
import type { SystemStatus } from '../shared/api/bindings'
import { loadSystemStatus, systemStatusLabel } from '../shared/api/system-status'
import AppShell, { type AppPage } from './AppShell.vue'

const route = useRoute()
const router = useRouter()
const activePage = computed(() => (route.name ?? 'dashboard') as AppPage)
const systemStatus = ref<AppResult<SystemStatus>>()
const statusLabel = computed(() => systemStatusLabel(systemStatus.value))

onMounted(async () => {
  systemStatus.value = await loadSystemStatus()
})
</script>

<template>
  <AppShell
    learner-name="小树"
    :active-page="activePage"
    :system-status="statusLabel"
    @navigate="router.push({ name: $event })"
  >
    <RouterView v-slot="{ Component }">
      <Transition
        name="page"
        mode="out-in"
      >
        <component :is="Component" />
      </Transition>
    </RouterView>
  </AppShell>
</template>

<style>
.page-enter-active, .page-leave-active { transition: opacity var(--motion-page) var(--ease-standard), transform var(--motion-page) var(--ease-standard); }
.page-enter-from { opacity: 0; transform: translateY(8px); }
.page-leave-to { opacity: 0; transform: translateY(-4px); }
</style>
