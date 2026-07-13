<script setup lang="ts">
import { isTauri } from '@tauri-apps/api/core'
import { ChartNoAxesColumnIncreasing, Clock3, FileArchive, FileDown, Flame, RotateCcw, Trash2 } from '@lucide/vue'
import { computed, onMounted, ref } from 'vue'
import { commands, type DeletedExportSnapshotSummary, type ExportLayout, type ExportSnapshotSummary, type ProblemSummary, type ReportSummary } from '../../shared/api/bindings'
import { normalizeAppResult } from '../../shared/api/normalize-result'

const report = ref<ReportSummary>()
const snapshots = ref<ExportSnapshotSummary[]>([])
const deletedSnapshots = ref<DeletedExportSnapshotSummary[]>([])
const activeProblems = ref<ProblemSummary[]>([])
const snapshotsLoaded = ref(false)
const trashLoaded = ref(false)
const problemsLoaded = ref(false)
const loading = ref(true)
const saving = ref(false)
const deletingId = ref('')
const generatingId = ref('')
const restoringId = ref('')
const generatedMessage = ref('')
const errorMessage = ref('')
const title = ref(`错题复盘 ${new Date().toLocaleDateString('zh-CN')}`)
const layout = ref<ExportLayout>('question_answer_alternating')
const maxReviewCount = computed(() => Math.max(1, ...(report.value?.dailyActivity.map(day => day.reviewCount) ?? [1])))
const rememberedPercent = computed(() => Math.round((report.value?.rememberedRate ?? 0) * 100))
const durationMinutes = computed(() => Math.round((report.value?.totalDurationMs ?? 0) / 60_000))

function dayLabel(value: number | null) {
  return value == null ? '—' : new Date(value).toLocaleDateString('zh-CN', { month: 'numeric', day: 'numeric', timeZone: 'UTC' })
}

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

async function load() {
  loading.value = true
  errorMessage.value = ''
  snapshotsLoaded.value = false
  trashLoaded.value = false
  problemsLoaded.value = false
  try {
    if (!isTauri()) return
    const [reportResult, snapshotsResult, trashResult, problemsResult] = await Promise.all([
      commands.reportSummary(),
      commands.exportList(),
      commands.exportTrashList(),
      commands.problemList('active', null),
    ])
    const normalizedReport = normalizeAppResult(reportResult)
    const normalizedSnapshots = normalizeAppResult(snapshotsResult)
    const normalizedTrash = normalizeAppResult(trashResult)
    const normalizedProblems = normalizeAppResult(problemsResult)
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
    if (normalizedProblems.ok) {
      activeProblems.value = normalizedProblems.data
      problemsLoaded.value = true
    }
    else errors.push('活动题目没有读取成功，暂时不能保存快照。')
    errorMessage.value = errors.join(' ')
  }
  catch {
    errorMessage.value = '学习报告暂时无法读取，请重新打开应用后再试。'
  }
  finally {
    loading.value = false
  }
}

