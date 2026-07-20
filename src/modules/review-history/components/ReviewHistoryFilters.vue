<script setup lang="ts">
import { RotateCcw, Search } from '@lucide/vue'
import { reactive } from 'vue'
import type { FsrsRating, ReviewHistoryRange } from '@/shared/api/bindings'

export interface HistoryFiltersValue {
  range: ReviewHistoryRange
  rating: FsrsRating | null
  subject: string | null
  search: string
}

defineProps<{ subjects: string[]; loading: boolean }>()
const emit = defineEmits<{ submit: [filters: HistoryFiltersValue]; reset: [] }>()
const model = reactive<HistoryFiltersValue>({ range: '30_days', rating: null, subject: null, search: '' })

function submit() {
  emit('submit', { ...model, search: model.search.trim() })
}
function reset() {
  Object.assign(model, { range: '30_days', rating: null, subject: null, search: '' })
  emit('reset')
}
</script>

<template>
  <form
    class="history-filters"
    aria-label="筛选复习历史"
    @submit.prevent="submit"
  >
    <label><span>时间</span><select
      v-model="model.range"
      :disabled="loading"
    ><option value="7_days">近 7 天</option><option value="30_days">近 30 天</option><option value="all">全部时间</option></select></label>
    <label><span>评分</span><select
      v-model="model.rating"
      :disabled="loading"
    ><option :value="null">全部评分</option><option value="again">忘记</option><option value="hard">困难</option><option value="good">记住</option><option value="easy">轻松</option></select></label>
    <label><span>科目</span><select
      v-model="model.subject"
      :disabled="loading"
    ><option :value="null">全部科目</option><option
      v-for="subject in subjects"
      :key="subject"
      :value="subject"
    >{{ subject }}</option></select></label>
    <label class="search-field"><span>笔记关键词</span><span class="search-control"><Search
      :size="16"
      aria-hidden="true"
    /><input
      v-model="model.search"
      maxlength="80"
      placeholder="例如：圆锥曲线"
      :disabled="loading"
    ></span></label>
    <button
      class="apply-button"
      type="submit"
      :disabled="loading"
    >
      {{ loading ? '正在筛选…' : '应用筛选' }}
    </button>
    <button
      class="reset-button"
      type="button"
      :disabled="loading"
      @click="reset"
    >
      <RotateCcw
        :size="15"
        aria-hidden="true"
      />重置
    </button>
  </form>
</template>

<style scoped>
.history-filters{display:grid;grid-template-columns:130px 130px 150px minmax(210px,1fr) auto auto;gap:10px;align-items:end;padding:16px;border:1px solid var(--line);border-radius:6px 18px 18px;background:rgba(255,253,247,.72);box-shadow:var(--shadow-soft)}label{display:grid;gap:6px;min-width:0}label>span:first-child{color:var(--ink-muted);font-size:10px;font-weight:750;letter-spacing:.06em}select,.search-control{min-height:44px;border:1px solid var(--line);border-radius:11px;background:var(--paper-raised);color:var(--ink)}select{width:100%;padding:0 11px;cursor:pointer}.search-control{display:flex;gap:8px;align-items:center;padding:0 12px;color:var(--ink-muted)}.search-control input{width:100%;min-width:0;border:0;outline:0;background:transparent;color:var(--ink)}button{display:inline-flex;gap:6px;align-items:center;justify-content:center;min-height:44px;padding:0 16px;border:1px solid var(--line);border-radius:999px;cursor:pointer;transition:transform var(--motion-feedback) var(--ease-standard),box-shadow var(--motion-standard) var(--ease-standard)}.apply-button{color:var(--paper-raised);border-color:var(--green-deep);background:var(--green-deep)}.reset-button{background:transparent}.apply-button:hover:not(:disabled),.reset-button:hover:not(:disabled){transform:translateY(-1px)}.apply-button:hover:not(:disabled){box-shadow:0 8px 20px rgba(33,51,45,.16)}button:disabled{opacity:.5;cursor:not-allowed}@media(max-width:1100px){.history-filters{grid-template-columns:repeat(3,minmax(0,1fr))}.search-field{grid-column:1/3}}@media(max-width:620px){.history-filters{grid-template-columns:1fr 1fr;padding:13px}.search-field{grid-column:1/-1}.apply-button,.reset-button{width:100%}}@media(prefers-reduced-motion:reduce){button{transition:none}.apply-button:hover:not(:disabled),.reset-button:hover:not(:disabled){transform:none}}
</style>
