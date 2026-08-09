<script setup lang="ts">
import { CheckCircle2, Cloud, GitCompareArrows, Laptop, RefreshCw, TriangleAlert } from '@lucide/vue'
import { computed, inject, nextTick, onMounted, ref } from 'vue'
import { syncControllerKey } from '../../../app/sync-controller'
import {
  commands,
  type JsonValue,
  type SyncConflictChoice,
  type SyncConflictSummary,
} from '../../../shared/api/bindings'
import { normalizeAppResult } from '../../../shared/api/normalize-result'
import { useSyncConflictOperations } from '../composables/useSyncConflictOperations'
import SyncConflictBulkDialog from './SyncConflictBulkDialog.vue'

interface ConflictGroup {
  key: string
  entityType: string
  entityId: string
  entityLabel: string
  conflicts: SyncConflictSummary[]
}

interface PendingGroupResolution {
  group: ConflictGroup
  choice: SyncConflictChoice
}

type NativeJsonValue =
  null | boolean | number | string | NativeJsonValue[] | { [key: string]: NativeJsonValue }

const emit = defineEmits<{
  changed: []
}>()
const syncController = inject(syncControllerKey, undefined)

const {
  conflicts,
  loading,
  busyKey,
  errorMessage,
  statusMessage,
  reload,
  resolve,
} = useSyncConflictOperations({
  listConflicts: async () => normalizeAppResult(await commands.syncConflictList()),
  scheduleSync: () => syncController?.scheduleMutation(),
  onChanged: () => emit('changed'),
})
const pendingGroupResolution = ref<PendingGroupResolution>()
const centerElement = ref<HTMLElement>()
const statusElement = ref<HTMLElement>()

const groups = computed<ConflictGroup[]>(() => {
  const grouped = new Map<string, ConflictGroup>()
  for (const conflict of conflicts.value) {
    const key = `${conflict.entityType}:${conflict.entityId}`
    const existing = grouped.get(key)
    if (existing) existing.conflicts.push(conflict)
    else {
      grouped.set(key, {
        key,
        entityType: conflict.entityType,
        entityId: conflict.entityId,
        entityLabel: conflict.entityLabel,
        conflicts: [conflict],
      })
    }
  }
  return [...grouped.values()]
})

const entityTypeLabels: Record<string, string> = {
  learner_profile: '学习档案',
  problem: '错题',
  export_snapshot: '导出批次',
}

const fieldLabels: Record<string, string> = {
  __deleted__: '删除状态',
  name: '档案名称',
  subject: '科目',
  tags: '标签',
  note: '笔记',
  status: '题目状态',
  timeLimitSeconds: '答题时限',
  assets: '题目与答案图片',
  title: '导出名称',
  problemIds: '导出题目',
  configuration: '导出配置',
}

const statusLabels: Record<string, string> = {
  active: '活动题',
  archived: '已归档',
  trashed: '回收站',
}

function fieldLabel(fieldName: string) {
  return fieldLabels[fieldName] ?? '同步内容'
}

function entityTypeLabel(entityType: string) {
  return entityTypeLabels[entityType] ?? '同步内容'
}

function unwrapJsonValue(value: JsonValue): NativeJsonValue {
  switch (value.kind) {
    case 'null': return null
    case 'bool':
    case 'string': return value.value
    case 'number': return Number(value.value)
    case 'array': return value.value.map(unwrapJsonValue)
    case 'object': return Object.fromEntries(
      Object.entries(value.value).map(([key, item]) => [key, unwrapJsonValue(item)]),
    )
  }
}

function valueKind(fieldName: string, source: JsonValue) {
  const value = unwrapJsonValue(source)
  if (fieldName === '__deleted__') return 'deletion'
  if (Array.isArray(value)) return 'array'
  if (value !== null && typeof value === 'object') return 'object'
  return 'scalar'
}

