<script setup lang="ts">
import { computed } from 'vue'
import {
  BookOpen,
  ChartNoAxesColumnIncreasing,
  Cloud,
  Inbox,
  LayoutDashboard,
  Settings,
  SquareStack,
} from '@lucide/vue'
import ProfileSwitcher from '../modules/profiles/components/ProfileSwitcher.vue'
import type { ProfileSummary } from '../shared/api/bindings'
import type { SyncStatusCopy } from './sync-controller'

export type AppPage = 'dashboard' | 'inbox' | 'library' | 'review' | 'report' | 'settings'

const props = defineProps<{
  profiles: ProfileSummary[]
  activeProfileId: string
  profileBusy: boolean
  profileError: string
  activePage: AppPage
  syncStatus: SyncStatusCopy
}>()

const emit = defineEmits<{
  navigate: [page: AppPage]
  profileSelect: [profileId: string]
  profileCreate: [name: string]
  profileRename: [profileId: string, name: string]
  profileDelete: [profileId: string, confirmationName: string]
  profileRetry: []
}>()

function emitProfileRename(profileId: string, name: string) {
  emit('profileRename', profileId, name)
}

function emitProfileDelete(profileId: string, confirmationName: string) {
  emit('profileDelete', profileId, confirmationName)
}

const navigation = [
  { id: 'dashboard', label: '训练台', icon: LayoutDashboard },
  { id: 'inbox', label: '采集整理', icon: Inbox },
  { id: 'library', label: '题库', icon: SquareStack },
  { id: 'review', label: '训练室', icon: BookOpen },
  { id: 'report', label: '学习报告', icon: ChartNoAxesColumnIncreasing },
  { id: 'settings', label: '设置', icon: Settings },
] as const

const activeNavigationIndex = computed(() =>
  Math.max(0, navigation.findIndex(item => item.id === props.activePage)),
)
</script>

<template>
  <div class="app-frame">
    <aside class="side-rail">
      <div
        class="brand"
        aria-label="Mistake Trainer Next"
      >
        <span class="brand-mark">错</span>
        <span><strong>Mistake</strong><small>Trainer Next</small></span>
      </div>

      <nav
        aria-label="主导航"
        :style="{
          '--active-index': activeNavigationIndex,
          '--active-y': `${activeNavigationIndex * 49}px`,
          '--active-x': `${activeNavigationIndex * 100}%`,
        }"
      >
        <span
          class="nav-indicator"
          aria-hidden="true"
        />
        <button
          v-for="item in navigation"
          :key="item.id"
          type="button"
          :class="['nav-item', { active: activePage === item.id }]"
          :aria-label="item.label"
          :aria-current="activePage === item.id ? 'page' : undefined"
          @click="$emit('navigate', item.id)"
        >
          <component
            :is="item.icon"
            :size="19"
            aria-hidden="true"
          />
          <span>{{ item.label }}</span>
        </button>
      </nav>

      <div class="rail-footer">
        <div
          class="sync-state"
          role="status"
          aria-live="polite"
          :data-tone="syncStatus.tone"
        >
          <Cloud
            :size="15"
            aria-hidden="true"
          />
          <span>{{ syncStatus.label }}</span>
        </div>
        <ProfileSwitcher
          :profiles="profiles"
          :active-profile-id="activeProfileId"
          :busy="profileBusy"
          :error-message="profileError"
          @select="$emit('profileSelect', $event)"
          @create="$emit('profileCreate', $event)"
          @rename="emitProfileRename"
          @delete="emitProfileDelete"
          @retry="$emit('profileRetry')"
        />
      </div>
    </aside>
    <section class="app-content">
      <slot />
    </section>
  </div>
</template>

