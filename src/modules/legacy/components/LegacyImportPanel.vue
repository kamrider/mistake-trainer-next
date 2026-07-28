<script setup lang="ts">
import { listen, type UnlistenFn } from '@tauri-apps/api/event'
import { ArchiveRestore, Clock3, FolderSearch, History, ShieldCheck, TriangleAlert } from '@lucide/vue'
import { computed, inject, nextTick, onMounted, ref } from 'vue'
import { syncControllerKey } from '../../../app/sync-controller'
import {
  commands,
  type AppResult,
  type LegacyImportCandidate,
  type LegacyImportReceipt,
  type LegacyImportSummary,
  type LegacyRollbackReceipt,
} from '../../../shared/api/bindings'
import { normalizeAppResult } from '../../../shared/api/normalize-result'
import LegacyImportDialog from './LegacyImportDialog.vue'
import LegacyImportResult from './LegacyImportResult.vue'

type FallibleInvocation<T> = AppResult<T> | { status: 'ok', data: AppResult<T> } | { status: 'error', error: null }
type LegacyImportPhase = 'validating' | 'encrypting' | 'writing' | 'verifying' | 'completed'
interface LegacyImportProgress {
  candidateId: string
  phase: LegacyImportPhase
  completed: number
  total: number
}

const emit = defineEmits<{ changed: [] }>()
const syncController = inject(syncControllerKey, undefined)
const candidate = ref<LegacyImportCandidate>()
const receipt = ref<LegacyImportReceipt>()
const rollbackReceipt = ref<LegacyRollbackReceipt>()
const imports = ref<LegacyImportSummary[]>([])
const scanning = ref(false)
const importing = ref(false)
const rollingBack = ref(false)
const historyLoading = ref(false)
const errorMessage = ref('')
const dialogMode = ref<'import' | 'rollback'>()
const rollbackTarget = ref<LegacyImportSummary>()
const progress = ref<LegacyImportProgress>()
const scanButton = ref<HTMLButtonElement>()
const confirmButton = ref<HTMLButtonElement>()
const rollbackButton = ref<HTMLButtonElement>()
let unlistenProgress: UnlistenFn | undefined

const canImport = computed(() => Boolean(candidate.value && !candidate.value.report.truncated && candidate.value.problemCount > 0 && candidate.value.report.members > 0))
const progressPercent = computed(() => {
  const value = progress.value
  if (!value) return 0
  const ratio = value.total > 0 ? Math.min(1, Math.max(0, value.completed / value.total)) : 0
  if (value.phase === 'validating') return 4
  if (value.phase === 'encrypting') return 8 + ratio * 37
  if (value.phase === 'writing') return 45 + ratio * 45
  if (value.phase === 'verifying') return 94
  return 100
})
const progressLabel = computed(() => ({
  validating: '正在核对旧目录是否发生变化',
  encrypting: '正在校验并加密图片',
  writing: '正在写入档案、错题与训练记录',
  verifying: '正在做最后一致性校验',
  completed: '迁移完成',
}[progress.value?.phase ?? 'validating']))

function unwrap<T>(value: FallibleInvocation<T>): AppResult<T> {
  if ('status' in value) {
    if (value.status === 'error') throw new Error('command rejected')
    return value.data
  }
  return value
}

function issueLabel(code: string) {
  return {
    missing_metadata: '缺少 metadata', invalid_metadata: 'metadata 损坏', missing_pair: '配对缺失',
    orphan_answer: '孤立答案', invalid_training_date: '训练日期无效', missing_relative_path: '路径缺失',
    unsafe_relative_path: '路径越界', unsafe_metadata_path: '元数据越界', unsafe_member_path: '档案目录越界',
    unsafe_files_root: '图片目录越界', unsafe_asset_path: '资源越界', missing_asset: '图片缺失',
    metadata_too_large: '元数据过大', asset_too_large: '图片过大', scan_limit_exceeded: '扫描已截断',
    hash_mismatch: '哈希不一致', duplicate_asset: '重复图片', unreadable_asset: '图片不可读',
  }[code] ?? code
}

