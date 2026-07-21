<script setup lang="ts">
import { CalendarClock, CheckCircle2, History, Images, LibraryBig, Search } from '@lucide/vue'
import { computed, ref } from 'vue'
import type { ExportCandidate, ExportCandidateSource } from '../../../shared/api/bindings'

const props = defineProps<{
  candidates: ExportCandidate[]
  source: ExportCandidateSource
  selectedIds: string[]
  loading: boolean
}>()

const emit = defineEmits<{
  source: [source: ExportCandidateSource]
  toggle: [problemId: string]
  selectAll: [problemIds: string[]]
  clear: []
}>()

const search = ref('')
const selected = computed(() => new Set(props.selectedIds))
const normalizedSearch = computed(() => search.value.trim().toLocaleLowerCase('zh-CN'))
const filteredCandidates = computed(() => {
  if (!normalizedSearch.value)
    return props.candidates
  return props.candidates.filter(candidate => `${candidate.subject} ${candidate.note}`
    .toLocaleLowerCase('zh-CN')
    .includes(normalizedSearch.value))
})

const sourceOptions: Array<{
  value: ExportCandidateSource
  label: string
  hint: string
  icon: typeof CalendarClock
}> = [
  { value: 'due', label: '到期队列', hint: '新题和现在应该复习的题', icon: CalendarClock },
  { value: 'latest_review_session', label: '最近训练批次', hint: '保留上一次训练的题目顺序', icon: History },
  { value: 'all_active', label: '全部活动题', hint: '从题库中手动筛选', icon: LibraryBig },
]

const emptyCopy = computed(() => {
  if (props.source === 'latest_review_session')
    return '还没有可用的最近训练批次。'
  if (props.source === 'due')
    return '当前没有新题或到期题。'
  return '题库中还没有活动题目。'
})

function dueLabel(value: number | null) {
  if (value == null)
    return '尚未训练'
  return `到期 ${new Date(value).toLocaleDateString('zh-CN')}`
}

function selectionLabel(candidate: ExportCandidate) {
  return `选择${candidate.subject}：${candidate.note || '无笔记题目'}`
}
</script>

<template>
  <section class="candidate-picker">
    <div
      class="source-grid"
      role="radiogroup"
      aria-label="导出题目来源"
    >
      <button
        v-for="option in sourceOptions"
        :key="option.value"
        type="button"
        role="radio"
        class="source-card"
        :class="{ active: source === option.value }"
        :aria-checked="source === option.value"
        :disabled="loading"
        @click="emit('source', option.value)"
      >
        <component
          :is="option.icon"
          :size="19"
          aria-hidden="true"
        />
        <span><strong>{{ option.label }}</strong><small>{{ option.hint }}</small></span>
        <CheckCircle2
          v-if="source === option.value"
          class="source-check"
          :size="17"
          aria-hidden="true"
        />
      </button>
    </div>

    <div class="picker-toolbar">
      <label class="search-field">
        <Search
          :size="16"
          aria-hidden="true"
        />
        <span class="sr-only">搜索候选题</span>
        <input
          v-model="search"
          type="search"
          aria-label="搜索候选题"
          placeholder="搜索科目或笔记"
          :disabled="loading || candidates.length === 0"
        >
      </label>
      <span class="selection-count">已选 {{ selectedIds.length }} / {{ candidates.length }}</span>
      <button
        type="button"
        :disabled="loading || filteredCandidates.length === 0"
        @click="emit('selectAll', filteredCandidates.map(candidate => candidate.id))"
      >
        全选当前结果
      </button>
      <button
        type="button"
        :disabled="loading || selectedIds.length === 0"
        @click="emit('clear')"
      >
        清空选择
      </button>
    </div>

    <div
      class="candidate-viewport"
      :aria-busy="loading"
    >
      <p
        v-if="loading"
        class="picker-state"
        aria-live="polite"
      >
        <span class="loading-mark" />正在读取可导出的题目…
      </p>
      <p
        v-else-if="candidates.length === 0"
        class="picker-state"
      >
        {{ emptyCopy }}
      </p>
      <p
        v-else-if="filteredCandidates.length === 0"
        class="picker-state"
      >
        没有匹配“{{ search.trim() }}”的题目。
      </p>
      <TransitionGroup
        v-else
        name="candidate-row"
        tag="div"
        class="candidate-list"
      >
        <label
          v-for="candidate in filteredCandidates"
          :key="candidate.id"
          class="candidate-row"
          :class="{ selected: selected.has(candidate.id) }"
        >
          <input
            type="checkbox"
            :checked="selected.has(candidate.id)"
            :aria-label="selectionLabel(candidate)"
            @change="emit('toggle', candidate.id)"
          >
          <span class="selection-mark"><CheckCircle2 :size="17" /></span>
          <span class="candidate-copy">
            <span><strong>{{ candidate.subject || '未分类' }}</strong><small>{{ dueLabel(candidate.dueAtUtcMs) }} · {{ candidate.reviewCount }} 次复习</small></span>
            <p>{{ candidate.note || '这道题还没有复盘笔记。' }}</p>
          </span>
          <span class="asset-badges">
            <span><Images :size="13" />题 {{ candidate.questionAssetCount }} · 答 {{ candidate.answerAssetCount }}</span>
            <small :class="{ incomplete: candidate.questionAssetCount === 0 || candidate.answerAssetCount === 0 }">
              {{ candidate.questionAssetCount > 0 && candidate.answerAssetCount > 0 ? '题答齐全' : candidate.questionAssetCount === 0 ? '缺少题图' : '缺少答案' }}
            </small>
          </span>
        </label>
      </TransitionGroup>
    </div>
  </section>