<style scoped>
.app-frame { display: grid; grid-template-columns: 228px minmax(0,1fr); min-height: 100vh; }
.side-rail { position: sticky; top: 0; display: flex; height: 100vh; flex-direction: column; padding: 28px 18px 20px; border-right: 1px solid var(--line); background: rgba(246,241,231,.86); backdrop-filter: blur(18px); }
.brand { display: flex; gap: 11px; align-items: center; padding: 0 9px 26px; }
.brand-mark { display: grid; width: 34px; height: 34px; place-items: center; color: var(--white); border-radius: 9px 9px 9px 3px; background: var(--cinnabar); font-family: Georgia, serif; font-weight: 700; }
.brand strong, .brand small { display: block; }
.brand strong { font-family: Georgia, serif; font-size: 17px; letter-spacing: .02em; }
.brand small { margin-top: 1px; color: var(--ink-muted); font-size: 12px; letter-spacing: .13em; text-transform: uppercase; }
nav { position: relative; display: grid; gap: 5px; isolation: isolate; }
.nav-indicator { position: absolute; z-index: -1; top: 0; left: 0; width: 100%; height: 44px; border: 1px solid rgba(33,51,45,.08); border-radius: 11px; background: var(--green-soft); box-shadow: 0 8px 22px rgba(33,51,45,.06); pointer-events: none; transform: translate3d(0,var(--active-y),0); transition: transform var(--motion-page) var(--ease-standard), border-radius var(--motion-standard) var(--ease-standard); will-change: transform; }
.nav-item { position: relative; z-index: 1; display: flex; gap: 12px; align-items: center; width: 100%; min-height: 44px; padding: 0 13px; color: var(--ink-muted); border: 0; border-radius: 11px; background: transparent; cursor: pointer; text-align: left; transition: color var(--motion-standard) var(--ease-standard), background var(--motion-standard) var(--ease-standard), transform var(--motion-feedback) var(--ease-standard); }
.nav-item:hover { color: var(--ink); background: rgba(232,221,199,.52); }
.nav-item:active { transform: scale(.985); }
.nav-item.active { color: var(--green-deep); background: transparent; font-weight: 700; }
.nav-item svg { transition: transform var(--motion-standard) var(--ease-standard); }
.nav-item.active svg { transform: scale(1.06); }
.rail-footer { display: grid; gap: 12px; margin-top: auto; }
.sync-state { display: flex; gap: 7px; align-items: center; min-height: 19px; padding: 0 11px; color: var(--ink-muted); font-size: 12px; }
.sync-state svg { flex: 0 0 auto; color: #657f70; transition: color var(--motion-standard) var(--ease-standard), transform var(--motion-standard) var(--ease-standard), opacity var(--motion-standard) var(--ease-standard); }
.sync-state[data-tone="active"] svg { color: var(--cinnabar); transform: scale(1.08); }
.sync-state[data-tone="success"] { color: #557263; }
.sync-state[data-tone="success"] svg { color: #557263; }
.sync-state[data-tone="waiting"] { color: #8a653c; }
.sync-state[data-tone="waiting"] svg { color: #9a7143; }
.sync-state[data-tone="warning"] { color: #8d4635; }
.sync-state[data-tone="warning"] svg { color: var(--cinnabar); }
.app-content { min-width: 0; }
@media (max-width: 760px) { .app-frame { grid-template-columns: 1fr; padding-bottom: 68px; } .side-rail { position: fixed; z-index: 20; top: auto; right: 0; bottom: 0; left: 0; height: 68px; padding: 8px 10px; border-top: 1px solid var(--line); border-right: 0; } .brand, .rail-footer { display: none; } nav { display: grid; grid-template-columns: repeat(6,1fr); } .nav-indicator { width: calc(100% / 6); height: 50px; transform: translate3d(var(--active-x),0,0); } .nav-item { justify-content: center; min-height: 50px; padding: 0; } .nav-item span { display: none; } }
@media (prefers-reduced-motion: reduce) { .nav-indicator, .nav-item, .nav-item svg, .sync-state svg { transition: none; } .sync-state[data-tone="active"] svg { transform: none; } }
</style>
