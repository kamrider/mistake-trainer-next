<script setup lang="ts">
import { CalendarDays } from '@lucide/vue'
import type { DueForecastDay } from '../../../shared/api/bindings'

defineProps<{ days: DueForecastDay[] }>()

function dateLabel(value: string, index: number) {
  if (index === 0) return '今天'
  return new Date(`${value}T00:00:00`).toLocaleDateString('zh-CN', { month: 'numeric', day: 'numeric', weekday: 'short' })
}
</script>

<template>
  <article class="forecast-panel">
    <header>
      <div><p>按本地日期</p><h2>未来七天任务</h2></div>
      <CalendarDays
        :size="22"
        aria-hidden="true"
      />
    </header>
    <ol
      v-if="days.length"
      aria-label="未来七天到期题预测"
    >
      <li
        v-for="(day, index) in days"
        :key="day.localDate"
      >
        <span>{{ dateLabel(day.localDate, index) }}</span>
        <i aria-hidden="true"><b :style="{ width: `${Math.max(5, day.dueCount / Math.max(1, ...days.map(item => item.dueCount)) * 100)}%` }" /></i>
        <strong>{{ day.dueCount }} 题</strong>
        <small v-if="index === 0 && day.overdueCount > 0">另有 {{ day.overdueCount }} 题已到复习时间</small>
      </li>
    </ol>
    <p
      v-else
      class="sparse-copy"
    >
      暂时没有可预测的到期任务。
    </p>
  </article>
</template>

<style scoped>
.forecast-panel { padding: 26px; border: 1px solid var(--line); border-radius: var(--radius-md); background: rgba(255,253,247,.72); }
header { display: flex; justify-content: space-between; gap: 20px; align-items: center; }
header p { margin: 0 0 5px; color: var(--cinnabar); font-size: 11px; font-weight: 760; letter-spacing: .08em; }
h2 { margin: 0; font-size: 25px; }
header > svg { color: var(--cinnabar); }
ol { display: grid; gap: 9px; margin: 20px 0 0; padding: 0; list-style: none; }
li { display: grid; grid-template-columns: 64px 1fr 46px; gap: 10px; align-items: center; min-height: 29px; }
li > span, li > strong { font-size: 12px; }
li > strong { text-align: right; }
li > i { height: 7px; overflow: hidden; border-radius: 999px; background: var(--sand); }
li > i > b { display: block; height: 100%; border-radius: inherit; background: var(--green-deep); }
li > small { grid-column: 1 / -1; color: #725f3d; font-size: 11px; }
.sparse-copy { color: var(--ink-muted); }
</style>
