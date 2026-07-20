<script setup lang="ts">
import { ChevronRight, History } from '@lucide/vue'
import { computed } from 'vue'
import type { ReviewHistoryItem } from '@/shared/api/bindings'

const props = defineProps<{ items: ReviewHistoryItem[]; selectedId: string; nextCursor: string | null; loadingMore: boolean }>()
const emit = defineEmits<{ select: [eventId: string, trigger: HTMLElement]; more: [] }>()
const groups = computed(() => {
  const formatter = new Intl.DateTimeFormat('zh-CN', { year: 'numeric', month: 'long', day: 'numeric' })
  const grouped: Array<{ key: string; label: string; items: ReviewHistoryItem[] }> = []
  for (const item of props.items) {
    const date = new Date(item.occurredAtUtcMs ?? 0)
    const key = `${date.getFullYear()}-${date.getMonth()}-${date.getDate()}`
    let group = grouped.at(-1)
    if (!group || group.key !== key) {
      group = { key, label: formatter.format(date), items: [] }
      grouped.push(group)
    }
    group.items.push(item)
  }
  return grouped
})
const ratingCopy = { again: '忘记', hard: '困难', good: '记住', easy: '轻松' } as const
function durationLabel(value: number | null) { return value == null ? '—' : value < 60_000 ? `${Math.max(1, Math.round(value / 1000))} 秒` : `${Math.round(value / 60_000)} 分钟` }
function timeLabel(value: number | null) { return value == null ? '—' : new Date(value).toLocaleTimeString('zh-CN', { hour: '2-digit', minute: '2-digit' }) }
</script>

<template>
  <section
    class="history-timeline"
    aria-label="复习记录时间线"
  >
    <div
      v-for="group in groups"
      :key="group.key"
      class="day-group"
    >
      <h2>{{ group.label }}</h2><ol>
        <li
          v-for="item in group.items"
          :key="item.eventId"
        >
          <button
            type="button"
            :class="['history-row',{selected:selectedId===item.eventId}]"
            :aria-pressed="selectedId===item.eventId"
            :aria-label="`${timeLabel(item.occurredAtUtcMs)} ${item.subject} ${ratingCopy[item.rating]}，查看审计详情`"
            @click="emit('select',item.eventId,$event.currentTarget as HTMLElement)"
          >
            <span class="timeline-mark"><History
              :size="15"
              aria-hidden="true"
            /></span><span class="row-time">{{ timeLabel(item.occurredAtUtcMs) }}</span><span class="row-copy"><strong>{{ item.subject || '未分类' }}</strong><small>{{ item.notePreview || '未填写笔记' }}</small></span><span :class="['rating-seal',item.rating]">{{ ratingCopy[item.rating] }}</span><span class="duration">{{ durationLabel(item.durationMs) }}</span><ChevronRight
              :size="17"
              aria-hidden="true"
            />
          </button>
        </li>
      </ol>
    </div>
    <button
      v-if="nextCursor"
      class="more-button"
      type="button"
      :disabled="loadingMore"
      @click="emit('more')"
    >
      {{ loadingMore ? '正在读取…' : '加载更多' }}
    </button>
  </section>
</template>

<style scoped>
.history-timeline{min-width:0}.day-group+.day-group{margin-top:24px}.day-group h2{margin:0 0 10px 13px;color:var(--ink-muted);font-size:11px;font-weight:780;letter-spacing:.08em}.day-group ol{display:grid;gap:8px;margin:0;padding:0;list-style:none}.history-row{position:relative;display:grid;width:100%;min-height:76px;grid-template-columns:30px 56px minmax(120px,1fr) auto 62px 18px;gap:10px;align-items:center;padding:10px 14px;color:var(--ink);border:1px solid var(--line);border-radius:5px 16px 16px;background:rgba(255,253,247,.7);box-shadow:0 8px 24px rgba(34,48,43,.035);cursor:pointer;text-align:left;transition:opacity var(--motion-feedback) var(--ease-standard),transform var(--motion-feedback) var(--ease-standard),border-color var(--motion-standard) var(--ease-standard),box-shadow var(--motion-standard) var(--ease-standard);animation:row-in var(--motion-feedback) var(--ease-standard) both}.history-row:hover{transform:translateY(-2px);border-color:rgba(33,51,45,.28);box-shadow:0 12px 28px rgba(34,48,43,.075)}.history-row.selected{border-color:rgba(185,88,63,.48);background:#fff9ef;box-shadow:0 12px 30px rgba(185,88,63,.09)}.timeline-mark{display:grid;width:28px;height:28px;place-items:center;color:var(--green-deep);border-radius:50%;background:var(--green-soft)}.row-time{color:var(--ink-muted);font-family:var(--font-serif);font-size:13px}.row-copy{min-width:0}.row-copy strong,.row-copy small{display:block}.row-copy strong{font-size:13px}.row-copy small{margin-top:5px;overflow:hidden;color:var(--ink-muted);font-size:11px;text-overflow:ellipsis;white-space:nowrap}.rating-seal{display:grid;min-width:42px;min-height:30px;padding:0 8px;place-items:center;color:#fffdf7;border-radius:10px 3px 10px 10px;background:var(--green-deep);font-size:10px;font-weight:780;transform:rotate(-1deg)}.rating-seal.again{background:var(--cinnabar)}.rating-seal.hard{color:#77412f;background:#efd2b8}.rating-seal.easy{background:#557263}.duration{color:var(--ink-muted);font-size:10px;text-align:right}.more-button{display:flex;min-height:44px;margin:18px auto 0;padding:0 22px;align-items:center;justify-content:center;color:var(--green-deep);border:1px solid rgba(33,51,45,.22);border-radius:999px;background:var(--green-soft);cursor:pointer}.more-button:disabled{opacity:.55;cursor:not-allowed}@keyframes row-in{from{opacity:0;transform:translateY(5px)}}@media(max-width:620px){.history-row{grid-template-columns:44px minmax(0,1fr) auto 16px;min-height:82px}.timeline-mark{display:none}.row-time{grid-column:1}.row-copy{grid-column:2}.rating-seal{grid-column:3;grid-row:1}.duration{grid-column:3;grid-row:2}.history-row>svg{grid-column:4;grid-row:1/3}.row-copy small{white-space:normal;display:-webkit-box;-webkit-line-clamp:2;-webkit-box-orient:vertical}}@media(prefers-reduced-motion:reduce){.history-row{animation:none;transition:none}.history-row:hover{transform:none}}
</style>
