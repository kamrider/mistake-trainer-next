<script setup lang="ts">
import { Archive, BookOpen, CheckCheck, ClipboardCheck, FilePenLine, Image, ListChecks, LoaderCircle, Play, Plus, RotateCcw, Search, Trash2, X } from '@lucide/vue'
import { computed, ref } from 'vue'
import type { ProblemStatusFilter, ProblemSummary } from '../../../shared/api/bindings'
import { EMPTY_LIBRARY_FILTERS, type LibraryAdvancedFilters } from '../domain/libraryFilters'
import LibraryFilterPanel from './LibraryFilterPanel.vue'

const props = defineProps<{
  profileName: string
  status: ProblemStatusFilter
  search: string
  loading: boolean
  loadingMore?: boolean
  hasMore?: boolean
  problems: ProblemSummary[]
  errorMessage?: string
  selectedProblemIds?: string[]
  startingExperience?: 'review' | 'exam' | null
  changingBatchStatus?: ProblemStatusFilter | null
  bulkMetadataBusy?: boolean
  advancedFilters?: LibraryAdvancedFilters
  subjectOptions?: string[]
  tagOptions?: string[]
}>()

const emit = defineEmits<{
  capture: []
  statusChange: [status: ProblemStatusFilter]
  searchChange: [search: string]
  openDetail: [problemId: string]
  toggleSelection: [problemId: string]
  batchStatus: [status: ProblemStatusFilter]
  trainSelection: []
  startExam: []
  selectAll: []
  clearSelection: []
  filtersChange: [filters: LibraryAdvancedFilters]
  bulkMetadata: []
  loadMore: []
}>()

const filters: Array<{ value: ProblemStatusFilter; label: string }> = [
  { value: 'active', label: '正在学习' },
  { value: 'archived', label: '已归档' },
  { value: 'trashed', label: '回收站' },
]

const selectionMode = ref(false)
const isSelecting = computed(() => selectionMode.value || Boolean(props.selectedProblemIds?.length))
const batchInteractionBusy = computed(() => Boolean(
  props.startingExperience || props.changingBatchStatus || props.bulkMetadataBusy))
const resolvedFilters = computed(() => props.advancedFilters ?? EMPTY_LIBRARY_FILTERS)
const resolvedSubjectOptions = computed(() => props.subjectOptions ?? [])
const resolvedTagOptions = computed(() => props.tagOptions ?? [])

