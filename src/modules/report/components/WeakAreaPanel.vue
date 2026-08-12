<script setup lang="ts">
import { ArrowRight, Target } from '@lucide/vue'
import type { WeakAreaSummary } from '../../../shared/api/bindings'

defineProps<{ areas: WeakAreaSummary[] }>()

function percent(value: number | null) {
  return `${Math.round((value ?? 0) * 100)}%`
}

function duration(value: number | null) {
  const seconds = Math.round((value ?? 0) / 1_000)
  return seconds >= 60 ? `${Math.round(seconds / 60)} 分钟` : `${seconds} 秒`
}

function libraryHref(area: WeakAreaSummary) {
  const key = area.kind === 'reason' ? 'tag' : 'subject'
  return `#/library?${key}=${encodeURIComponent(area.label)}`
}
</script>

<template>
  <article class="weak-panel">
    <header>
      <div>
        <p>近 30 天真实评分</p>
        <h2>本周最值得修正</h2>
      </div>
      <Target
        :size="22"
        aria-hidden="true"
      />
    </header>
    <ol
      v-if="areas.length"
      aria-label="薄弱点排序"
    >
      <li
        v-for="area in areas"
        :key="`${area.kind}-${area.label}`"
      >
        <div class="rank-copy">
          <span>{{ area.kind === 'reason' ? '错因' : '科目' }}</span>
          <strong>{{ area.kind === 'reason' ? area.label.replace(/^错因·/u, '') : area.label }}</strong>
          <small>{{ area.reviewedCount }} 次复习 · 平均 {{ duration(area.averageDurationMs) }}</small>
        </div>
        <div class="rate">
          <strong>{{ percent(area.lapseRate) }}</strong>
          <small>{{ area.lapseCount }} 次需要重来</small>
        </div>
        <a
          :href="libraryHref(area)"
          :aria-label="`筛选题库中的${area.kind === 'reason' ? '错因' : '科目'} ${area.label}`"
        >
          <ArrowRight
            :size="16"
            aria-hidden="true"
          />
        </a>
      </li>
    </ol>
    <p
      v-else
      class="sparse-copy"
    >
      还没有足够证据判断薄弱点。每个科目或错因至少完成 2 次真实评分后，这里才会给出排序。
    </p>
  </article>
</template>

<style scoped>
.weak-panel { padding: 26px; border: 1px solid var(--line); border-radius: var(--radius-md); background: rgba(255,253,247,.72); }
header { display: flex; justify-content: space-between; gap: 20px; align-items: center; }
header p { margin: 0 0 5px; color: var(--cinnabar); font-size: 11px; font-weight: 760; letter-spacing: .08em; }
h2 { margin: 0; font-size: 25px; }
header > svg { color: var(--cinnabar); }
ol { display: grid; gap: 4px; margin: 20px 0 0; padding: 0; list-style: none; }
li { display: grid; grid-template-columns: 1fr auto 42px; gap: 16px; align-items: center; padding: 12px 0; border-top: 1px solid var(--line); }
.rank-copy { display: grid; gap: 3px; }
.rank-copy > span { color: var(--ink-muted); font-size: 11px; }
.rank-copy small, .rate small { color: var(--ink-muted); font-size: 12px; }
.rate { display: grid; gap: 3px; text-align: right; }
.rate > strong { color: var(--green-deep); font-size: 20px; }
a { display: grid; width: 40px; height: 40px; place-items: center; color: var(--green-deep); border: 1px solid var(--line); border-radius: 50%; }
.sparse-copy { margin: 20px 0 0; padding: 18px; color: var(--ink-muted); border-radius: 12px; background: var(--green-soft); line-height: 1.65; }
</style>
