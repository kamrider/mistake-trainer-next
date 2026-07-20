<script setup lang="ts">
import { isTauri } from '@tauri-apps/api/core'
import { ChartNoAxesColumnIncreasing, Clock3, FileArchive, Flame, RotateCcw } from '@lucide/vue'
import { computed, onMounted, ref } from 'vue'
import ExportCandidatePicker from '../../modules/export/components/ExportCandidatePicker.vue'
import ExportSnapshotHistory from '../../modules/export/components/ExportSnapshotHistory.vue'
import { commands, type DeletedExportSnapshotSummary, type ExportCandidate, type ExportCandidateSource, type ExportLayout, type ExportSnapshotSummary, type ReportSummary } from '../../shared/api/bindings'
import { normalizeAppResult } from '../../shared/api/normalize-result'

const report = ref<ReportSummary>()
const snapshots = ref<ExportSnapshotSummary[]>([])
const deletedSnapshots = ref<DeletedExportSnapshotSummary[]>([])
const candidates = ref<ExportCandidate[]>([])
const candidateSource = ref<ExportCandidateSource>('due')
const selectedIds = ref<string[]>([])
const snapshotsLoaded = ref(false)
const trashLoaded = ref(false)
const loading = ref(true)
const candidateLoading = ref(false)
const saving = ref(false)
const deletingId = ref('')
const generatingId = ref('')
const restoringId = ref('')
const generatedMessage = ref('')
const errorMessage = ref('')
const candidateError = ref('')
const title = ref(`错题复盘 ${new Date().toLocaleDateString('zh-CN')}`)
const layout = ref<ExportLayout>('question_answer_alternating')
const maxReviewCount = computed(() => Math.max(1, ...(report.value?.dailyActivity.map(day => day.reviewCount) ?? [1])))
const rememberedPercent = computed(() => Math.round((report.value?.rememberedRate ?? 0) * 100))
const durationMinutes = computed(() => Math.round((report.value?.totalDurationMs ?? 0) / 60_000))
const layoutOptions: Array<{ value: ExportLayout; label: string; hint: string }> = [
  { value: 'question_answer_alternating', label: '题答交替', hint: '每道题后紧跟答案，适合逐题复盘' },
  { value: 'questions_then_answers', label: '题目后集中答案', hint: '先完成整份题目，再统一核对答案' },
  { value: 'original_image_folder', label: '原图文件夹', hint: '按题号与题答角色整理原始图片' },
]
const orderedSelectedProblemIds = computed(() => {
  const selected = new Set(selectedIds.value)
  return candidates.value.filter(candidate => selected.has(candidate.id)).map(candidate => candidate.id)
})
let candidateRequest = 0

function isDevelopmentPreview() {
  return import.meta.env.DEV && window.location.hash.includes('preview=report')
}

function developmentCandidates(source: ExportCandidateSource): ExportCandidate[] {
  const items: ExportCandidate[] = [
    { id: 'preview-math-1', subject: '数学', note: '圆锥曲线：焦点弦长度', questionAssetCount: 2, answerAssetCount: 2, dueAtUtcMs: Date.now() - 3_600_000, reviewCount: 4 },
    { id: 'preview-physics-1', subject: '物理', note: '带电粒子在复合场中的运动', questionAssetCount: 1, answerAssetCount: 1, dueAtUtcMs: Date.now() - 86_400_000, reviewCount: 2 },
    { id: 'preview-chemistry-1', subject: '化学', note: '有机反应路线推断', questionAssetCount: 3, answerAssetCount: 0, dueAtUtcMs: null, reviewCount: 0 },
    { id: 'preview-english-1', subject: '英语', note: '完形填空中的语境线索', questionAssetCount: 2, answerAssetCount: 1, dueAtUtcMs: Date.now() + 86_400_000, reviewCount: 7 },
  ]
  if (source === 'due') return items.slice(0, 3)
  if (source === 'latest_review_session') return [items[1]!, items[0]!, items[3]!]
  return items
}