function toggleBatchManagement() {
  if (isSelecting.value) {
    selectionMode.value = false
    emit('clearSelection')
    return
  }
  selectionMode.value = true
}
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
        添加素材
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
          :disabled="batchInteractionBusy"
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
          :disabled="batchInteractionBusy"
          placeholder="搜索科目、标签或复盘笔记"
          @input="$emit('searchChange', ($event.target as HTMLInputElement).value)"
        >
      </label>
      <LibraryFilterPanel
        :model-value="resolvedFilters"
        :subject-options="resolvedSubjectOptions"
        :tag-options="resolvedTagOptions"
        :disabled="batchInteractionBusy"
        @update:model-value="$emit('filtersChange', $event)"
      />
      <div
        v-if="problems.length"
        class="selection-tools"
      >
        <button
          v-if="isSelecting"
          class="select-all-action"
          type="button"
          :disabled="batchInteractionBusy"
          @click="$emit('selectAll')"
        >
          <CheckCheck
            :size="16"
            aria-hidden="true"
          />
          全选当前结果
        </button>
        <button
          class="select-all-action"
          type="button"
          :aria-pressed="isSelecting"
          :disabled="batchInteractionBusy"
          @click="toggleBatchManagement"
        >
          <ListChecks
            :size="16"
            aria-hidden="true"
          />
          {{ isSelecting ? '退出批量管理' : '批量管理' }}
        </button>
      </div>
    </section>

    <Transition name="deck-dock">
      <section
        v-if="selectedProblemIds?.length"
        class="batch-bar"
        aria-label="所选题目操作"
      >
        <div
          class="selection-summary"
          role="status"
          aria-live="polite"
          aria-atomic="true"
        >
          <span class="selection-count">{{ selectedProblemIds.length }}</span>
          <span>道题已放入本轮卡组</span>
        </div>
        <div class="batch-actions">
          <button
            v-if="status === 'active'"
            class="start-review-action"
            type="button"
            :disabled="batchInteractionBusy"
            @click="$emit('trainSelection')"
          >
            <LoaderCircle
              v-if="startingExperience === 'review'"
              class="spin"
              :size="17"
              aria-hidden="true"
            />
            <Play
              v-else
              :size="17"
              aria-hidden="true"
            />
            {{ startingExperience === 'review' ? '正在整理训练卡组…' : `开始训练 ${selectedProblemIds.length} 道题` }}
          </button>
          <button
            v-if="status === 'active'"
            type="button"
            class="start-exam-action"
            :disabled="batchInteractionBusy"
            @click="$emit('startExam')"
          >
            <LoaderCircle
              v-if="startingExperience === 'exam'"
              class="spin"
              :size="17"
              aria-hidden="true"
            />
            <ClipboardCheck
              v-else
              :size="17"
              aria-hidden="true"
            />
            {{ startingExperience === 'exam' ? '正在准备模拟考试…' : `模拟考试 ${selectedProblemIds.length} 道题` }}
          </button>
          <button
            v-if="status === 'active'"
            type="button"
            :disabled="batchInteractionBusy"
            @click="$emit('bulkMetadata')"
          >
            <LoaderCircle
              v-if="bulkMetadataBusy"
              class="spin"
              :size="15"
              aria-hidden="true"
            />
            <FilePenLine
              v-else
              :size="15"
              aria-hidden="true"
            />
            {{ bulkMetadataBusy ? '正在批量修改…' : '批量修改' }}
          </button>
          <button
            v-if="status === 'active'"
            type="button"
            :disabled="batchInteractionBusy"
            @click="$emit('batchStatus', 'archived')"
          >
            <LoaderCircle
              v-if="changingBatchStatus === 'archived'"
              class="spin"
              :size="15"
              aria-hidden="true"
            />
            <Archive
              v-else
              :size="15"
              aria-hidden="true"
            />{{ changingBatchStatus === 'archived' ? '正在归档…' : '归档' }}
          </button>
          <button
            v-if="status !== 'trashed'"
            type="button"
            :disabled="batchInteractionBusy"
            @click="$emit('batchStatus', 'trashed')"
          >
            <LoaderCircle
              v-if="changingBatchStatus === 'trashed'"
              class="spin"
              :size="15"
              aria-hidden="true"
            />
            <Trash2
              v-else
              :size="15"
              aria-hidden="true"
            />{{ changingBatchStatus === 'trashed' ? '正在移入回收站…' : '移入回收站' }}
          </button>
          <button
            v-else
            type="button"
            :disabled="batchInteractionBusy"
            @click="$emit('batchStatus', 'active')"
          >
            <LoaderCircle
              v-if="changingBatchStatus === 'active'"
              class="spin"
              :size="15"
              aria-hidden="true"
            />
            <RotateCcw
              v-else
              :size="15"
              aria-hidden="true"
            />{{ changingBatchStatus === 'active' ? '正在恢复学习…' : '恢复学习' }}
          </button>
          <button
            class="clear-selection"
            type="button"
            :disabled="batchInteractionBusy"
            @click="$emit('clearSelection')"
          >
            <X
              :size="15"
              aria-hidden="true"
            />清空选择
          </button>
        </div>
      </section>
    </Transition>

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

    <div
      v-else-if="problems.length > 0"
      class="problem-list-state"
    >
      <section
        class="problem-grid"
        aria-label="错题列表"
      >
        <article
          v-for="problem in problems"
          :key="problem.id"
          class="problem-card"
          :class="{ selected: selectedProblemIds?.includes(problem.id) }"
        >
          <div class="problem-card__topline">
            <label
              v-if="isSelecting"
              class="select-problem"
            >
              <input
                type="checkbox"
                :checked="selectedProblemIds?.includes(problem.id)"
                :disabled="batchInteractionBusy"
                :aria-label="`选择 ${problem.subject || '未分类'} 错题`"
                @change="$emit('toggleSelection', problem.id)"
              >
              <span class="sr-only">选择这道题</span>
            </label>
            <span class="subject">{{ problem.subject || '未分类' }}</span>
            <span class="status-dot">{{ problem.status === 'active' ? '学习中' : problem.status }}</span>
          </div>
          <button
            class="problem-preview"
            type="button"
            :disabled="batchInteractionBusy"
            :aria-label="`打开 ${problem.subject || '未分类'} 错题详情`"
            @click="$emit('openDetail', problem.id)"
          >
            <img
              v-if="problem.questionPreviewDataUrl"
              :src="problem.questionPreviewDataUrl"
              :alt="`${problem.subject || '未分类'} 题图预览`"
              loading="lazy"
            >
            <span v-else>
              <Image
                :size="23"
                aria-hidden="true"
              />
              题图预览暂不可用
            </span>
            <small>点击题图进入题卡</small>
          </button>
          <div
            v-if="problem.tags?.length"
            class="problem-tags"
            :aria-label="`${problem.subject || '未分类'} 标签`"
          >
            <span
              v-for="tag in (problem.tags ?? []).slice(0, 3)"
              :key="tag"
            >{{ tag }}</span>
            <span v-if="(problem.tags?.length ?? 0) > 3">+{{ (problem.tags?.length ?? 0) - 3 }}</span>
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
        </article>
      </section>
      <div
        v-if="hasMore"
        class="load-more-row"
      >
        <button
          type="button"
          :disabled="loadingMore || batchInteractionBusy"
          @click="$emit('loadMore')"
        >
          <LoaderCircle
            v-if="loadingMore"
            class="spin"
            :size="17"
            aria-hidden="true"
          />
          {{ loadingMore ? '正在加载更多…' : '加载更多' }}
        </button>
      </div>
    </div>

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
        添加第一份素材
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
.library-toolbar { display: flex; justify-content: space-between; gap: 12px; margin-top: 42px; padding-bottom: 17px; border-bottom: 1px solid var(--line); }
.selection-tools { display: flex; gap: 8px; align-items: center; }
.select-all-action { display: inline-flex; gap: 6px; align-items: center; min-height: 44px; padding: 0 13px; color: var(--green-deep); border: 1px solid var(--line); border-radius: 999px; background: rgba(255,253,247,.65); cursor: pointer; white-space: nowrap; }
.batch-bar { position: sticky; z-index: 12; bottom: 18px; display: flex; justify-content: space-between; gap: 16px; align-items: center; margin-top: 16px; padding: 11px 12px 11px 16px; border: 1px solid rgba(33,51,45,.22); border-radius: 17px; background: rgba(246,241,231,.95); box-shadow: 0 18px 46px rgba(34,48,43,.18); backdrop-filter: blur(14px); font-size: 13px; font-weight: 720; }
.selection-summary, .batch-actions { display: flex; gap: 8px; align-items: center; }
.selection-count { display: grid; width: 30px; height: 30px; place-items: center; color: var(--paper); border-radius: 10px 3px 10px 10px; background: var(--cinnabar); font-family: var(--font-serif); font-size: 16px; }
.batch-bar button { display: inline-flex; gap: 6px; align-items: center; min-height: 44px; padding: 0 12px; color: var(--green-deep); border: 1px solid rgba(33,51,45,.16); border-radius: 999px; background: rgba(255,253,247,.78); cursor: pointer; transition: transform var(--motion-feedback) var(--ease-standard), background var(--motion-standard) var(--ease-standard), opacity var(--motion-feedback) var(--ease-standard); }
.batch-bar button:hover:not(:disabled) { transform: translateY(-1px); background: #fffdf7; }
.batch-bar button:disabled, .select-all-action:disabled { opacity: .58; cursor: wait; }
.batch-bar .start-review-action { min-height: 44px; padding-inline: 16px; color: var(--paper); border-color: var(--green-deep); background: var(--green-deep); box-shadow: 0 7px 18px rgba(33,51,45,.18); }
.batch-bar .start-review-action:hover:not(:disabled) { background: #182923; }
.batch-bar .start-exam-action { min-height: 44px; padding-inline: 16px; color: #873e2f; border-color: rgba(185,88,63,.42); background: #fff6ed; box-shadow: 0 7px 18px rgba(185,88,63,.1); }
.batch-bar .start-exam-action:hover:not(:disabled) { color: #713123; border-color: var(--cinnabar); background: #fffaf3; }
.batch-bar .start-review-action:not(:disabled):active,
.batch-bar .start-exam-action:not(:disabled):active { transform: scale(.975); }
.deck-dock-enter-active, .deck-dock-leave-active { transition: transform var(--motion-page) var(--ease-standard), opacity var(--motion-standard) var(--ease-standard); }
.deck-dock-enter-from, .deck-dock-leave-to { opacity: 0; transform: translateY(12px) scale(.98); }
.spin { animation: spin .8s linear infinite; }
.filter-tabs { display: flex; gap: 5px; }
.filter-tabs button { min-height: 44px; padding: 0 14px; color: var(--ink-muted); border: 0; border-radius: 999px; background: transparent; cursor: pointer; }
.filter-tabs button.active { color: var(--green-deep); background: var(--green-soft); font-weight: 720; }
.search-field { display: flex; gap: 8px; align-items: center; min-width: 240px; padding: 0 13px; color: var(--ink-muted); border: 1px solid var(--line); border-radius: 999px; background: rgba(255,253,247,.55); }
.search-field input { width: 100%; min-height: 44px; border: 0; outline: 0; background: transparent; }
.search-field:focus-within { border-color: var(--green-deep); box-shadow: 0 0 0 3px rgba(33,51,45,.1); }
.error-banner { margin: 20px 0 0; padding: 12px 15px; color: #7f3829; border: 1px solid rgba(185,88,63,.28); border-radius: 10px; background: rgba(185,88,63,.08); }
.load-more-row { display: flex; justify-content: center; margin-top: 24px; }
.load-more-row button { display: inline-flex; gap: 8px; align-items: center; justify-content: center; min-width: 148px; min-height: 44px; padding: 0 18px; color: var(--green-deep); border: 1px solid var(--line); border-radius: 999px; background: rgba(255,253,247,.72); cursor: pointer; }
.load-more-row button:disabled { opacity: .58; cursor: wait; }
.loading-state { display: grid; gap: 12px; padding: 42px 0; }
.loading-state span { display: block; width: 100%; height: 72px; border-radius: 14px; background: rgba(232,221,199,.55); animation: pulse 1.2s ease-in-out infinite alternate; }
.loading-state p { color: var(--ink-muted); }
.problem-grid { display: grid; grid-template-columns: repeat(2,minmax(0,1fr)); gap: 16px; margin-top: 24px; }
.problem-card { padding: 23px; border: 1px solid var(--line); border-radius: 4px 18px 18px 18px; background: rgba(255,253,247,.7); box-shadow: 0 10px 30px rgba(34,48,43,.05); transition: transform var(--motion-standard) var(--ease-standard), border-color var(--motion-standard) var(--ease-standard), background var(--motion-standard) var(--ease-standard), box-shadow var(--motion-standard) var(--ease-standard); }
.problem-card.selected { border-color: rgba(185,88,63,.55); background: rgba(255,249,239,.96); box-shadow: 0 15px 36px rgba(185,88,63,.12); transform: translateY(-2px); }
.problem-card__topline, .asset-counts { display: flex; gap: 9px; align-items: center; }
.problem-card__topline { justify-content: flex-start; }
.problem-card__topline .status-dot { margin-left: auto; }
.select-problem { display: inline-flex; min-width: 44px; min-height: 44px; gap: 9px; align-items: center; justify-content: center; cursor: pointer; }
.select-problem input { width: 16px; height: 16px; accent-color: var(--green-deep); }
.subject { font-weight: 760; }
.status-dot { color: #567064; font-size: 12px; }
.problem-preview { position: relative; display: grid; width: 100%; aspect-ratio: 4 / 3; margin-top: 18px; overflow: hidden; place-items: center; color: var(--ink-muted); border: 1px solid rgba(33,51,45,.12); border-radius: 14px; background: linear-gradient(145deg,rgba(232,221,199,.64),rgba(255,253,247,.92)); cursor: pointer; transition: transform var(--motion-standard) var(--ease-standard), border-color var(--motion-standard) var(--ease-standard), box-shadow var(--motion-standard) var(--ease-standard); }
.problem-preview:hover { border-color: rgba(185,88,63,.48); box-shadow: 0 12px 26px rgba(34,48,43,.11); transform: translateY(-2px); }
.problem-preview:focus-visible { outline: 3px solid rgba(185,88,63,.34); outline-offset: 2px; }
.problem-preview img { width: 100%; height: 100%; object-fit: contain; background: #fff; }
.problem-preview > span { display: grid; gap: 8px; place-items: center; padding: 24px; font-size: 12px; }
.problem-preview small { position: absolute; right: 9px; bottom: 8px; padding: 5px 8px; color: var(--paper); border-radius: 999px; background: rgba(33,51,45,.76); font-size: 12px; font-weight: 720; }
.problem-tags { display: flex; flex-wrap: wrap; gap: 7px; margin-top: 14px; }
.problem-tags span { max-width: 150px; padding: 5px 9px; overflow: hidden; color: var(--green-deep); border: 1px solid rgba(33,51,45,.12); border-radius: 999px; background: var(--green-soft); font-size: 12px; font-weight: 720; text-overflow: ellipsis; white-space: nowrap; animation: tag-rise var(--motion-standard) var(--ease-standard); }
.problem-note { min-height: 48px; margin: 20px 0; font-size: 16px; line-height: 1.55; }
.asset-counts { color: var(--ink-muted); font-size: 12px; }
.asset-counts span { display: inline-flex; gap: 5px; align-items: center; }
.empty-state { display: grid; justify-items: center; max-width: 680px; margin: 52px auto 0; padding: 62px 42px; border: 1px solid var(--line); border-radius: 4px 24px 24px 24px; background: rgba(255,253,247,.66); box-shadow: var(--shadow-soft); text-align: center; }
.empty-icon { display: grid; width: 54px; height: 54px; place-items: center; color: var(--green-deep); border-radius: 18px 18px 18px 5px; background: var(--green-soft); }
.empty-kicker { margin: 24px 0 7px; color: var(--cinnabar); font-size: 12px; letter-spacing: .08em; }
.empty-state h2 { margin: 0; font-size: 30px; letter-spacing: -.03em; }
.empty-state > p:not(.empty-kicker) { max-width: 480px; margin: 13px 0 24px; color: var(--ink-muted); line-height: 1.65; }
.empty-footnote { display: inline-flex; gap: 7px; align-items: center; margin-top: 23px; color: var(--ink-muted); font-size: 12px; }
.search-empty { padding-block: 48px; }
.sr-only { position: absolute; width: 1px; height: 1px; overflow: hidden; clip: rect(0,0,0,0); }
@keyframes pulse { from { opacity: .45; } to { opacity: .8; } }
@keyframes spin { to { transform: rotate(360deg); } }
@keyframes tag-rise { from { opacity: 0; transform: translateY(3px); } }
@media (max-width: 980px) { .batch-bar { align-items: flex-start; flex-direction: column; } .batch-actions { width: 100%; flex-wrap: wrap; } }
@media (max-width: 820px) { .library-workspace { padding: 34px 22px 110px; } .library-header { align-items: flex-start; flex-direction: column; } .library-toolbar { align-items: stretch; flex-direction: column; } .search-field { min-width: 0; } .selection-tools { width: 100%; } .selection-tools .select-all-action { flex: 1; justify-content: center; } .problem-grid { grid-template-columns: 1fr; } .batch-actions .start-review-action, .batch-actions .start-exam-action { flex: 1 0 calc(50% - 4px); justify-content: center; } }
@media (prefers-reduced-motion: reduce) { .loading-state span, .spin, .problem-tags span { animation: none; } .primary-action, .batch-bar button, .deck-dock-enter-active, .deck-dock-leave-active, .problem-card, .problem-preview { transition: none; } .deck-dock-enter-from, .deck-dock-leave-to { transform: none; } .problem-preview:hover { transform: none; } }
</style>
