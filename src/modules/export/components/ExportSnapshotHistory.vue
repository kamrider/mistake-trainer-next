<script setup lang="ts">
import { FileDown, Layers3, RotateCcw, Trash2 } from '@lucide/vue'
import type { DeletedExportSnapshotSummary, ExportLayout, ExportSnapshotSummary } from '../../../shared/api/bindings'

defineProps<{
  snapshots: ExportSnapshotSummary[]
  deletedSnapshots: DeletedExportSnapshotSummary[]
  snapshotsLoaded: boolean
  trashLoaded: boolean
  generatingId: string
  deletingId: string
  restoringId: string
}>()

const emit = defineEmits<{
  generate: [snapshot: ExportSnapshotSummary]
  delete: [snapshotId: string]
  restore: [snapshot: DeletedExportSnapshotSummary]
}>()

function snapshotDate(value: number | null) {
  return value == null ? '—' : new Date(value).toLocaleString('zh-CN')
}

function layoutLabel(value: ExportLayout) {
  return {
    question_answer_alternating: '题答交替',
    questions_then_answers: '题目后集中答案',
    original_image_folder: '原图文件夹',
  }[value]
}
</script>

<template>
  <section class="history-panel">
    <header class="history-heading">
      <div>
        <p>可重复生成</p>
        <h3>导出快照</h3>
        <span>快照保存题目顺序与排版，不保存可重新生成的文件。</span>
      </div>
      <Layers3
        :size="22"
        aria-hidden="true"
      />
    </header>

    <TransitionGroup
      v-if="snapshots.length"
      name="snapshot-row"
      tag="ul"
      class="snapshot-list"
    >
      <li
        v-for="snapshot in snapshots"
        :key="snapshot.id"
        class="snapshot-card"
      >
        <span class="snapshot-seal">{{ snapshot.problemCount }}</span>
        <span class="snapshot-copy">
          <strong>{{ snapshot.title }}</strong>
          <small>{{ snapshot.problemCount }} 题 · {{ layoutLabel(snapshot.layout) }}</small>
          <time :datetime="new Date(snapshot.createdAtUtcMs ?? 0).toISOString()">{{ snapshotDate(snapshot.createdAtUtcMs) }}</time>
        </span>
        <span class="snapshot-actions">
          <button
            type="button"
            class="generate-action"
            :aria-label="generatingId === snapshot.id ? `正在生成：${snapshot.title}` : `生成导出文件：${snapshot.title}`"
            :disabled="Boolean(generatingId)"
            @click="emit('generate', snapshot)"
          >
            <FileDown :size="16" />{{ generatingId === snapshot.id ? '生成中…' : '生成文件' }}
          </button>
          <button
            type="button"
            class="delete-action"
            :aria-label="`删除导出快照：${snapshot.title}`"
            :disabled="deletingId === snapshot.id"
            @click="emit('delete', snapshot.id)"
          >
            <Trash2 :size="16" />
          </button>
        </span>
      </li>
    </TransitionGroup>
    <p
      v-else-if="snapshotsLoaded"
      class="history-state"
    >
      还没有保存过导出快照。
    </p>
    <p
      v-else
      class="history-state"
    >
      正在读取导出快照…
    </p>

    <section class="trash-panel">
      <header>
        <div><p>30 天回收区</p><h4>已删除快照</h4></div>
        <span>到期前可以恢复，原题不会受到影响。</span>
      </header>
      <TransitionGroup
        v-if="deletedSnapshots.length"
        name="snapshot-row"
        tag="ul"
        class="snapshot-list trash-list"
      >
        <li
          v-for="deleted in deletedSnapshots"
          :key="deleted.snapshot.id"
          class="snapshot-card deleted-card"
        >
          <span class="snapshot-seal">{{ deleted.snapshot.problemCount }}</span>
          <span class="snapshot-copy">
            <strong>{{ deleted.snapshot.title }}</strong>
            <small>{{ layoutLabel(deleted.snapshot.layout) }}</small>
            <time :datetime="new Date(deleted.purgeAfterUtcMs ?? 0).toISOString()">删除于 {{ snapshotDate(deleted.deletedAtUtcMs) }} · 保留至 {{ snapshotDate(deleted.purgeAfterUtcMs) }}</time>
          </span>
          <button
            type="button"
            class="restore-action"
            :aria-label="`恢复导出快照：${deleted.snapshot.title}`"
            :disabled="restoringId === deleted.snapshot.id"
            @click="emit('restore', deleted)"
          >
            <RotateCcw :size="15" />{{ restoringId === deleted.snapshot.id ? '恢复中…' : '恢复' }}
          </button>
        </li>
      </TransitionGroup>
      <p
        v-else-if="trashLoaded"
        class="history-state compact"
      >
        回收区为空。
      </p>
      <p
        v-else
        class="history-state compact"
      >
        正在读取回收区…
      </p>
    </section>
  </section>