function formatTime(timestamp: number | null) {
  if (timestamp === null) return '时间未知'
  return new Intl.DateTimeFormat('zh-CN', { dateStyle: 'medium', timeStyle: 'short' }).format(new Date(timestamp))
}

async function loadImports() {
  historyLoading.value = true
  try {
    const result = normalizeAppResult(await commands.legacyImportList())
    if (result.ok) imports.value = result.data
  }
  catch {
    // History is secondary; scanning and importing remain available.
  }
  finally {
    historyLoading.value = false
  }
}

async function scanLegacy() {
  if (scanning.value || importing.value) return
  scanning.value = true
  errorMessage.value = ''
  rollbackReceipt.value = undefined
  try {
    const result = normalizeAppResult(unwrap(await commands.legacyScan()))
    if (result.ok && result.data) {
      candidate.value = result.data
      receipt.value = undefined
      progress.value = undefined
    }
    else if (!result.ok) {
      candidate.value = undefined
      errorMessage.value = result.error.userMessage
    }
  }
  catch {
    candidate.value = undefined
    errorMessage.value = '旧版目录没有扫描成功；原目录未被修改，请稍后重试。'
  }
  finally {
    scanning.value = false
  }
}

function openImportDialog() {
  if (!canImport.value) return
  dialogMode.value = 'import'
}

async function closeDialog() {
  if (importing.value || rollingBack.value) return
  const previousMode = dialogMode.value
  dialogMode.value = undefined
  await nextTick()
  if (previousMode === 'import') confirmButton.value?.focus()
  else rollbackButton.value?.focus()
}

async function beginImport() {
  const selected = candidate.value
  if (!selected || importing.value || !canImport.value) return
  dialogMode.value = undefined
  importing.value = true
  errorMessage.value = ''
  progress.value = { candidateId: selected.candidateId, phase: 'validating', completed: 0, total: 1 }
  try {
    unlistenProgress = await listen<LegacyImportProgress>('legacy_import_progress', (event) => {
      if (event.payload.candidateId === selected.candidateId) progress.value = event.payload
    })
    const result = normalizeAppResult(unwrap(await commands.legacyImport(selected.candidateId)))
    if (result.ok) {
      receipt.value = result.data
      candidate.value = undefined
      syncController?.scheduleMutation()
      emit('changed')
      await loadImports()
    }
    else {
      errorMessage.value = result.error.userMessage
    }
  }
  catch {
    errorMessage.value = '迁移没有完成；旧版目录未被修改，新题库已回滚，请稍后重试。'
  }
  finally {
    unlistenProgress?.()
    unlistenProgress = undefined
    importing.value = false
  }
}

function requestRollback(item: LegacyImportSummary, trigger: HTMLButtonElement | null) {
  if (rollingBack.value) return
  rollbackTarget.value = item
  rollbackButton.value = trigger ?? undefined
  dialogMode.value = 'rollback'
}

async function beginRollback() {
  const selected = rollbackTarget.value
  if (!selected || rollingBack.value) return
  rollingBack.value = true
  errorMessage.value = ''
  try {
    const result = normalizeAppResult(unwrap(await commands.legacyRollback(selected.importId)))
    if (result.ok) {
      rollbackReceipt.value = result.data
      dialogMode.value = undefined
      rollbackTarget.value = undefined
      receipt.value = undefined
      syncController?.scheduleMutation()
      emit('changed')
      await loadImports()
    }
    else errorMessage.value = result.error.userMessage
  }
  catch {
    errorMessage.value = '这次撤销没有完成；现有题库保持不变，请稍后重试。'
  }
  finally {
    rollingBack.value = false
  }
}

function restart() {
  receipt.value = undefined
  progress.value = undefined
  errorMessage.value = ''
  nextTick(() => scanButton.value?.focus())
}