function scalarValue(fieldName: string, source: JsonValue, side: SyncConflictChoice) {
  const value = unwrapJsonValue(source)
  if (fieldName === '__deleted__') return side === 'remote' ? '云端已删除' : '保留本机内容'
  if (value === null || value === '') return '未设置'
  if (fieldName === 'status' && typeof value === 'string') return statusLabels[value] ?? value
  if (fieldName === 'timeLimitSeconds' && typeof value === 'number') return `${value} 秒`
  if (typeof value === 'boolean') return value ? '是' : '否'
  if (typeof value === 'string' || typeof value === 'number') return String(value)
  return ''
}

function arrayValues(source: JsonValue) {
  const value = unwrapJsonValue(source)
  return Array.isArray(value) ? value.map(item => summarizeValue(item)) : []
}

function arrayEmptyLabel(fieldName: string) {
  if (fieldName === 'tags') return '无标签'
  if (fieldName === 'assets') return '没有图片'
  if (fieldName === 'problemIds') return '没有题目'
  return '空列表'
}

function summarizeValue(value: NativeJsonValue): string {
  if (value === null) return '未设置'
  if (typeof value === 'string' || typeof value === 'number') return String(value)
  if (typeof value === 'boolean') return value ? '是' : '否'
  if (Array.isArray(value)) return `${value.length} 项`
  const role = typeof value.role === 'string'
    ? value.role === 'question' ? '题图' : value.role === 'answer' ? '答案图' : value.role
    : ''
  if (role) return role
  return Object.entries(value)
    .slice(0, 3)
    .map(([key, item]) => `${fieldLabels[key] ?? key}：${summarizeValue(item)}`)
    .join(' · ') || '空内容'
}

function objectRows(source: JsonValue) {
  const value = unwrapJsonValue(source)
  if (value === null || Array.isArray(value) || typeof value !== 'object') return []
  return Object.entries(value)
    .slice(0, 8)
    .map(([key, item]) => ({
      key,
      label: fieldLabels[key] ?? key,
      value: summarizeValue(item),
    }))
}

async function restoreFocus(preferredGroupKey: string) {
  try {
    await nextTick()
    const preferred = centerElement.value?.querySelector<HTMLElement>(
      `[data-conflict-group="${CSS.escape(preferredGroupKey)}"] button:not(:disabled)`,
    )
    const first = centerElement.value?.querySelector<HTMLElement>('.conflict-card button:not(:disabled)')
    ;(preferred ?? first ?? statusElement.value)?.focus()
  }
  catch {
    // Focus recovery is best effort and must not change the durable resolution result.
  }
}

async function focusGroupAction(groupKey: string, choice: SyncConflictChoice) {
  try {
    await nextTick()
    centerElement.value?.querySelector<HTMLElement>(
      `[data-conflict-group="${CSS.escape(groupKey)}"] [data-choice="${choice}"]`,
    )?.focus()
  }
  catch {
    // The operation result remains authoritative if the focus target disappeared.
  }
}

function groupIncludesRemoteDeletion(group: ConflictGroup) {
  return group.conflicts.some(conflict => conflict.fieldName === '__deleted__')
}

function requestGroupResolution(group: ConflictGroup, choice: SyncConflictChoice) {
  if (loading.value || busyKey.value) return
  pendingGroupResolution.value = { group, choice }
}

async function cancelGroupResolution() {
  const pending = pendingGroupResolution.value
  if (!pending) return
  pendingGroupResolution.value = undefined
  await focusGroupAction(pending.group.key, pending.choice)
}

async function confirmGroupResolution() {
  const pending = pendingGroupResolution.value
  if (!pending) return
  pendingGroupResolution.value = undefined
  await resolveGroup(pending.group, pending.choice)
}