</template>

<style scoped>
.history-panel { margin-top: 18px; padding-top: 20px; border-top: 1px solid var(--line); }
.history-heading,
.trash-panel > header { display: flex; gap: 18px; align-items: flex-start; justify-content: space-between; }
.history-heading p,
.trash-panel header p { margin: 0 0 5px; color: var(--cinnabar); font-size: 10px; font-weight: 780; letter-spacing: .12em; }
.history-heading h3,
.trash-panel h4 { margin: 0; color: var(--green-deep); font-family: var(--font-serif); }
.history-heading h3 { font-size: 19px; }
.trash-panel h4 { font-size: 16px; }
.history-heading span,
.trash-panel header span { display: block; margin-top: 6px; color: var(--ink-muted); font-size: 10px; }
.snapshot-list { display: grid; gap: 9px; margin: 14px 0 0; padding: 0; list-style: none; }
.snapshot-card { position: relative; display: grid; min-height: 76px; grid-template-columns: auto minmax(0, 1fr) auto; gap: 12px; align-items: center; padding: 12px 14px; border: 1px solid var(--line); border-radius: 4px 13px 13px; background: rgba(255, 253, 247, .82); box-shadow: 3px 3px 0 rgba(232, 221, 199, .52); transition: transform var(--motion-feedback) var(--ease-standard), box-shadow var(--motion-standard) var(--ease-standard); }
.snapshot-card:hover { box-shadow: 5px 7px 0 rgba(232, 221, 199, .46); transform: translateY(-2px); }
.snapshot-seal { display: grid; width: 38px; height: 38px; place-items: center; color: var(--paper); border-radius: 12px 3px 12px 12px; background: var(--green-deep); font-family: var(--font-serif); font-size: 13px; }
.snapshot-copy { min-width: 0; }
.snapshot-copy strong,
.snapshot-copy small,
.snapshot-copy time { display: block; overflow: hidden; text-overflow: ellipsis; white-space: nowrap; }
.snapshot-copy strong { color: var(--ink); font-size: 13px; }
.snapshot-copy small { margin-top: 4px; color: var(--green-deep); font-size: 10px; }
.snapshot-copy time { margin-top: 3px; color: var(--ink-muted); font-size: 9px; }
.snapshot-actions { display: flex; gap: 7px; align-items: center; }
button { display: inline-flex; gap: 7px; align-items: center; justify-content: center; min-height: 36px; padding: 0 11px; border: 1px solid var(--line); border-radius: 999px; background: var(--paper-raised); cursor: pointer; font-size: 11px; font-weight: 700; }
button:disabled { cursor: wait; opacity: .45; }
.generate-action { color: var(--green-deep); }
.delete-action { width: 36px; padding: 0; color: var(--cinnabar); }
.trash-panel { margin-top: 20px; padding-top: 18px; border-top: 1px dashed var(--line); }
.trash-panel > header { align-items: end; }
.deleted-card { background: rgba(246, 241, 231, .5); box-shadow: none; }
.deleted-card .snapshot-seal { color: var(--ink-muted); background: var(--sand); }
.restore-action { color: var(--green-deep); }
.history-state { display: grid; min-height: 90px; margin: 12px 0 0; place-items: center; color: var(--ink-muted); border: 1px dashed var(--line); border-radius: 12px; font-size: 11px; }
.history-state.compact { min-height: 58px; }
.snapshot-row-enter-active,
.snapshot-row-leave-active { transition: opacity var(--motion-standard) var(--ease-standard), transform var(--motion-standard) var(--ease-standard); }
.snapshot-row-enter-from,
.snapshot-row-leave-to { opacity: 0; transform: translateY(7px) scale(.99); }

@media (max-width: 700px) {
  .snapshot-card { grid-template-columns: auto minmax(0, 1fr); }
  .snapshot-actions,
  .restore-action { grid-column: 2; justify-self: start; }
  .trash-panel > header { align-items: flex-start; flex-direction: column; }
}

@media (prefers-reduced-motion: reduce) {
  .snapshot-card,
  .snapshot-row-enter-active,
  .snapshot-row-leave-active { transition: none; }
  .snapshot-card:hover,
  .snapshot-row-enter-from,
  .snapshot-row-leave-to { transform: none; }
}
</style>