onMounted(loadImports)
</script>

<template>
  <section class="legacy-panel">
    <header class="legacy-header">
      <div>
        <p>旧版迁移 · 原子复制</p>
        <h2>把旧题库安全搬进来</h2>
        <span>先只读预检，再由你确认导入；任何失败都会撤销新侧写入，原目录永远不动。</span>
      </div>
      <button
        ref="scanButton"
        type="button"
        :disabled="scanning || importing || rollingBack"
        @click="scanLegacy"
      >
        <FolderSearch :size="17" />{{ scanning ? '正在预检…' : candidate ? '重新选择并预检' : '选择旧版目录并扫描' }}
      </button>
    </header>

    <p
      v-if="errorMessage"
      class="legacy-error"
      role="alert"
    >
      <TriangleAlert :size="16" />{{ errorMessage }}
    </p>

    <LegacyImportResult
      v-if="receipt"
      :receipt="receipt"
      @restart="restart"
    />
    <LegacyImportResult
      v-if="rollbackReceipt"
      :rollback="rollbackReceipt"
    />

    <template v-if="candidate && !receipt">
      <div class="candidate-banner">
        <ShieldCheck :size="20" />
        <div><strong>预检完成，尚未写入新题库</strong><span>候选将在 30 分钟后失效；页面只保存匿名候选 ID，不保存目录路径。</span></div>
        <small><Clock3 :size="13" />{{ formatTime(candidate.expiresAtUtcMs) }} 前有效</small>
      </div>
      <dl class="migration-stats">
        <div><dt>学习档案</dt><dd>{{ candidate.report.members }}</dd></div>
        <div><dt>可导入错题</dt><dd>{{ candidate.problemCount }}</dd></div>
        <div><dt>可读图片</dt><dd>{{ candidate.report.existingAssets }}</dd></div>
        <div><dt>训练记录</dt><dd>{{ candidate.report.trainingRecords }}</dd></div>
        <div><dt>冻结记录</dt><dd>{{ candidate.report.frozenRecords }}</dd></div>
        <div><dt>待核对项</dt><dd>{{ candidate.report.issues.length }}</dd></div>
      </dl>

      <p
        v-if="candidate.report.truncated"
        class="candidate-warning"
      >
        <TriangleAlert :size="16" />数据超过安全上限，扫描报告已截断，不能导入。
      </p>
      <p
        v-else-if="candidate.problemCount === 0"
        class="candidate-warning"
      >
        <TriangleAlert :size="16" />没有找到可导入的题目，请检查旧版目录结构。
      </p>
      <ol
        v-if="candidate.report.issues.length"
        class="issue-list"
      >
        <li
          v-for="(issue, index) in candidate.report.issues.slice(0, 50)"
          :key="`${issue.member}-${issue.recordId}-${index}`"
        >
          <strong>{{ issueLabel(issue.code) }}</strong>
          <span>{{ issue.member }}<template v-if="issue.recordId"> · {{ issue.recordId }}</template></span>
          <small>{{ issue.detail }}</small>
        </li>
      </ol>
      <p
        v-if="candidate.report.issues.length > 50"
        class="issue-overflow"
      >
        当前仅展示前 50 项，共发现 {{ candidate.report.issues.length }} 项。
      </p>

      <div
        v-if="importing"
        class="progress-card"
        aria-live="polite"
      >
        <div><strong>{{ progressLabel }}</strong><span>{{ Math.round(progressPercent) }}%</span></div>
        <div
          class="progress-track"
          role="progressbar"
          aria-label="旧版资料导入进度"
          :aria-valuenow="Math.round(progressPercent)"
          aria-valuemin="0"
          aria-valuemax="100"
        >
          <span
            class="progress-fill"
            :style="{ '--progress': progressPercent / 100 }"
          />
        </div>
        <small>请保持应用开启；此时不会修改旧版目录。</small>
      </div>
      <div
        v-else
        class="candidate-actions"
      >
        <span>{{ canImport ? '准备好后再确认一次，即可开始加密复制。' : '请先处理上面的阻止项。' }}</span>
        <button
          ref="confirmButton"
          type="button"
          :disabled="!canImport"
          @click="openImportDialog"
        >
          查看范围并确认导入
        </button>
      </div>
    </template>

    <section
      class="import-history"
      aria-labelledby="legacy-history-title"
    >
      <header>
        <div>
          <p>迁移记录</p><h3 id="legacy-history-title">
            过去的导入批次
          </h3>
        </div><History :size="19" />
      </header>
      <p
        v-if="historyLoading && !imports.length"
        class="history-empty"
      >
        正在读取迁移记录…
      </p>
      <p
        v-else-if="!imports.length"
        class="history-empty"
      >
        还没有完成过旧版迁移。
      </p>
      <ol v-else>
        <li
          v-for="item in imports"
          :key="item.importId"
        >
          <div><strong>{{ item.problemCount }} 道题 · {{ item.memberCount }} 个档案</strong><span>{{ item.assetCount }} 张图片 · {{ item.reviewCount }} 条训练记录</span><small>{{ formatTime(item.createdAtUtcMs) }}</small></div>
          <span
            class="history-status"
            :class="item.status"
          >{{ item.status === 'completed' ? '已导入' : '已撤销' }}</span>
          <button
            v-if="item.status === 'completed'"
            type="button"
            :disabled="rollingBack || importing"
            @click="requestRollback(item, $event.currentTarget as HTMLButtonElement)"
          >
            <ArchiveRestore :size="15" />撤销这次导入
          </button>
        </li>
      </ol>
    </section>

    <LegacyImportDialog
      v-if="dialogMode === 'import' && candidate"
      mode="import"
      :busy="importing"
      :member-count="candidate.report.members"
      :problem-count="candidate.problemCount"
      @cancel="closeDialog"
      @confirm="beginImport"
    />
    <LegacyImportDialog
      v-if="dialogMode === 'rollback' && rollbackTarget"
      mode="rollback"
      :busy="rollingBack"
      :member-count="rollbackTarget.memberCount"
      :problem-count="rollbackTarget.problemCount"
      @cancel="closeDialog"
      @confirm="beginRollback"
    />
  </section>