async function resolveField(conflict: SyncConflictSummary, choice: SyncConflictChoice) {
  const operationKey = `field:${conflict.id}`
  const groupKey = `${conflict.entityType}:${conflict.entityId}`
  const resolved = await resolve(
    operationKey,
    async () => normalizeAppResult(await commands.syncConflictResolve({
      conflictId: conflict.id,
      choice,
    })),
    `${fieldLabel(conflict.fieldName)}已采用${choice === 'local' ? '本机' : '云端'}版本。`,
  )
  if (resolved) await restoreFocus(groupKey)
}

async function resolveGroup(group: ConflictGroup, choice: SyncConflictChoice) {
  const operationKey = `group:${group.key}`
  const resolved = await resolve(
    operationKey,
    async () => normalizeAppResult(await commands.syncConflictResolveEntity({
      entityType: group.entityType,
      entityId: group.entityId,
      choice,
    })),
    `${group.entityLabel}已全部采用${choice === 'local' ? '本机' : '云端'}版本。`,
  )
  if (resolved) await restoreFocus(group.key)
  else await focusGroupAction(group.key, choice)
}

defineExpose({ reload })
onMounted(reload)
</script>

<template>
  <section
    ref="centerElement"
    class="conflict-center"
    aria-labelledby="sync-conflict-title"
    :aria-busy="loading || Boolean(busyKey)"
  >
    <header>
      <div>
        <p>同步冲突 · 需要你的判断</p>
        <h2 id="sync-conflict-title">
          本机和云端改了同一处内容
        </h2>
        <span>不同字段已自动合并。这里只有无法安全猜测的内容；每次选择都会保留审计记录。</span>
      </div>
      <button
        type="button"
        :disabled="loading || Boolean(busyKey)"
        aria-label="重新读取同步冲突"
        @click="reload"
      >
        <RefreshCw :size="16" />
        {{ loading ? '读取中…' : '刷新' }}
      </button>
    </header>

    <p
      v-if="errorMessage"
      class="conflict-message error"
      role="alert"
    >
      <TriangleAlert :size="16" />
      {{ errorMessage }}
    </p>
    <p
      v-if="statusMessage"
      ref="statusElement"
      class="conflict-message success"
      role="status"
      tabindex="-1"
    >
      <CheckCircle2 :size="16" />
      {{ statusMessage }}
    </p>

    <div
      v-if="loading && !conflicts.length"
      class="conflict-empty"
      role="status"
    >
      正在核对本机和云端记录…
    </div>

    <TransitionGroup
      v-else-if="groups.length"
      name="conflict-card"
      tag="div"
      class="conflict-list"
    >
      <article
        v-for="group in groups"
        :key="group.key"
        class="conflict-card"
        :data-conflict-group="group.key"
        :aria-label="`${group.entityLabel}的同步冲突`"
      >
        <header class="conflict-card-header">
          <div>
            <span class="entity-type">{{ entityTypeLabel(group.entityType) }}</span>
            <h3>{{ group.entityLabel }}</h3>
            <small>{{ group.conflicts.length }} 处需要确认</small>
          </div>
          <GitCompareArrows :size="22" />
        </header>

        <div class="field-list">
          <section
            v-for="conflict in group.conflicts"
            :key="conflict.id"
            class="field-conflict"
            role="group"
            :aria-label="`${fieldLabel(conflict.fieldName)}冲突`"
          >
            <h4>{{ fieldLabel(conflict.fieldName) }}</h4>
            <div class="version-grid">
              <article class="version-panel local">
                <header><Laptop :size="15" /><strong>本机版本</strong></header>
                <p
                  v-if="valueKind(conflict.fieldName, conflict.localValue) === 'scalar'
                    || valueKind(conflict.fieldName, conflict.localValue) === 'deletion'"
                  class="scalar-value"
                >
                  {{ scalarValue(conflict.fieldName, conflict.localValue, 'local') }}
                </p>
                <div
                  v-else-if="valueKind(conflict.fieldName, conflict.localValue) === 'array'"
                  class="value-chips"
                >
                  <span
                    v-for="(value, index) in arrayValues(conflict.localValue)"
                    :key="`${value}-${index}`"
                  >{{ value }}</span>
                  <em v-if="!arrayValues(conflict.localValue).length">{{ arrayEmptyLabel(conflict.fieldName) }}</em>
                </div>
                <dl
                  v-else
                  class="object-value"
                >
                  <div
                    v-for="row in objectRows(conflict.localValue)"
                    :key="row.key"
                  >
                    <dt>{{ row.label }}</dt><dd>{{ row.value }}</dd>
                  </div>
                </dl>
                <button
                  type="button"
                  :disabled="loading || Boolean(busyKey)"
                  :aria-label="`${fieldLabel(conflict.fieldName)}采用本机版本`"
                  @click="resolveField(conflict, 'local')"
                >
                  保留本机
                </button>
              </article>

              <article class="version-panel remote">
                <header><Cloud :size="15" /><strong>云端版本</strong></header>
                <p
                  v-if="valueKind(conflict.fieldName, conflict.remoteValue) === 'scalar'
                    || valueKind(conflict.fieldName, conflict.remoteValue) === 'deletion'"
                  class="scalar-value"
                >
                  {{ scalarValue(conflict.fieldName, conflict.remoteValue, 'remote') }}
                </p>
                <div
                  v-else-if="valueKind(conflict.fieldName, conflict.remoteValue) === 'array'"
                  class="value-chips"
                >
                  <span
                    v-for="(value, index) in arrayValues(conflict.remoteValue)"
                    :key="`${value}-${index}`"
                  >{{ value }}</span>
                  <em v-if="!arrayValues(conflict.remoteValue).length">{{ arrayEmptyLabel(conflict.fieldName) }}</em>
                </div>
                <dl
                  v-else
                  class="object-value"
                >
                  <div
                    v-for="row in objectRows(conflict.remoteValue)"
                    :key="row.key"
                  >
                    <dt>{{ row.label }}</dt><dd>{{ row.value }}</dd>
                  </div>
                </dl>
                <button
                  type="button"
                  :disabled="loading || Boolean(busyKey)"
                  :aria-label="`${fieldLabel(conflict.fieldName)}采用云端版本`"
                  @click="resolveField(conflict, 'remote')"
                >
                  采用云端
                </button>
              </article>
            </div>
          </section>
        </div>

        <footer class="group-actions">
          <span>整条内容统一选择</span>
          <button
            type="button"
            data-choice="local"
            :disabled="loading || Boolean(busyKey)"
            :aria-label="`${group.entityLabel}全部采用本机版本`"
            @click="requestGroupResolution(group, 'local')"
          >
            全部保留本机
          </button>
          <button
            type="button"
            class="remote-all"
            data-choice="remote"
            :disabled="loading || Boolean(busyKey)"
            :aria-label="`${group.entityLabel}全部采用云端版本`"
            @click="requestGroupResolution(group, 'remote')"
          >
            全部采用云端
          </button>
        </footer>
      </article>
    </TransitionGroup>

    <div
      v-else
      class="conflict-empty resolved"
      role="status"
    >
      <CheckCircle2 :size="20" />
      <span>
        <strong>同步内容没有待处理冲突</strong>
        <small>本机可以继续离线编辑；下次同步仍会自动合并不同字段。</small>
      </span>
    </div>

    <SyncConflictBulkDialog
      v-if="pendingGroupResolution"
      :entity-label="pendingGroupResolution.group.entityLabel"
      :conflict-count="pendingGroupResolution.group.conflicts.length"
      :choice="pendingGroupResolution.choice"
      :includes-remote-deletion="groupIncludesRemoteDeletion(pendingGroupResolution.group)"
      @cancel="cancelGroupResolution"
      @confirm="confirmGroupResolution"
    />
  </section>