function loadDevelopmentPreview() {
  const day = 86_400_000
  const today = Math.floor(Date.now() / day) * day
  report.value = {
    activeProblemCount: 46,
    dueProblemCount: 8,
    reviewCount: 128,
    rememberedRate: 0.84,
    totalDurationMs: 4_920_000,
    currentStreakDays: 12,
    dailyActivity: Array.from({ length: 14 }, (_, index) => ({
      dayStartUtcMs: today - (13 - index) * day,
      reviewCount: [3, 7, 4, 0, 8, 12, 6, 9, 5, 14, 11, 7, 16, 10][index] ?? 0,
      durationMs: null,
    })),
    subjectActivity: [
      { subject: '数学', problemCount: 18, reviewCount: 52 },
      { subject: '物理', problemCount: 11, reviewCount: 31 },
      { subject: '英语', problemCount: 9, reviewCount: 26 },
      { subject: '化学', problemCount: 8, reviewCount: 19 },
    ],
  }
  candidates.value = developmentCandidates(candidateSource.value)
  selectedIds.value = candidates.value.map(candidate => candidate.id)
  snapshots.value = [
    { id: 'preview-snapshot-1', title: '本周重点错题复盘', problemCount: 12, layout: 'question_answer_alternating', createdAtUtcMs: Date.now() - day },
    { id: 'preview-snapshot-2', title: '月考前数学题册', problemCount: 24, layout: 'questions_then_answers', createdAtUtcMs: Date.now() - 4 * day },
  ]
  deletedSnapshots.value = [{
    snapshot: { id: 'preview-deleted-1', title: '旧版物理整理', problemCount: 7, layout: 'original_image_folder', createdAtUtcMs: Date.now() - 12 * day },
    deletedAtUtcMs: Date.now() - 2 * day,
    purgeAfterUtcMs: Date.now() + 28 * day,
  }]
  snapshotsLoaded.value = true
  trashLoaded.value = true
}

function dayLabel(value: number | null) {
  return value == null ? '—' : new Date(value).toLocaleDateString('zh-CN', { month: 'numeric', day: 'numeric', timeZone: 'UTC' })
}

async function load() {
  loading.value = true
  errorMessage.value = ''
  snapshotsLoaded.value = false
  trashLoaded.value = false
  try {
    if (!isTauri()) {
      if (isDevelopmentPreview()) loadDevelopmentPreview()
      return
    }
    const candidateTask = loadCandidates(candidateSource.value)
    const [reportResult, snapshotsResult, trashResult] = await Promise.all([
      commands.reportSummary(),
      commands.exportList(),
      commands.exportTrashList(),
    ])
    const normalizedReport = normalizeAppResult(reportResult)
    const normalizedSnapshots = normalizeAppResult(snapshotsResult)
    const normalizedTrash = normalizeAppResult(trashResult)
    const errors: string[] = []
    if (normalizedReport.ok) report.value = normalizedReport.data
    else errors.push(normalizedReport.error.userMessage)
    if (normalizedSnapshots.ok) {
      snapshots.value = normalizedSnapshots.data
      snapshotsLoaded.value = true
    }
    else errors.push('导出快照没有读取成功。')
    if (normalizedTrash.ok) {
      deletedSnapshots.value = normalizedTrash.data
      trashLoaded.value = true
    }
    else errors.push('导出快照回收区没有读取成功。')
    errorMessage.value = errors.join(' ')
    await candidateTask
  }
  catch {
    errorMessage.value = '学习报告暂时无法读取，请重新打开应用后再试。'
  }
  finally {
    loading.value = false
  }
}

async function loadCandidates(source: ExportCandidateSource) {
  const request = ++candidateRequest
  candidateLoading.value = true
  candidateError.value = ''
  candidates.value = []
  selectedIds.value = []
  try {
    const result = normalizeAppResult(await commands.exportCandidates(source))
    if (request !== candidateRequest) return
    if (!result.ok) {
      candidateError.value = result.error.userMessage
      return
    }
    candidates.value = result.data
    selectedIds.value = result.data.map(candidate => candidate.id)
  }
  catch {
    if (request === candidateRequest)
      candidateError.value = '可导出的题目没有读取成功，请稍后重试。'
  }
  finally {
    if (request === candidateRequest)
      candidateLoading.value = false
  }
}

function changeCandidateSource(source: ExportCandidateSource) {
  if (source === candidateSource.value || candidateLoading.value) return
  candidateSource.value = source
  if (isDevelopmentPreview()) {
    candidates.value = developmentCandidates(source)
    selectedIds.value = candidates.value.map(candidate => candidate.id)
    return
  }
  void loadCandidates(source)
}

function toggleCandidate(problemId: string) {
  const next = new Set(selectedIds.value)
  if (next.has(problemId)) next.delete(problemId)
  else next.add(problemId)
  selectedIds.value = candidates.value.filter(candidate => next.has(candidate.id)).map(candidate => candidate.id)
}