</template>

<style scoped>
.legacy-panel{margin-top:16px;padding:26px;border:1px solid var(--line);border-radius:17px;background:rgba(255,253,247,.78);box-shadow:0 16px 48px rgba(34,48,43,.05)}.legacy-header{display:flex;gap:24px;align-items:flex-start;justify-content:space-between;margin-bottom:20px}.legacy-header p,.import-history header p{margin:0 0 8px;color:var(--cinnabar);font-size:12px;font-weight:850;letter-spacing:.14em}.legacy-header h2,.import-history h3{margin:0;color:var(--green-deep);font-family:Georgia,'Microsoft YaHei',serif;font-size:21px}.legacy-header span{display:block;max-width:680px;margin-top:9px;color:var(--ink-muted)}button{display:inline-flex;min-height:44px;gap:7px;align-items:center;justify-content:center;padding:10px 14px;border:1px solid var(--line);border-radius:10px;background:var(--paper-raised);cursor:pointer}button:disabled{opacity:.5;cursor:default}.legacy-error,.candidate-warning{display:flex;gap:8px;align-items:center;padding:12px 14px;color:#843d2c;border-radius:10px;background:rgba(185,88,63,.08)}.candidate-banner{display:grid;grid-template-columns:auto 1fr auto;gap:12px;align-items:center;padding:15px;color:#557263;border:1px solid rgba(33,51,45,.14);border-radius:12px;background:rgba(33,51,45,.055)}.candidate-banner>svg{align-self:start}.candidate-banner div{display:grid;gap:4px}.candidate-banner strong{color:var(--green-deep)}.candidate-banner span,.candidate-banner small{color:var(--ink-muted);font-size:11px;line-height:1.6}.candidate-banner small{display:flex;gap:5px;align-items:center}.migration-stats{display:grid;grid-template-columns:repeat(6,minmax(0,1fr));gap:10px;margin:14px 0 0}.migration-stats div{padding:15px;border-radius:12px;background:rgba(232,221,199,.34)}.migration-stats dt{color:var(--ink-muted);font-size:11px}.migration-stats dd{margin:6px 0 0;color:var(--green-deep);font-family:Georgia,serif;font-size:25px;font-weight:700}.issue-list{display:grid;gap:8px;max-height:330px;margin:14px 0 0;padding:0;overflow:auto;list-style:none}.issue-list li{display:grid;grid-template-columns:108px minmax(120px,.6fr) 1fr;gap:12px;align-items:start;padding:12px 14px;border-radius:10px;background:rgba(185,88,63,.06)}.issue-list strong{color:#843d2c;font-size:12px}.issue-list span,.issue-list small,.issue-overflow{color:var(--ink-muted);font-size:11px;overflow-wrap:anywhere}.candidate-actions{display:flex;gap:18px;align-items:center;justify-content:flex-end;margin-top:16px}.candidate-actions>span{margin-right:auto;color:var(--ink-muted);font-size:12px}.candidate-actions button{color:#fffdf7;border-color:var(--green-deep);background:var(--green-deep)}.progress-card{display:grid;gap:10px;margin-top:16px;padding:16px;border-radius:12px;background:rgba(33,51,45,.06)}.progress-card>div:first-child{display:flex;justify-content:space-between;color:var(--green-deep);font-size:12px}.progress-track{height:8px;overflow:hidden;border-radius:999px;background:rgba(33,51,45,.12)}.progress-fill{display:block;width:100%;height:100%;border-radius:inherit;background:linear-gradient(90deg,var(--green-deep),#6f8e7f);transform:scaleX(var(--progress));transform-origin:left;transition:transform var(--motion-standard) var(--ease-standard)}.progress-card small{color:var(--ink-muted)}.import-history{margin-top:22px;padding-top:20px;border-top:1px solid var(--line)}.import-history>header{display:flex;align-items:center;justify-content:space-between;margin:0}.import-history header p{font-size:10px}.import-history h3{font-size:17px}.history-empty{margin:13px 0 0;color:var(--ink-muted);font-size:12px}.import-history ol{display:grid;gap:8px;margin:13px 0 0;padding:0;list-style:none}.import-history li{display:grid;grid-template-columns:1fr auto auto;gap:12px;align-items:center;padding:13px 14px;border-radius:11px;background:rgba(232,221,199,.27)}.import-history li>div{display:grid;gap:3px}.import-history li strong{color:var(--green-deep);font-size:12px}.import-history li span,.import-history li small{color:var(--ink-muted);font-size:10px}.history-status{padding:5px 8px;border-radius:999px;background:rgba(33,51,45,.08)}.history-status.rolled_back{color:#843d2c;background:rgba(185,88,63,.08)}.import-history button{color:#843d2c;border-color:rgba(185,88,63,.22);background:transparent}@media(max-width:980px){.migration-stats{grid-template-columns:repeat(3,minmax(0,1fr))}}@media(max-width:760px){.legacy-panel{padding:20px 16px}.legacy-header{flex-direction:column}.legacy-header button{width:100%}.candidate-banner{grid-template-columns:auto 1fr}.candidate-banner small{grid-column:2}.migration-stats{grid-template-columns:repeat(2,minmax(0,1fr))}.issue-list li{grid-template-columns:1fr;gap:4px}.candidate-actions{align-items:stretch;flex-direction:column}.candidate-actions button{width:100%}.import-history li{grid-template-columns:1fr auto}.import-history button{grid-column:1/-1;width:100%}}@media(prefers-reduced-motion:reduce){.progress-fill{transition:none}}
</style>