</template>

<style scoped>
.conflict-center {
  margin-top: 16px;
  padding: 26px;
  border: 1px solid rgba(185, 88, 63, .24);
  border-radius: 17px;
  background:
    linear-gradient(135deg, rgba(255, 253, 247, .96), rgba(247, 235, 220, .38)),
    repeating-linear-gradient(0deg, transparent 0 27px, rgba(33, 51, 45, .025) 28px);
  box-shadow: 0 16px 48px rgba(34, 48, 43, .06);
}
.conflict-center > header {
  display: flex;
  justify-content: space-between;
  gap: 24px;
  align-items: flex-start;
  margin-bottom: 18px;
}
.conflict-center > header p {
  margin: 0 0 8px;
  color: var(--cinnabar);
  font-size: 12px;
  font-weight: 800;
  letter-spacing: .14em;
}
.conflict-center h2,
.conflict-card h3 {
  margin: 0;
  color: var(--green-deep);
  font-family: Georgia, 'Microsoft YaHei', serif;
}
.conflict-center > header span {
  display: block;
  max-width: 720px;
  margin-top: 8px;
  color: var(--ink-muted);
  font-size: 12px;
  line-height: 1.65;
}
button {
  display: inline-flex;
  min-height: 44px;
  gap: 7px;
  align-items: center;
  justify-content: center;
  padding: 9px 13px;
  border: 1px solid var(--line);
  border-radius: 10px;
  background: var(--paper-raised);
  color: var(--green-deep);
  cursor: pointer;
  transition: transform var(--motion-feedback) var(--ease-standard);
}
button:hover:not(:disabled) {
  transform: translateY(-1px);
  border-color: rgba(33, 51, 45, .42);
  box-shadow: 0 8px 18px rgba(34, 48, 43, .08);
}
button:disabled {
  cursor: wait;
  opacity: .5;
}
.conflict-message {
  display: flex;
  gap: 8px;
  align-items: center;
  margin: 0 0 14px;
  padding: 11px 13px;
  border-radius: 10px;
  font-size: 12px;
}
.conflict-message.error {
  color: #843d2c;
  background: rgba(185, 88, 63, .09);
}
.conflict-message.success {
  color: #486455;
  background: rgba(85, 114, 99, .1);
}
.conflict-list {
  position: relative;
  display: grid;
  gap: 14px;
}
.conflict-card {
  overflow: hidden;
  border: 1px solid rgba(33, 51, 45, .18);
  border-radius: 8px 18px 18px;
  background: rgba(255, 253, 247, .88);
  box-shadow: 0 12px 30px rgba(34, 48, 43, .06);
}
.conflict-card-header {
  display: flex;
  justify-content: space-between;
  gap: 18px;
  align-items: center;
  padding: 18px 20px;
  border-bottom: 1px solid var(--line);
  background: rgba(232, 221, 199, .25);
}
.entity-type {
  color: var(--cinnabar);
  font-size: 12px;
  font-weight: 800;
  letter-spacing: .12em;
}
.conflict-card h3 {
  margin-top: 4px;
  font-size: 20px;
}
.conflict-card-header small {
  display: block;
  margin-top: 4px;
  color: var(--ink-muted);
  font-size: 12px;
}
.conflict-card-header > svg {
  color: var(--cinnabar);
}
.field-list {
  display: grid;
}
.field-conflict {
  padding: 18px 20px;
  border-bottom: 1px solid var(--line);
}
.field-conflict h4 {
  margin: 0 0 11px;
  color: var(--green-deep);
  font-size: 13px;
}
.version-grid {
  display: grid;
  grid-template-columns: repeat(2, minmax(0, 1fr));
  gap: 11px;
}
.version-panel {
  display: grid;
  grid-template-rows: auto minmax(52px, auto) auto;
  gap: 10px;
  min-width: 0;
  padding: 13px;
  border: 1px solid var(--line);
  border-radius: 11px;
}
.version-panel.local {
  border-color: rgba(33, 51, 45, .22);
  background: rgba(220, 228, 220, .25);
}
.version-panel.remote {
  border-color: rgba(185, 88, 63, .2);
  background: rgba(247, 225, 216, .25);
}
.version-panel > header {
  display: flex;
  gap: 7px;
  align-items: center;
  color: var(--ink-muted);
  font-size: 12px;
}
.version-panel.remote > header {
  color: #8d4635;
}
.scalar-value {
  margin: 0;
  color: var(--ink);
  font-size: 13px;
  line-height: 1.7;
  overflow-wrap: anywhere;
  white-space: pre-wrap;
}
.value-chips {
  display: flex;
  gap: 6px;
  align-content: start;
  flex-wrap: wrap;
}
.value-chips span {
  height: fit-content;
  padding: 5px 8px;
  border-radius: 999px;
  background: rgba(33, 51, 45, .09);
  color: var(--green-deep);
  font-size: 12px;
}
.value-chips em {
  color: var(--ink-muted);
  font-size: 12px;
  font-style: normal;
}
.object-value {
  display: grid;
  gap: 5px;
  margin: 0;
}
.object-value div {
  display: grid;
  grid-template-columns: minmax(72px, .35fr) 1fr;
  gap: 8px;
}
.object-value dt,
.object-value dd {
  margin: 0;
  font-size: 12px;
  line-height: 1.5;
  overflow-wrap: anywhere;
}
.object-value dt {
  color: var(--ink-muted);
}
.object-value dd {
  color: var(--ink);
}
.version-panel > button {
  width: 100%;
  color: var(--paper);
  border-color: var(--green-deep);
  background: var(--green-deep);
}
.version-panel.remote > button {
  border-color: var(--cinnabar);
  background: var(--cinnabar);
}
.group-actions {
  display: flex;
  gap: 8px;
  align-items: center;
  justify-content: flex-end;
  padding: 14px 20px;
}
.group-actions span {
  margin-right: auto;
  color: var(--ink-muted);
  font-size: 12px;
}
.group-actions .remote-all {
  color: #8d4635;
  border-color: rgba(185, 88, 63, .34);
  background: rgba(247, 225, 216, .5);
}
.conflict-empty {
  display: flex;
  gap: 10px;
  align-items: center;
  justify-content: center;
  min-height: 88px;
  padding: 18px;
  color: var(--ink-muted);
  border: 1px dashed var(--line);
  border-radius: 12px;
  background: rgba(255, 253, 247, .55);
  font-size: 12px;
}
.conflict-empty.resolved {
  color: #557263;
}
.conflict-empty.resolved span {
  display: grid;
  gap: 3px;
}
.conflict-empty.resolved small {
  color: var(--ink-muted);
}
.conflict-card-enter-active,
.conflict-card-leave-active {
  transition:
    transform var(--motion-standard) var(--ease-standard),
    opacity var(--motion-standard) var(--ease-standard);
}
.conflict-card-enter-from,
.conflict-card-leave-to {
  transform: translateY(-8px) scale(.985);
  opacity: 0;
}
.conflict-card-leave-active {
  position: absolute;
  width: 100%;
}
@media (max-width: 760px) {
  .conflict-center {
    padding: 20px 16px;
  }
  .conflict-center > header {
    align-items: stretch;
    flex-direction: column;
  }
  .version-grid {
    grid-template-columns: 1fr;
  }
  .group-actions {
    align-items: stretch;
    flex-direction: column;
  }
  .group-actions span {
    margin: 0 0 3px;
  }
}
@media (prefers-reduced-motion: reduce) {
  button,
  .conflict-card-enter-active,
  .conflict-card-leave-active {
    transition: none;
  }
  button:hover:not(:disabled),
  .conflict-card-enter-from,
  .conflict-card-leave-to {
    transform: none;
  }
}
</style>