function selectCandidates(problemIds: string[]) {
  const next = new Set([...selectedIds.value, ...problemIds])
  selectedIds.value = candidates.value.filter(candidate => next.has(candidate.id)).map(candidate => candidate.id)
}

async function createSnapshot() {
  if (orderedSelectedProblemIds.value.length === 0 || !title.value.trim()) return
  saving.value = true
  errorMessage.value = ''
  try {
    const result = normalizeAppResult(await commands.exportCreate({
      title: title.value,
      problemIds: orderedSelectedProblemIds.value,
      layout: layout.value,
    }))
    if (!result.ok) {
      errorMessage.value = result.error.userMessage
      return
    }
    snapshots.value = [result.data, ...snapshots.value]
    generatedMessage.value = `已保存“${result.data.title}”，随时可以从下方重新生成。`
  }
  catch {
    errorMessage.value = '导出快照没有保存，请稍后重试。'
  }
  finally {
    saving.value = false
  }
}

async function deleteSnapshot(snapshotId: string) {
  if (!window.confirm('删除这个导出快照吗？原题不会受到影响。')) return
  deletingId.value = snapshotId
  errorMessage.value = ''
  try {
    const result = normalizeAppResult(await commands.exportDelete(snapshotId))
    if (result.ok) {
      snapshots.value = snapshots.value.filter(snapshot => snapshot.id !== snapshotId)
      const trashResult = normalizeAppResult(await commands.exportTrashList())
      if (trashResult.ok) {
        deletedSnapshots.value = trashResult.data
        trashLoaded.value = true
      }
      else errorMessage.value = '快照已删除，但回收区暂时没有刷新成功。'
    }
    else errorMessage.value = result.error.userMessage
  }
  catch {
    errorMessage.value = '导出快照没有删除，请稍后重试。'
  }
  finally {
    deletingId.value = ''
  }
}

async function generateSnapshot(snapshot: ExportSnapshotSummary) {
  generatingId.value = snapshot.id
  generatedMessage.value = ''
  errorMessage.value = ''
  try {
    const result = normalizeAppResult(await commands.exportGenerate(snapshot.id))
    if (!result.ok) {
      errorMessage.value = result.error.userMessage
      return
    }
    if (result.data) {
      generatedMessage.value = `已生成 ${result.data.outputName}，共 ${result.data.problemCount} 题。`
    }
  }
  catch {
    errorMessage.value = '文件没有生成，请检查目标目录空间与权限后重试。'
  }
  finally {
    generatingId.value = ''
  }
}

async function restoreSnapshot(deleted: DeletedExportSnapshotSummary) {
  restoringId.value = deleted.snapshot.id
  errorMessage.value = ''
  try {
    const result = normalizeAppResult(await commands.exportRestore(deleted.snapshot.id))
    if (result.ok) {
      snapshots.value = [deleted.snapshot, ...snapshots.value]
      deletedSnapshots.value = deletedSnapshots.value.filter(item => item.snapshot.id !== deleted.snapshot.id)
    }
    else errorMessage.value = result.error.userMessage
  }
  catch {
    errorMessage.value = '导出快照没有恢复，请稍后重试。'
  }
  finally {
    restoringId.value = ''
  }
}

onMounted(load)
</script>

