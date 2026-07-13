<script setup lang="ts">
import { Archive, BookOpen, Image, Plus, RotateCcw, Search, Trash2 } from '@lucide/vue'
import type { ProblemStatusFilter, ProblemSummary } from '../../../shared/api/bindings'

defineProps<{
  profileName: string
  status: ProblemStatusFilter
  search: string
  loading: boolean
  problems: ProblemSummary[]
  errorMessage?: string
  selectedProblemIds?: string[]
}>()

defineEmits<{
  capture: []
  statusChange: [status: ProblemStatusFilter]
  searchChange: [search: string]
  openDetail: [problemId: string]
  toggleSelection: [problemId: string]
  batchStatus: [status: ProblemStatusFilter]
}>()

const filters: Array<{ value: ProblemStatusFilter; label: string }> = [
  { value: 'active', label: '正在学习' },
  { value: 'archived', label: '已归档' },
  { value: 'trashed', label: '回收站' },
]
</script>

<template>
  <main
    class="library-workspace"
    aria-labelledby="library-title"
  >
    <header class="library-header">
      <div>
        <p class="eyebrow">
          {{ profileName }} · 私人题库
        </p>
        <h1 id="library-title">
          题库
        </h1>
        <p class="intro">
          把已经发生的错误整理清楚，下一次就少绕一点路。
        </p>
      </div>
      <button
        class="primary-action"
        type="button"
        @click="$emit('capture')"
      >
        <Plus
          :size="18"
          aria-hidden="true"
        />
        录入新错题
      </button>
    </header>

    <section
      class="library-toolbar"
      aria-label="题库筛选"
    >
      <div class="filter-tabs">
        <button
          v-for="filter in filters"
          :key="filter.value"
          type="button"
          :class="{ active: status === filter.value }"
          :aria-pressed="status === filter.value"
          @click="$emit('statusChange', filter.value)"
        >
          {{ filter.label }}
        </button>
      </div>
      <label class="search-field">
        <Search
          :size="17"
          aria-hidden="true"
        />
        <span class="sr-only">搜索题库</span>
        <input
          type="search"
          :value="search"
          placeholder="搜索科目或复盘笔记"
          @input="$emit('searchChange', ($event.target as HTMLInputElement).value)"
        >
      </label>
    </section>

    <section
      v-if="selectedProblemIds?.length"
      class="batch-bar"
      aria-label="批量操作"
    >
      <span>已选择 {{ selectedProblemIds.length }} 道题</span>
      <div>
        <button
          v-if="status === 'active'"
          type="button"
          @click="$emit('batchStatus', 'archived')"
        >
          <Archive :size="15" />归档
        </button>
        <button
          v-if="status !== 'trashed'"
          type="button"
          @click="$emit('batchStatus', 'trashed')"
        >
          <Trash2 :size="15" />移入回收站
        </button>
        <button
          v-else
          type="button"
          @click="$emit('batchStatus', 'active')"
        >
          <RotateCcw :size="15" />恢复学习
        </button>
      </div>
    </section>

    <p
      v-if="errorMessage"
      class="error-banner"
      role="alert"
    >
      {{ errorMessage }}
    </p>

    <section
      v-if="loading"
      class="loading-state"
      aria-live="polite"
    >
      <span /><span /><span />
      <p>正在打开加密题库…</p>
    </section>

    <section
      v-else-if="problems.length > 0"
      class="problem-grid"
      aria-label="错题列表"
    >
      <article
        v-for="problem in problems"
        :key="problem.id"
        class="problem-card"
      >
        <div class="problem-card__topline">
          <label class="select-problem">
            <input
              type="checkbox"
              :checked="selectedProblemIds?.includes(problem.id)"
              :aria-label="`选择 ${problem.subject || '未分类'} 错题`"
              @change="$emit('toggleSelection', problem.id)"
            >
            <span class="subject">{{ problem.subject || '未分类' }}</span>
          </label>
          <span class="status-dot">{{ problem.status === 'active' ? '学习中' : problem.status }}</span>
        </div>
        <p class="problem-note">
          {{ problem.note || '这道题还没有补充笔记。' }}
        </p>
        <div class="asset-counts">
          <span><Image
            :size="15"
            aria-hidden="true"
          />{{ problem.questionAssetCount }} 张题图</span>
          <span>·</span>
          <span>{{ problem.answerAssetCount }} 张答案</span>
        </div>
        <button
          type="button"
          class="card-link"
          @click="$emit('openDetail', problem.id)"
        >
          查看详情
          <span aria-hidden="true">→</span>
        </button>
      </article>
    </section>

    <section
      v-else-if="search"
      class="empty-state search-empty"
    >
      <span class="empty-icon"><Search
        :size="28"
        aria-hidden="true"
      /></span>
      <p class="empty-kicker">
        换一个更短的关键词试试
      </p>
      <h2>没有找到匹配的错题</h2>
      <p>当前筛选范围内，没有科目或复盘笔记包含“{{ search }}”的题目。</p>
    </section>

    <section
      v-else
      class="empty-state"
    >
      <span class="empty-icon"><BookOpen
        :size="28"
        aria-hidden="true"
      /></span>
      <p class="empty-kicker">
        第一张纸，留给真正想重看的题
      </p>
      <h2>题库还是空的</h2>
      <p>先录入一道题图和答案。图片会在本机加密保存，然后出现在这里。</p>
      <button
        type="button"
        class="primary-action"
        @click="$emit('capture')"
      >
        <Plus
          :size="18"
          aria-hidden="true"
        />
        录入第一道错题
      </button>
      <span class="empty-footnote"><Archive
        :size="14"
        aria-hidden="true"
      />以后归档的题也不会丢失</span>
    </section>
  </main>