</template>

<style scoped>
.candidate-picker { display: grid; gap: 14px; }
.source-grid { display: grid; grid-template-columns: repeat(3, minmax(0, 1fr)); gap: 10px; }
.source-card { position: relative; display: grid; min-height: 82px; grid-template-columns: auto 1fr auto; gap: 11px; align-items: center; padding: 14px; color: var(--ink); border: 1px solid var(--line); border-radius: 5px 15px 15px; background: rgba(255, 253, 247, .72); cursor: pointer; text-align: left; transition: transform var(--motion-feedback) var(--ease-standard), border-color var(--motion-standard) var(--ease-standard), background var(--motion-standard) var(--ease-standard); }
.source-card:hover:not(:disabled) { transform: translateY(-2px); }
.source-card.active { border-color: rgba(185, 88, 63, .48); background: #fbf2e7; box-shadow: inset 3px 0 var(--cinnabar); }
.source-card:disabled { cursor: wait; opacity: .62; }
.source-card span { min-width: 0; }
.source-card strong,
.source-card small { display: block; }
.source-card strong { font-size: 13px; }
.source-card small { margin-top: 5px; color: var(--ink-muted); font-size: 10px; line-height: 1.45; }
.source-check { color: var(--cinnabar); }

.picker-toolbar { display: flex; gap: 8px; align-items: center; min-height: 42px; }
.search-field { display: flex; min-width: 190px; flex: 1; gap: 8px; align-items: center; padding: 0 12px; border: 1px solid var(--line); border-radius: 999px; background: var(--paper-raised); }
.search-field input { width: 100%; min-height: 38px; padding: 0; border: 0; outline: 0; background: transparent; color: var(--ink); }
.selection-count { color: var(--ink-muted); font-size: 11px; white-space: nowrap; }
.picker-toolbar button { min-height: 36px; padding: 0 11px; color: var(--green-deep); border: 1px solid var(--line); border-radius: 999px; background: transparent; cursor: pointer; font-size: 11px; }
.picker-toolbar button:disabled { cursor: default; opacity: .42; }

.candidate-viewport { min-height: 168px; max-height: 430px; overflow: auto; overscroll-behavior: contain; border: 1px solid var(--line); border-radius: 5px 15px 15px; background: rgba(255, 253, 247, .5); }
.candidate-list { display: grid; }
.candidate-row { position: relative; display: grid; min-height: 84px; grid-template-columns: auto minmax(0, 1fr) auto; gap: 12px; align-items: center; padding: 13px 15px; border-bottom: 1px solid var(--line); cursor: pointer; transition: transform var(--motion-feedback) var(--ease-standard), background var(--motion-standard) var(--ease-standard), box-shadow var(--motion-standard) var(--ease-standard); }
.candidate-row:last-child { border-bottom: 0; }
.candidate-row:hover { background: rgba(232, 221, 199, .22); transform: translateX(2px); }
.candidate-row.selected { background: rgba(227, 235, 228, .7); box-shadow: inset 3px 0 var(--green-deep); }
.candidate-row > input { position: absolute; width: 1px; height: 1px; overflow: hidden; opacity: 0; }
.selection-mark { display: grid; width: 28px; height: 28px; place-items: center; color: var(--ink-muted); border: 1px solid var(--line); border-radius: 50%; background: var(--paper-raised); transition: color var(--motion-standard) var(--ease-standard), background var(--motion-standard) var(--ease-standard), transform var(--motion-feedback) var(--ease-standard); }
.candidate-row.selected .selection-mark { color: white; border-color: var(--green-deep); background: var(--green-deep); transform: scale(1.06); }
.candidate-row:focus-within { outline: 3px solid rgba(185, 88, 63, .25); outline-offset: -3px; }
.candidate-copy { min-width: 0; }
.candidate-copy > span { display: flex; gap: 10px; align-items: baseline; }
.candidate-copy strong { color: var(--green-deep); font-size: 13px; }
.candidate-copy small { color: var(--ink-muted); font-size: 10px; }
.candidate-copy p { overflow: hidden; margin: 7px 0 0; color: var(--ink); font-size: 12px; text-overflow: ellipsis; white-space: nowrap; }
.asset-badges { display: grid; gap: 6px; justify-items: end; }
.asset-badges > span { display: inline-flex; gap: 5px; align-items: center; color: var(--ink-muted); font-size: 10px; }
.asset-badges small { padding: 4px 7px; color: #48665a; border-radius: 999px; background: var(--green-soft); font-size: 9px; }
.asset-badges small.incomplete { color: #8b4634; background: #f6e5dc; }
.picker-state { display: flex; min-height: 168px; gap: 9px; align-items: center; justify-content: center; margin: 0; color: var(--ink-muted); font-size: 12px; }
.loading-mark { width: 16px; height: 16px; border: 2px solid var(--sand); border-top-color: var(--cinnabar); border-radius: 50%; animation: spin .8s linear infinite; }
.sr-only { position: absolute; width: 1px; height: 1px; overflow: hidden; clip: rect(0, 0, 0, 0); white-space: nowrap; }

.candidate-row-enter-active,
.candidate-row-leave-active { transition: opacity var(--motion-standard) var(--ease-standard), transform var(--motion-standard) var(--ease-standard); }
.candidate-row-enter-from,
.candidate-row-leave-to { opacity: 0; transform: translateY(6px) scale(.99); }
@keyframes spin { to { transform: rotate(360deg); } }

@media (max-width: 820px) {
  .source-grid { grid-template-columns: 1fr; }
  .source-card { min-height: 68px; }
  .picker-toolbar { align-items: stretch; flex-wrap: wrap; }
  .search-field { width: 100%; flex-basis: 100%; }
  .candidate-row { grid-template-columns: auto minmax(0, 1fr); }
  .asset-badges { grid-column: 2; justify-items: start; }
}

@media (prefers-reduced-motion: reduce) {
  .source-card,
  .candidate-row,
  .selection-mark,
  .candidate-row-enter-active,
  .candidate-row-leave-active { transition: none; }
  .source-card:hover:not(:disabled),
  .candidate-row:hover,
  .candidate-row.selected .selection-mark,
  .candidate-row-enter-from,
  .candidate-row-leave-to { transform: none; }
  .loading-mark { animation: none; }
}
</style>