<template>
  <main class="report-page">
    <header class="page-heading">
      <div>
        <p>学习报告 · 本地实时计算</p>
        <h1>把练习变成看得见的节奏</h1>
        <span>只呈现事实，不用夸张的红色数字制造焦虑。</span>
      </div>
      <div class="heading-actions">
        <a
          class="history-link"
          href="#/report/history"
        ><Clock3
          :size="16"
          aria-hidden="true"
        />查看完整复习历史</a>
        <button
          type="button"
          :disabled="loading"
          @click="load"
        >
          <RotateCcw
            :size="16"
            aria-hidden="true"
          />刷新
        </button>
      </div>
    </header>

    <p
      v-if="errorMessage"
      class="error-banner"
      role="alert"
    >
      {{ errorMessage }}
    </p>
    <section
      class="metric-grid"
      :aria-busy="loading"
    >
      <article><span>当前记住率</span><strong>{{ rememberedPercent }}<small>%</small></strong><p>{{ report?.reviewCount ?? 0 }} 次真实评分</p></article>
      <article><span>今天可训练</span><strong>{{ report?.dueProblemCount ?? 0 }}<small>题</small></strong><p>新题与到期题合计</p></article>
      <article>
        <span>连续训练</span><strong>{{ report?.currentStreakDays ?? 0 }}<small>天</small></strong><p>
          <Flame
            :size="14"
            aria-hidden="true"
          />按 UTC 训练日计算
        </p>
      </article>
      <article>
        <span>累计专注</span><strong>{{ durationMinutes }}<small>分钟</small></strong><p>
          <Clock3
            :size="14"
            aria-hidden="true"
          />来自每次提交用时
        </p>
      </article>
    </section>

    <section class="report-grid">
      <article class="paper-panel activity-panel">
        <header>
          <div><p>近 14 天</p><h2>复习节奏</h2></div><ChartNoAxesColumnIncreasing
            :size="22"
            aria-hidden="true"
          />
        </header>
        <div
          class="bar-chart"
          aria-label="近十四天复习次数柱状图"
        >
          <div
            v-for="day in report?.dailyActivity ?? []"
            :key="day.dayStartUtcMs ?? 0"
            class="bar-column"
          >
            <span class="bar-value">{{ day.reviewCount }}</span>
            <i :style="{ height: `${Math.max(4, day.reviewCount / maxReviewCount * 100)}%` }" />
            <small>{{ dayLabel(day.dayStartUtcMs) }}</small>
          </div>
        </div>
      </article>

      <article class="paper-panel subject-panel">
        <header><div><p>题库结构</p><h2>科目分布</h2></div></header>
        <ol
          v-if="report?.subjectActivity.length"
          class="subject-list"
        >
          <li
            v-for="subject in report.subjectActivity"
            :key="subject.subject"
          >
            <span>{{ subject.subject }}</span><i><b :style="{ width: `${Math.max(8, subject.problemCount / Math.max(1, report.activeProblemCount) * 100)}%` }" /></i>
            <small>{{ subject.problemCount }} 题 · {{ subject.reviewCount }} 次复习</small>
          </li>
        </ol>
        <p
          v-else
          class="empty-copy"
        >
          采集第一道错题后，这里会形成真实的科目结构。
        </p>
      </article>
    </section>

    <section class="paper-panel export-panel">
      <header>
        <div><p>导出中心</p><h2>保存配置并生成学习材料</h2><span>先保存可同步快照，再由 Rust 解密并原子生成 DOCX 或原图文件夹。</span></div><FileArchive
          :size="24"
          aria-hidden="true"
        />
      </header>
      <div class="export-form">
        <div class="candidate-step">
          <div class="step-heading">
            <span>01</span><div><strong>挑选题目</strong><small>来源只决定初始选择，你仍可逐题增删。</small></div>
          </div>
          <p
            v-if="candidateError"
            class="candidate-error"
            role="alert"
          >
            <span>{{ candidateError }}</span>
            <button
              type="button"
              :disabled="candidateLoading"
              @click="loadCandidates(candidateSource)"
            >
              重新读取候选题
            </button>
          </p>
          <ExportCandidatePicker
            :candidates="candidates"
            :source="candidateSource"
            :selected-ids="selectedIds"
            :loading="candidateLoading"
            @source="changeCandidateSource"
            @toggle="toggleCandidate"
            @select-all="selectCandidates"
            @clear="selectedIds = []"
          />
        </div>
        <div class="configuration-step">
          <div class="step-heading">
            <span>02</span><div><strong>设置材料</strong><small>快照保存后可随时换文件夹重新生成。</small></div>
          </div>
          <label>快照名称<input
            v-model="title"
            maxlength="80"
          ></label>
          <fieldset>
            <legend>排版方式</legend>
            <label
              v-for="option in layoutOptions"
              :key="option.value"
              class="layout-option"
              :class="{ active: layout === option.value }"
            >
              <input
                v-model="layout"
                type="radio"
                :value="option.value"
              ><span><strong>{{ option.label }}</strong><small>{{ option.hint }}</small></span>
            </label>
          </fieldset>
          <button
            type="button"
            :disabled="saving || candidateLoading || orderedSelectedProblemIds.length === 0 || !title.trim()"
            @click="createSnapshot"
          >
            {{ saving ? '正在保存…' : `保存 ${orderedSelectedProblemIds.length} 道题的导出快照` }}
          </button>
        </div>
      </div>
      <p
        v-if="generatedMessage"
        class="success-banner"
        role="status"
      >
        {{ generatedMessage }}
      </p>
      <ExportSnapshotHistory
        :snapshots="snapshots"
        :deleted-snapshots="deletedSnapshots"
        :snapshots-loaded="snapshotsLoaded"
        :trash-loaded="trashLoaded"
        :generating-id="generatingId"
        :deleting-id="deletingId"
        :restoring-id="restoringId"
        @generate="generateSnapshot"
        @delete="deleteSnapshot"
        @restore="restoreSnapshot"
      />
    </section>
  </main>
