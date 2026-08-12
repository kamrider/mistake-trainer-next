<script setup lang="ts">
import { ChevronDown, SlidersHorizontal, X } from '@lucide/vue'
import { computed, ref } from 'vue'
import type { ProblemAnswerState, ProblemReviewState } from '../../../shared/api/bindings'
import {
  EMPTY_LIBRARY_FILTERS,
  hasLibraryFilters,
  type LibraryAdvancedFilters,
} from '../domain/libraryFilters'

const props = withDefaults(defineProps<{
  modelValue: LibraryAdvancedFilters
  subjectOptions?: string[]
  tagOptions?: string[]
  disabled?: boolean
}>(), {
  subjectOptions: () => [],
  tagOptions: () => [],
  disabled: false,
})

const emit = defineEmits<{
  'update:modelValue': [filters: LibraryAdvancedFilters]
}>()

const expanded = ref(false)
const active = computed(() => hasLibraryFilters(props.modelValue))
const reviewLabels: Record<ProblemReviewState, string> = {
  any: '全部复习状态',
  never_reviewed: '从未复习',
  due: '已经到期',
  recently_forgotten: '最近遗忘',
}
const answerLabels: Record<ProblemAnswerState, string> = {
  any: '全部答案状态',
  has_answer: '已有答案',
  missing_answer: '缺少答案',
}

function update(patch: Partial<LibraryAdvancedFilters>) {
  emit('update:modelValue', { ...props.modelValue, ...patch })
}

function toggleList(key: 'subjects' | 'tags', value: string) {
  const current = props.modelValue[key]
  update({ [key]: current.includes(value) ? current.filter(item => item !== value) : [...current, value] })
}

function clear() {
  emit('update:modelValue', {
    ...EMPTY_LIBRARY_FILTERS,
    subjects: [],
    tags: [],
  })
}
</script>

<template>
  <div class="advanced-filter">
    <button
      type="button"
      class="filter-trigger"
      :class="{ active }"
      :aria-expanded="expanded"
      aria-controls="library-advanced-filter-panel"
      :disabled="disabled"
      @click="expanded = !expanded"
    >
      <SlidersHorizontal
        :size="16"
        aria-hidden="true"
      />
      更多筛选
      <span
        v-if="active"
        class="active-dot"
        aria-label="已有筛选"
      />
      <ChevronDown
        :size="15"
        aria-hidden="true"
      />
    </button>

    <div
      v-if="active"
      class="active-filters"
      aria-label="当前高级筛选"
    >
      <button
        v-for="subject in modelValue.subjects"
        :key="`subject-${subject}`"
        type="button"
        :disabled="disabled"
        :aria-label="`移除科目 ${subject}`"
        @click="toggleList('subjects', subject)"
      >
        科目：{{ subject }} <X
          :size="13"
          aria-hidden="true"
        />
      </button>
      <button
        v-for="tag in modelValue.tags"
        :key="`tag-${tag}`"
        type="button"
        :disabled="disabled"
        :aria-label="`移除标签 ${tag}`"
        @click="toggleList('tags', tag)"
      >
        标签：{{ tag }} <X
          :size="13"
          aria-hidden="true"
        />
      </button>
      <button
        v-if="modelValue.reviewState !== 'any'"
        type="button"
        :disabled="disabled"
        aria-label="移除复习状态筛选"
        @click="update({ reviewState: 'any' })"
      >
        {{ reviewLabels[modelValue.reviewState] }} <X
          :size="13"
          aria-hidden="true"
        />
      </button>
      <button
        v-if="modelValue.answerState !== 'any'"
        type="button"
        :disabled="disabled"
        aria-label="移除答案状态筛选"
        @click="update({ answerState: 'any' })"
      >
        {{ answerLabels[modelValue.answerState] }} <X
          :size="13"
          aria-hidden="true"
        />
      </button>
      <button
        type="button"
        class="clear-filter"
        :disabled="disabled"
        @click="clear"
      >
        清除全部筛选
      </button>
    </div>

    <section
      v-if="expanded"
      id="library-advanced-filter-panel"
      class="filter-panel"
      aria-label="高级筛选条件"
    >
      <fieldset v-if="subjectOptions.length">
        <legend>科目（可多选）</legend>
        <label
          v-for="subject in subjectOptions"
          :key="subject"
        >
          <input
            type="checkbox"
            :checked="modelValue.subjects.includes(subject)"
            :disabled="disabled"
            @change="toggleList('subjects', subject)"
          >{{ subject }}
        </label>
      </fieldset>
      <fieldset v-if="tagOptions.length">
        <legend>标签（可多选）</legend>
        <label
          v-for="tag in tagOptions"
          :key="tag"
        >
          <input
            type="checkbox"
            :checked="modelValue.tags.includes(tag)"
            :disabled="disabled"
            @change="toggleList('tags', tag)"
          >{{ tag }}
        </label>
      </fieldset>
      <label>
        复习状态
        <select
          :value="modelValue.reviewState"
          :disabled="disabled"
          @change="update({ reviewState: ($event.target as HTMLSelectElement).value as ProblemReviewState })"
        >
          <option
            v-for="(label, value) in reviewLabels"
            :key="value"
            :value="value"
          >{{ label }}</option>
        </select>
      </label>
      <label>
        答案状态
        <select
          :value="modelValue.answerState"
          :disabled="disabled"
          @change="update({ answerState: ($event.target as HTMLSelectElement).value as ProblemAnswerState })"
        >
          <option
            v-for="(label, value) in answerLabels"
            :key="value"
            :value="value"
          >{{ label }}</option>
        </select>
      </label>
    </section>
  </div>