</template>

<style scoped>
.library-workspace { max-width: 1120px; margin: 0 auto; padding: 58px 50px 72px; }
.library-header { display: flex; justify-content: space-between; gap: 32px; align-items: flex-end; }
.eyebrow { margin: 0 0 8px; color: var(--cinnabar); font-size: 12px; font-weight: 750; letter-spacing: .13em; }
h1 { margin: 0; font-size: clamp(42px,5vw,64px); letter-spacing: -.055em; line-height: 1; }
.intro { margin: 15px 0 0; color: var(--ink-muted); font-size: 16px; }
.primary-action { display: inline-flex; gap: 8px; align-items: center; justify-content: center; min-height: 44px; padding: 0 18px; color: var(--paper); border: 0; border-radius: 999px; background: var(--green-deep); font-weight: 720; cursor: pointer; transition: transform var(--motion-feedback) var(--ease-standard), background var(--motion-standard) var(--ease-standard); }
.primary-action:hover { background: #182923; }
.primary-action:active { transform: scale(.98); }
.library-toolbar { display: flex; justify-content: space-between; gap: 20px; margin-top: 42px; padding-bottom: 17px; border-bottom: 1px solid var(--line); }
.batch-bar { display: flex; justify-content: space-between; gap: 16px; align-items: center; margin-top: 16px; padding: 12px 15px; border: 1px solid rgba(33,51,45,.14); border-radius: 12px; background: var(--green-soft); font-size: 13px; font-weight: 720; }
.batch-bar div { display: flex; gap: 8px; }
.batch-bar button { display: inline-flex; gap: 6px; align-items: center; min-height: 34px; padding: 0 12px; color: var(--green-deep); border: 1px solid rgba(33,51,45,.16); border-radius: 999px; background: rgba(255,253,247,.72); cursor: pointer; }
.filter-tabs { display: flex; gap: 5px; }
.filter-tabs button { min-height: 37px; padding: 0 14px; color: var(--ink-muted); border: 0; border-radius: 999px; background: transparent; cursor: pointer; }
.filter-tabs button.active { color: var(--green-deep); background: var(--green-soft); font-weight: 720; }
.search-field { display: flex; gap: 8px; align-items: center; min-width: 240px; padding: 0 13px; color: var(--ink-muted); border: 1px solid var(--line); border-radius: 999px; background: rgba(255,253,247,.55); }
.search-field input { width: 100%; min-height: 36px; border: 0; outline: 0; background: transparent; }
.search-field:focus-within { border-color: var(--green-deep); box-shadow: 0 0 0 3px rgba(33,51,45,.1); }
.error-banner { margin: 20px 0 0; padding: 12px 15px; color: #7f3829; border: 1px solid rgba(185,88,63,.28); border-radius: 10px; background: rgba(185,88,63,.08); }
.loading-state { display: grid; gap: 12px; padding: 42px 0; }
.loading-state span { display: block; width: 100%; height: 72px; border-radius: 14px; background: rgba(232,221,199,.55); animation: pulse 1.2s ease-in-out infinite alternate; }
.loading-state p { color: var(--ink-muted); }
.problem-grid { display: grid; grid-template-columns: repeat(2,minmax(0,1fr)); gap: 16px; margin-top: 24px; }
.problem-card { padding: 23px; border: 1px solid var(--line); border-radius: 4px 18px 18px 18px; background: rgba(255,253,247,.7); box-shadow: 0 10px 30px rgba(34,48,43,.05); }
.problem-card__topline, .asset-counts { display: flex; gap: 9px; align-items: center; }
.problem-card__topline { justify-content: space-between; }
.select-problem { display: inline-flex; gap: 9px; align-items: center; cursor: pointer; }
.select-problem input { width: 16px; height: 16px; accent-color: var(--green-deep); }
.subject { font-weight: 760; }
.status-dot { color: #567064; font-size: 11px; }
.problem-note { min-height: 48px; margin: 22px 0; font-size: 16px; line-height: 1.55; }
.asset-counts { color: var(--ink-muted); font-size: 12px; }
.asset-counts span { display: inline-flex; gap: 5px; align-items: center; }
.card-link { display: flex; justify-content: space-between; width: 100%; margin-top: 20px; padding: 14px 0 0; color: var(--green-deep); border: 0; border-top: 1px solid var(--line); background: transparent; text-align: left; cursor: pointer; }
.card-link:hover { color: var(--cinnabar); }
.empty-state { display: grid; justify-items: center; max-width: 680px; margin: 52px auto 0; padding: 62px 42px; border: 1px solid var(--line); border-radius: 4px 24px 24px 24px; background: rgba(255,253,247,.66); box-shadow: var(--shadow-soft); text-align: center; }
.empty-icon { display: grid; width: 54px; height: 54px; place-items: center; color: var(--green-deep); border-radius: 18px 18px 18px 5px; background: var(--green-soft); }
.empty-kicker { margin: 24px 0 7px; color: var(--cinnabar); font-size: 12px; letter-spacing: .08em; }
.empty-state h2 { margin: 0; font-size: 30px; letter-spacing: -.03em; }
.empty-state > p:not(.empty-kicker) { max-width: 480px; margin: 13px 0 24px; color: var(--ink-muted); line-height: 1.65; }
.empty-footnote { display: inline-flex; gap: 7px; align-items: center; margin-top: 23px; color: var(--ink-muted); font-size: 11px; }
.search-empty { padding-block: 48px; }
.sr-only { position: absolute; width: 1px; height: 1px; overflow: hidden; clip: rect(0,0,0,0); }
@keyframes pulse { from { opacity: .45; } to { opacity: .8; } }
@media (max-width: 820px) { .library-workspace { padding: 34px 22px 90px; } .library-header { align-items: flex-start; flex-direction: column; } .library-toolbar { align-items: stretch; flex-direction: column; } .search-field { min-width: 0; } .problem-grid { grid-template-columns: 1fr; } }
@media (prefers-reduced-motion: reduce) { .loading-state span { animation: none; } }
</style>