</template>

<style scoped>
.report-page { min-height: 100vh; padding: 42px clamp(24px,5vw,72px) 72px; background: radial-gradient(circle at 90% 2%,rgba(185,88,63,.07),transparent 28%); }
.page-heading, .paper-panel > header { display: flex; justify-content: space-between; gap: 24px; align-items: flex-start; }
.page-heading { margin-bottom: 28px; }
.page-heading p, .paper-panel header p { margin: 0 0 8px; color: var(--cinnabar); font-size: 12px; font-weight: 800; letter-spacing: .14em; text-transform: uppercase; }
h1, h2 { margin: 0; font-family: Georgia,'Microsoft YaHei',serif; color: var(--green-deep); }
h1 { font-size: clamp(28px,4vw,44px); } h2 { font-size: 22px; }
.page-heading span, .paper-panel header span { display: block; margin-top: 9px; color: var(--ink-muted); }
button { display: inline-flex; gap: 7px; align-items: center; justify-content: center; border: 1px solid var(--line); border-radius: 10px; background: var(--paper-raised); color: var(--ink); cursor: pointer; }
.page-heading button { padding: 10px 14px; } button:disabled { cursor: not-allowed; opacity: .48; }
.heading-actions { display: flex; gap: 8px; align-items: center; }.history-link { display: inline-flex; gap: 7px; align-items: center; min-height: 44px; padding: 0 14px; color: var(--paper-raised); border-radius: 999px; background: var(--green-deep); font-size: 12px; font-weight: 720; text-decoration: none; transition: transform var(--motion-feedback) var(--ease-standard), box-shadow var(--motion-standard) var(--ease-standard); }.history-link:hover { transform: translateY(-1px); box-shadow: 0 8px 20px rgba(33,51,45,.16); }
.error-banner { padding: 12px 14px; border: 1px solid rgba(185,88,63,.28); border-radius: 10px; background: rgba(185,88,63,.08); color: #843d2c; }
.metric-grid { display: grid; grid-template-columns: repeat(4,minmax(0,1fr)); gap: 14px; }
.metric-grid article, .paper-panel { border: 1px solid var(--line); border-radius: 17px; background: rgba(255,253,247,.78); box-shadow: 0 16px 48px rgba(34,48,43,.05); }
.metric-grid article { padding: 20px; } .metric-grid span { color: var(--ink-muted); font-size: 12px; }
.metric-grid strong { display: block; margin: 10px 0 6px; font-family: Georgia,serif; color: var(--green-deep); font-size: 36px; }
.metric-grid strong small { margin-left: 4px; font: 600 13px 'Microsoft YaHei',sans-serif; } .metric-grid p { display: flex; gap: 5px; align-items: center; margin: 0; color: var(--ink-muted); font-size: 11px; }
.report-grid { display: grid; grid-template-columns: minmax(0,1.6fr) minmax(280px,.8fr); gap: 16px; margin-top: 16px; }
.paper-panel { padding: 24px; } .bar-chart { display: grid; grid-template-columns: repeat(14,1fr); gap: 7px; align-items: end; height: 230px; margin-top: 24px; padding-top: 26px; border-bottom: 1px solid var(--line); }
.bar-column { display: grid; grid-template-rows: 18px 1fr 24px; height: 100%; align-items: end; text-align: center; } .bar-column i { display: block; width: 100%; max-width: 24px; min-height: 4px; margin: 0 auto; border-radius: 5px 5px 2px 2px; background: var(--green-deep); transition: height var(--motion-page) var(--ease-standard); }
.bar-column small, .bar-value { color: var(--ink-muted); font-size: 9px; } .subject-list { display: grid; gap: 19px; margin: 24px 0 0; padding: 0; list-style: none; }
.subject-list li { display: grid; grid-template-columns: 1fr auto; gap: 7px; } .subject-list i { grid-column: 1/-1; height: 7px; overflow: hidden; border-radius: 10px; background: var(--sand); }
.subject-list b { display: block; height: 100%; border-radius: inherit; background: var(--cinnabar); } .subject-list small { color: var(--ink-muted); font-size: 10px; }
.export-panel { margin-top: 16px; }
.export-form { display: grid; gap: 18px; margin-top: 22px; }
.candidate-step,
.configuration-step { padding: 18px; border-radius: 5px 15px 15px; background: rgba(232, 221, 199, .25); }
.step-heading { display: flex; gap: 11px; align-items: center; margin-bottom: 15px; }
.step-heading > span { display: grid; width: 34px; height: 34px; place-items: center; color: var(--paper); border-radius: 11px 3px 11px 11px; background: var(--cinnabar); font-family: var(--font-serif); font-size: 12px; }
.step-heading strong,
.step-heading small { display: block; }
.step-heading strong { color: var(--green-deep); font-size: 13px; }
.step-heading small { margin-top: 3px; color: var(--ink-muted); font-size: 10px; }
.candidate-error { display: flex; gap: 12px; align-items: center; justify-content: space-between; margin: 0 0 12px; padding: 10px 12px; color: #843d2c; border: 1px solid rgba(185, 88, 63, .24); border-radius: 10px; background: rgba(185, 88, 63, .08); font-size: 11px; }
.candidate-error button { min-height: 32px; padding: 0 10px; color: #843d2c; background: transparent; white-space: nowrap; }
.configuration-step { display: grid; grid-template-columns: minmax(220px, .7fr) minmax(340px, 1.3fr) auto; gap: 18px; align-items: end; }
.configuration-step .step-heading { grid-column: 1 / -1; margin-bottom: 0; }
.export-form label,
.export-form legend { color: var(--ink-muted); font-size: 11px; font-weight: 700; }
.export-form input[type='text'],
.export-form label > input:not([type]) { display: block; width: 100%; margin-top: 7px; padding: 10px 11px; border: 1px solid var(--line); border-radius: 9px; background: var(--paper-raised); }
fieldset { display: grid; grid-template-columns: repeat(3, minmax(0, 1fr)); gap: 8px; margin: 0; padding: 0; border: 0; }
legend { width: 100%; margin-bottom: 7px; }
fieldset label { display: flex; gap: 7px; align-items: flex-start; }
.layout-option { padding: 10px; border: 1px solid var(--line); border-radius: 4px 11px 11px; background: rgba(255, 253, 247, .65); cursor: pointer; transition: transform var(--motion-feedback) var(--ease-standard), border-color var(--motion-standard) var(--ease-standard), background var(--motion-standard) var(--ease-standard); }
.layout-option:hover { transform: translateY(-2px); }
.layout-option.active { border-color: rgba(185, 88, 63, .42); background: #fbf2e7; }
.layout-option input { margin-top: 2px; accent-color: var(--cinnabar); }
.layout-option span,
.layout-option strong,
.layout-option small { display: block; }
.layout-option strong { color: var(--ink); font-size: 11px; }
.layout-option small { margin-top: 4px; color: var(--ink-muted); font-size: 9px; font-weight: 500; line-height: 1.4; }
.configuration-step > button { min-height: 40px; padding: 0 15px; color: white; border-color: var(--green-deep); background: var(--green-deep); white-space: nowrap; transition: transform var(--motion-feedback) var(--ease-standard), box-shadow var(--motion-standard) var(--ease-standard); }
.configuration-step > button:hover:not(:disabled) { box-shadow: 0 10px 24px rgba(33, 51, 45, .15); transform: translateY(-2px); }
.success-banner { margin: 16px 0 0; padding: 11px 13px; border: 1px solid rgba(33,51,45,.16); border-radius: 10px; background: var(--green-soft); color: var(--green-deep); font-size: 12px; }
.empty-copy { color: var(--ink-muted); font-size: 13px; }
@media (max-width: 1050px) { .metric-grid { grid-template-columns: repeat(2,1fr); } .report-grid { grid-template-columns: 1fr; } .configuration-step { grid-template-columns: 1fr; } .configuration-step .step-heading { grid-column: auto; } }
@media (max-width: 760px) { fieldset { grid-template-columns: 1fr; } .page-heading { flex-direction: column; } .heading-actions { width: 100%; flex-wrap: wrap; } }
@media (max-width: 620px) { .report-page { padding: 24px 16px 92px; } .metric-grid { grid-template-columns: 1fr; } .bar-chart { gap: 3px; overflow: hidden; } .bar-column small { writing-mode: vertical-rl; } }
@media (prefers-reduced-motion: reduce) { .configuration-step > button, .layout-option, .history-link { transition: none; } .configuration-step > button:hover:not(:disabled), .layout-option:hover, .history-link:hover { transform: none; } }
</style>
