<script setup lang="ts">
import {
  BookOpen,
  ChartNoAxesColumnIncreasing,
  CircleUserRound,
  Cloud,
  Inbox,
  LayoutDashboard,
  Settings,
  SquareStack,
} from '@lucide/vue'

export type AppPage = 'dashboard' | 'inbox' | 'library' | 'review' | 'report' | 'settings'

defineProps<{
  learnerName: string
  activePage: AppPage
  systemStatus: string
}>()

defineEmits<{
  navigate: [page: AppPage]
}>()

const navigation = [
  { id: 'dashboard', label: '训练台', icon: LayoutDashboard },
  { id: 'inbox', label: '采集整理', icon: Inbox },
  { id: 'library', label: '题库', icon: SquareStack },
  { id: 'review', label: '训练室', icon: BookOpen },
  { id: 'report', label: '学习报告', icon: ChartNoAxesColumnIncreasing },
  { id: 'settings', label: '设置', icon: Settings },
] as const
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

      <nav aria-label="主导航">
        <button
          v-for="item in navigation"
          :key="item.id"
          type="button"
          :class="['nav-item', { active: activePage === item.id }]"
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
          aria-live="polite"
        >
          <Cloud
            :size="15"
            aria-hidden="true"
          /><span>{{ systemStatus }}</span>
        </div>
        <button
          class="profile-button"
          type="button"
          :aria-label="`当前学习档案：${learnerName}`"
        >
          <CircleUserRound
            :size="21"
            aria-hidden="true"
          />
          <span><strong>{{ learnerName }}</strong><small>私人学习档案</small></span>
        </button>
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
.brand small { margin-top: 1px; color: var(--ink-muted); font-size: 10px; letter-spacing: .13em; text-transform: uppercase; }
nav { display: grid; gap: 5px; }
.nav-item { display: flex; gap: 12px; align-items: center; width: 100%; min-height: 43px; padding: 0 13px; color: var(--ink-muted); border: 0; border-radius: 11px; background: transparent; cursor: pointer; text-align: left; transition: color var(--motion-standard) var(--ease-standard), background var(--motion-standard) var(--ease-standard), transform var(--motion-feedback) var(--ease-standard); }
.nav-item:hover { color: var(--ink); background: rgba(232,221,199,.52); }
.nav-item:active { transform: scale(.985); }
.nav-item.active { color: var(--green-deep); background: var(--green-soft); font-weight: 700; }
.rail-footer { display: grid; gap: 12px; margin-top: auto; }
.sync-state { display: flex; gap: 7px; align-items: center; padding: 0 11px; color: var(--ink-muted); font-size: 12px; }
.sync-state svg { color: #657f70; }
.profile-button { display: flex; gap: 10px; align-items: center; width: 100%; padding: 12px; border: 1px solid var(--line); border-radius: 13px; background: rgba(255,253,247,.66); cursor: pointer; text-align: left; }
.profile-button strong, .profile-button small { display: block; }
.profile-button strong { font-size: 13px; }
.profile-button small { margin-top: 3px; color: var(--ink-muted); font-size: 10px; }
.app-content { min-width: 0; }
@media (max-width: 760px) { .app-frame { grid-template-columns: 1fr; padding-bottom: 68px; } .side-rail { position: fixed; z-index: 20; top: auto; right: 0; bottom: 0; left: 0; height: 68px; padding: 8px 10px; border-top: 1px solid var(--line); border-right: 0; } .brand, .rail-footer { display: none; } nav { display: grid; grid-template-columns: repeat(6,1fr); } .nav-item { justify-content: center; min-height: 50px; padding: 0; } .nav-item span { display: none; } }
</style>