</template>

<style scoped>
.advanced-filter { position: relative; display: grid; gap: 9px; }
.filter-trigger { display: inline-flex; min-height: 44px; gap: 7px; align-items: center; padding: 0 13px; color: var(--green-deep); border: 1px solid var(--line); border-radius: 999px; background: rgba(255,253,247,.7); cursor: pointer; }
.filter-trigger.active { border-color: rgba(185,88,63,.42); background: #fff6ed; }
.active-dot { width: 7px; height: 7px; border-radius: 50%; background: var(--cinnabar); }
.active-filters { display: flex; width: max-content; max-width: min(680px,80vw); gap: 6px; flex-wrap: wrap; justify-content: flex-end; }
.active-filters button { display: inline-flex; min-height: 32px; gap: 4px; align-items: center; padding: 0 9px; color: var(--green-deep); border: 1px solid rgba(33,51,45,.14); border-radius: 999px; background: var(--green-soft); cursor: pointer; }
.active-filters .clear-filter { color: #7f3829; background: #fff6ed; }
.filter-panel { position: absolute; z-index: 20; top: calc(100% + 8px); right: 0; display: grid; width: min(560px,calc(100vw - 44px)); grid-template-columns: repeat(2,minmax(0,1fr)); gap: 18px; padding: 18px; border: 1px solid var(--line); border-radius: 16px 4px 16px 16px; background: #fffdf7; box-shadow: 0 18px 46px rgba(34,48,43,.16); }
fieldset { display: flex; max-height: 160px; gap: 7px 13px; flex-wrap: wrap; padding: 0; overflow: auto; border: 0; }
legend { width: 100%; margin-bottom: 8px; font-size: 12px; font-weight: 760; }
label { display: flex; gap: 7px; align-items: center; color: var(--ink-muted); font-size: 13px; }
select { min-height: 40px; padding: 0 10px; border: 1px solid var(--line); border-radius: 9px; background: white; }
@media (max-width: 700px) { .filter-panel { grid-template-columns: 1fr; } .active-filters { max-width: 100%; justify-content: flex-start; } }
</style>