async function createSnapshot() {
  if (activeProblems.value.length === 0 || !title.value.trim()) return
  saving.value = true
  errorMessage.value = ''
  try {
    const result = normalizeAppResult(await commands.exportCreate({
      title: title.value,
      problemIds: activeProblems.value.slice(0, 500).map(problem => problem.id),
      layout: layout.value,
    }))
    if (!result.ok) {
      errorMessage.value = result.error.userMessage
      return
    }
    snapshots.value = [result.data, ...snapshots.value]
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
        <label>快照名称<input
          v-model="title"
          maxlength="80"
        ></label>
        <fieldset>
          <legend>排版方式</legend>
          <label
            v-for="option in (['question_answer_alternating', 'questions_then_answers', 'original_image_folder'] as ExportLayout[])"
            :key="option"
          >
            <input
              v-model="layout"
              type="radio"
              :value="option"
            >{{ layoutLabel(option) }}
          </label>
        </fieldset>
        <button
          type="button"
          :disabled="saving || !problemsLoaded || activeProblems.length === 0 || !title.trim()"
          @click="createSnapshot"
        >
          {{ saving ? '正在保存…' : `保存 ${Math.min(activeProblems.length, 500)} 道活动题配置` }}
        </button>
      </div>
      <p
        v-if="generatedMessage"
        class="success-banner"
        role="status"
      >
        {{ generatedMessage }}
      </p>
      <ul
        v-if="snapshots.length"
        class="snapshot-list"
      >
        <li
          v-for="snapshot in snapshots"
          :key="snapshot.id"
        >
          <span><strong>{{ snapshot.title }}</strong><small>{{ snapshot.problemCount }} 题 · {{ layoutLabel(snapshot.layout) }} · {{ snapshotDate(snapshot.createdAtUtcMs) }}</small></span>
          <div class="snapshot-actions">
            <button
              type="button"
              :aria-label="`生成导出文件：${snapshot.title}`"
              :disabled="Boolean(generatingId)"
              @click="generateSnapshot(snapshot)"
            >
              <FileDown :size="16" />{{ generatingId === snapshot.id ? '生成中…' : '生成文件' }}
            </button>
            <button
              type="button"
              :aria-label="`删除导出快照：${snapshot.title}`"
              :disabled="deletingId === snapshot.id"
              @click="deleteSnapshot(snapshot.id)"
            >
              <Trash2 :size="16" />
            </button>
          </div>
        </li>
      </ul>
      <p
        v-else-if="snapshotsLoaded"
        class="empty-copy"
      >
        尚无导出快照。保存一次选题与排版配置后，即可生成 DOCX 或原图文件夹。
      </p>
      <p
        v-else
        class="empty-copy"
      >
        正在读取导出快照…
      </p>
      <section class="trash-panel">
        <header><div><p>30 天回收区</p><h3>已删除快照</h3></div><span>刷新、切页或重启后仍可在到期前恢复。</span></header>
        <ul
          v-if="deletedSnapshots.length"
          class="snapshot-list trash-list"
        >
          <li
            v-for="deleted in deletedSnapshots"
            :key="deleted.snapshot.id"
          >
            <span><strong>{{ deleted.snapshot.title }}</strong><small>删除于 {{ snapshotDate(deleted.deletedAtUtcMs) }} · 保留至 {{ snapshotDate(deleted.purgeAfterUtcMs) }}</small></span>
            <button
              type="button"
              :aria-label="`恢复导出快照：${deleted.snapshot.title}`"
              :disabled="restoringId === deleted.snapshot.id"
              @click="restoreSnapshot(deleted)"
            >
              <RotateCcw :size="16" />恢复
            </button>
          </li>
        </ul>
        <p
          v-else-if="trashLoaded"
          class="empty-copy"
        >
          回收区为空。
        </p>
        <p
          v-else
          class="empty-copy"
        >
          正在读取回收区…
        </p>
      </section>
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
.export-panel { margin-top: 16px; } .export-form { display: grid; grid-template-columns: minmax(220px,.7fr) minmax(340px,1.3fr) auto; gap: 18px; align-items: end; margin-top: 22px; padding: 18px; border-radius: 13px; background: rgba(232,221,199,.32); }
.export-form label, .export-form legend { color: var(--ink-muted); font-size: 11px; font-weight: 700; } .export-form input[type='text'], .export-form label > input:not([type]) { display: block; width: 100%; margin-top: 7px; padding: 10px 11px; border: 1px solid var(--line); border-radius: 9px; background: var(--paper-raised); }
fieldset { display: flex; flex-wrap: wrap; gap: 10px 16px; margin: 0; padding: 0; border: 0; } legend { width: 100%; margin-bottom: 7px; } fieldset label { display: flex; gap: 5px; align-items: center; }
.export-form > button { min-height: 40px; padding: 0 15px; color: white; border-color: var(--green-deep); background: var(--green-deep); }
.snapshot-list { display: grid; gap: 8px; margin: 18px 0 0; padding: 0; list-style: none; } .snapshot-list li { display: flex; justify-content: space-between; gap: 16px; align-items: center; padding: 12px 14px; border-bottom: 1px solid var(--line); }
.snapshot-list strong, .snapshot-list small { display: block; } .snapshot-list small { margin-top: 4px; color: var(--ink-muted); } .snapshot-list button { min-width: 34px; height: 34px; padding: 0 9px; color: var(--cinnabar); } .snapshot-actions { display: flex; gap: 7px; align-items: center; } .snapshot-actions button:first-child { color: var(--green-deep); }
.success-banner { margin: 16px 0 0; padding: 11px 13px; border: 1px solid rgba(33,51,45,.16); border-radius: 10px; background: var(--green-soft); color: var(--green-deep); font-size: 12px; }
.trash-panel { margin-top: 22px; padding-top: 20px; border-top: 1px solid var(--line); } .trash-panel > header { display: flex; justify-content: space-between; gap: 16px; align-items: end; } .trash-panel h3 { margin: 0; color: var(--green-deep); font-family: Georgia,'Microsoft YaHei',serif; } .trash-panel header p, .trash-panel header span { margin: 0 0 5px; color: var(--ink-muted); font-size: 11px; } .trash-list button { width: auto; padding: 7px 10px; color: var(--green-deep); }
.empty-copy { color: var(--ink-muted); font-size: 13px; }
@media (max-width: 1050px) { .metric-grid { grid-template-columns: repeat(2,1fr); } .report-grid { grid-template-columns: 1fr; } .export-form { grid-template-columns: 1fr; } }
@media (max-width: 620px) { .report-page { padding: 24px 16px 92px; } .metric-grid { grid-template-columns: 1fr; } .bar-chart { gap: 3px; overflow: hidden; } .bar-column small { writing-mode: vertical-rl; } }
</style>
