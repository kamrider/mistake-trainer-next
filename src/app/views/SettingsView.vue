<script setup lang="ts">
import { isTauri } from '@tauri-apps/api/core'
import { Archive, ArchiveRestore, BookOpen, CloudOff, Database, FolderCheck, LockKeyhole, Plus, RotateCcw, ShieldCheck, Trash2, TriangleAlert, Volume2 } from '@lucide/vue'
import { nextTick, onMounted, ref } from 'vue'
import { commands, type BackupRestoreCandidate, type BackupSummary, type LegacyScanReport, type SettingsOverview, type SubjectPreferences } from '../../shared/api/bindings'
import { normalizeAppResult } from '../../shared/api/normalize-result'
import BackupRestoreDialog from '../BackupRestoreDialog.vue'

const overview = ref<SettingsOverview>()
const loading = ref(true)
const errorMessage = ref('')
const legacyReport = ref<LegacyScanReport>()
const scanningLegacy = ref(false)
const createdBackup = ref<BackupSummary>()
const restoreCandidate = ref<BackupRestoreCandidate>()
const creatingBackup = ref(false)
const preparingRestore = ref(false)
const restoreDialogOpen = ref(false)
const restoring = ref(false)
const restoreTrigger = ref<HTMLButtonElement>()
const builtinSubjects = ['语文', '数学', '英语', '政治', '历史', '地理', '物理', '化学', '生物']
const subjectPreferences = ref<SubjectPreferences>()
const customSubject = ref('')
const savingSubjects = ref(false)
const subjectMessage = ref('')

function addCustomSubject() {
  const value = customSubject.value.trim()
  const preferences = subjectPreferences.value
  if (!preferences || !value || value.length > 40 || preferences.customSubjects.includes(value)) return
  if (preferences.customSubjects.length >= 20) {
    subjectMessage.value = '自定义科目最多 20 个。'
    return
  }
  preferences.customSubjects.push(value)
  if (!preferences.enabledSubjects.includes(value)) preferences.enabledSubjects.push(value)
  customSubject.value = ''
  subjectMessage.value = ''
}

function removeCustomSubject(subject: string) {
  const preferences = subjectPreferences.value
  if (!preferences) return
  preferences.customSubjects = preferences.customSubjects.filter(value => value !== subject)
  preferences.enabledSubjects = preferences.enabledSubjects.filter(value => value !== subject)
}

async function saveSubjectPreferences() {
  const preferences = subjectPreferences.value
  if (!preferences || savingSubjects.value) return
  if (!preferences.enabledSubjects.length) {
    subjectMessage.value = '至少保留一个常用科目。'
    return
  }
  savingSubjects.value = true
  subjectMessage.value = ''
  try {
    const result = normalizeAppResult(await commands.subjectPreferencesSave({
      enabledSubjects: preferences.enabledSubjects,
      customSubjects: preferences.customSubjects,
      captureSoundEnabled: preferences.captureSoundEnabled,
    }))
    if (result.ok) {
      subjectPreferences.value = result.data
      subjectMessage.value = '科目配置已保存'
    }
    else subjectMessage.value = result.error.userMessage
  }
  catch {
    subjectMessage.value = '科目配置没有保存成功，原有配置保持不变。'
  }
  finally {
    savingSubjects.value = false
  }
}

function formatBytes(bytes: number | null) {
  if (bytes === null) return '未知大小'
  if (bytes < 1024) return `${bytes} B`
  if (bytes < 1024 * 1024) return `${(bytes / 1024).toFixed(1)} KB`
  if (bytes < 1024 * 1024 * 1024) return `${(bytes / 1024 / 1024).toFixed(1)} MB`
  return `${(bytes / 1024 / 1024 / 1024).toFixed(2)} GB`
}

function formatBackupTime(timestamp: number | null) {
  if (timestamp === null) return '时间未知'
  return new Intl.DateTimeFormat('zh-CN', {
    dateStyle: 'medium',
    timeStyle: 'short',
  }).format(new Date(timestamp))
}

async function createBackup() {
  creatingBackup.value = true
  errorMessage.value = ''
  try {
    const invocation = await commands.backupCreate()
    if (invocation.status === 'error') throw new Error('backup command rejected')
    const result = normalizeAppResult(invocation.data)
    if (result.ok) {
      if (result.data) createdBackup.value = result.data
    }
    else {
      createdBackup.value = undefined
      errorMessage.value = result.error.userMessage
    }
  }
  catch {
    createdBackup.value = undefined
    errorMessage.value = '加密备份没有完成，现有资料库未被替换，请检查磁盘空间后重试。'
  }
  finally {
    creatingBackup.value = false
  }
}

async function prepareRestore() {
  preparingRestore.value = true
  errorMessage.value = ''
  try {
    const invocation = await commands.backupPrepareRestore()
    if (invocation.status === 'error') throw new Error('backup command rejected')
    const result = normalizeAppResult(invocation.data)
    if (result.ok) {
      if (result.data) restoreCandidate.value = result.data
    }
    else {
      restoreCandidate.value = undefined
      errorMessage.value = result.error.userMessage
    }
  }
  catch {
    restoreCandidate.value = undefined
    errorMessage.value = '备份包没有验证成功；现有资料库未被修改，请稍后重试。'
  }
  finally {
    preparingRestore.value = false
  }
}

async function confirmRestore() {
  const candidate = restoreCandidate.value
  if (!candidate || restoring.value) return
  restoring.value = true
  errorMessage.value = ''
  try {
    const invocation = await commands.backupRestore(candidate.id)
    if (invocation.status === 'error') throw new Error('restore command rejected')
    const result = normalizeAppResult(invocation.data)
    if (!result.ok) {
      errorMessage.value = result.error.userMessage
      restoring.value = false
      await closeRestoreDialog()
    }
  }
  catch {
    errorMessage.value = '恢复任务没有开始；当前资料库保持不变，请稍后重试。'
    restoring.value = false
    await closeRestoreDialog()
  }
}

async function closeRestoreDialog() {
  if (restoring.value) return
  restoreDialogOpen.value = false
  await nextTick()
  restoreTrigger.value?.focus()
}

function issueLabel(code: string) {
  return {
    missing_metadata: '缺少 metadata',
    invalid_metadata: 'metadata 损坏',
    missing_pair: '配对缺失',
    missing_relative_path: '路径缺失',
    unsafe_relative_path: '路径越界',
    unsafe_metadata_path: '元数据越界',
    unsafe_member_path: '档案目录越界',
    unsafe_files_root: '图片目录越界',
    unsafe_asset_path: '资源越界',
    missing_asset: '图片缺失',
    metadata_too_large: '元数据过大',
    asset_too_large: '图片过大',
    scan_limit_exceeded: '扫描已截断',
    hash_mismatch: '哈希不一致',
    duplicate_asset: '重复图片',
    unreadable_asset: '图片不可读',
  }[code] ?? code
}

async function scanLegacy() {
  scanningLegacy.value = true
  errorMessage.value = ''
  try {
    const result = normalizeAppResult(await commands.legacyScan())
    if (result.ok) {
      if (result.data) legacyReport.value = result.data
    }
    else {
      legacyReport.value = undefined
      errorMessage.value = result.error.userMessage
    }
  }
  catch {
    legacyReport.value = undefined
    errorMessage.value = '旧版目录没有扫描成功；原目录未被修改，请稍后重试。'
  }
  finally {
    scanningLegacy.value = false
  }
}

async function load() {
  loading.value = true
  errorMessage.value = ''
  try {
    if (!isTauri()) return
    const result = normalizeAppResult(await commands.settingsOverview())
    if (result.ok) overview.value = result.data
    else errorMessage.value = result.error.userMessage
    const preferenceResult = normalizeAppResult(await commands.subjectPreferencesGet())
    if (preferenceResult.ok) subjectPreferences.value = preferenceResult.data
    else errorMessage.value = preferenceResult.error.userMessage
  }
  catch {
    errorMessage.value = '设置状态暂时无法读取，请重新打开应用后再试。'
  }
  finally {
    loading.value = false
  }
}

onMounted(load)
</script>

<template>
  <main class="settings-page">
    <header>
      <div><p>设置 · 本地资料库</p><h1>数据安静地待在该在的地方</h1><span>这里展示真实状态；尚未接通的云能力会明确标注。</span></div>
      <button
        type="button"
        :disabled="loading"
        @click="load"
      >
        <RotateCcw :size="16" />刷新
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
      v-if="overview"
      class="settings-grid"
      :aria-busy="loading"
    >
      <article class="setting-card encryption-card">
        <div class="icon">
          <LockKeyhole :size="22" />
        </div><div><p>本地加密</p><h2>{{ overview?.localEncryptionReady ? 'SQLCipher 已启用' : '正在检查' }}</h2><span>数据库与原图 blob 使用独立密钥；显式退出后的锁定策略将在账户模块接入。</span></div>
        <strong><ShieldCheck :size="15" />本机保护</strong>
      </article>
      <article class="setting-card">
        <div class="icon">
          <Database :size="22" />
        </div><div><p>题库状态</p><h2>{{ overview?.activeProblemCount ?? 0 }} 道活动题</h2><span><Archive :size="13" /> {{ overview?.archivedProblemCount ?? 0 }} 道归档 · <Trash2 :size="13" /> {{ overview?.trashedProblemCount ?? 0 }} 道回收站</span></div>
      </article>
      <article class="setting-card">
        <div class="icon">
          <CloudOff :size="22" />
        </div><div><p>云端同步</p><h2>{{ overview?.cloudSyncConfigured ? '已配置' : '尚未配置' }}</h2><span>本地 outbox 已记录 {{ overview?.pendingOperationCount ?? 0 }} 项变更；Supabase 凭据与远端 RPC 仍待接入。</span></div>
      </article>
      <article class="setting-card">
        <div class="icon">
          <TriangleAlert :size="22" />
        </div><div><p>需要关注</p><h2>{{ (overview?.failedOperationCount ?? 0) + (overview?.unresolvedConflictCount ?? 0) }} 项</h2><span>{{ overview?.failedOperationCount ?? 0 }} 项失败操作 · {{ overview?.unresolvedConflictCount ?? 0 }} 项未解决冲突</span></div>
      </article>
    </section>
    <p
      v-else-if="loading"
      class="state-copy"
      role="status"
    >
      正在检查本地资料库状态…
    </p>
    <p
      v-else
      class="state-copy"
    >
      资料库状态没有读取成功，页面不会用默认数字代替真实状态。
    </p>

    <section
      v-if="subjectPreferences"
      class="subject-panel"
      aria-labelledby="subject-settings-title"
    >
      <header>
        <div>
          <p>采集偏好 · 当前学习档案</p>
          <h2 id="subject-settings-title">
            常用科目
          </h2>
          <span>勾选会出现在采集台顶部；一键即可把整批题统一设置为同一科目。</span>
        </div>
        <BookOpen :size="24" />
      </header>
      <div class="builtin-subjects">
        <label
          v-for="subject in builtinSubjects"
          :key="subject"
          :class="{ selected: subjectPreferences.enabledSubjects.includes(subject) }"
        >
          <input
            v-model="subjectPreferences.enabledSubjects"
            type="checkbox"
            :value="subject"
            :disabled="subjectPreferences.enabledSubjects.length === 1 && subjectPreferences.enabledSubjects.includes(subject)"
          >
          <span>{{ subject }}</span>
        </label>
      </div>
      <div class="custom-subjects">
        <span
          v-for="subject in subjectPreferences.customSubjects"
          :key="subject"
        >{{ subject }}<button
          type="button"
          :aria-label="`删除自定义科目 ${subject}`"
          @click="removeCustomSubject(subject)"
        ><Trash2 :size="13" /></button></span>
      </div>
      <div class="subject-controls">
        <form @submit.prevent="addCustomSubject">
          <input
            v-model="customSubject"
            maxlength="40"
            placeholder="例如：编程、竞赛数学"
          >
          <button
            type="submit"
            aria-label="添加自定义科目"
          >
            <Plus :size="15" />添加
          </button>
        </form>
        <label class="sound-toggle">
          <input
            v-model="subjectPreferences.captureSoundEnabled"
            type="checkbox"
          >
          <Volume2 :size="16" /><span><strong>拖放成功音效</strong><small>仅在题卡融合成功后播放短提示音</small></span>
        </label>
        <button
          type="button"
          class="save-subjects"
          :disabled="savingSubjects"
          @click="saveSubjectPreferences"
        >
          {{ savingSubjects ? '保存中…' : '保存科目配置' }}
        </button>
      </div>
      <p
        v-if="subjectMessage"
        class="subject-message"
        role="status"
      >
        {{ subjectMessage }}
      </p>
    </section>

    <section class="backup-panel">
      <header>
        <div><p>备份与恢复 · 本机加密</p><h2>完整快照、双重校验与自动回滚</h2><span>数据库和原图密文会一起备份；清单仅记录密文哈希与大小，不包含题图明文、本机绝对路径或原始账户标识。</span></div>
        <div class="backup-actions">
          <button
            type="button"
            :disabled="creatingBackup || preparingRestore || restoring"
            @click="createBackup"
          >
            <ArchiveRestore :size="16" />{{ creatingBackup ? '正在创建…' : '创建加密备份' }}
          </button>
          <button
            type="button"
            :disabled="creatingBackup || preparingRestore || restoring"
            @click="prepareRestore"
          >
            <FolderCheck :size="16" />{{ preparingRestore ? '正在校验并暂存…' : '选择备份并准备恢复' }}
          </button>
        </div>
      </header>
      <p class="backup-boundary">
        当前备份依赖这台可信 Windows 设备保存的加密凭据；跨设备恢复将在账户同步阶段使用正式密钥封装接入，不使用弱口令派生方案。
      </p>
      <div
        v-if="createdBackup || restoreCandidate"
        class="backup-results"
      >
        <article v-if="createdBackup">
          <strong><ShieldCheck :size="16" />加密备份已创建</strong>
          <span>{{ createdBackup.label }}</span>
          <small>{{ formatBackupTime(createdBackup.createdAtUtcMs) }} · {{ createdBackup.assetCount }} 个资源 · {{ formatBytes(createdBackup.encryptedBytes) }}</small>
        </article>
        <article
          v-if="restoreCandidate"
          class="restore-ready-card"
        >
          <strong><FolderCheck :size="16" />安全恢复包已就绪</strong>
          <span>{{ restoreCandidate.summary.label }}</span>
          <small>{{ restoreCandidate.summary.assetCount }} 个资源 · {{ formatBytes(restoreCandidate.summary.encryptedBytes) }} · 已复制到隔离区并再次校验，当前资料库尚未改变</small>
          <button
            ref="restoreTrigger"
            type="button"
            :disabled="restoring"
            @click="restoreDialogOpen = true"
          >
            查看风险并确认恢复
          </button>
        </article>
      </div>
    </section>

    <section class="migration-panel">
      <header>
        <div><p>旧版迁移 · 只读预检</p><h2>先看清旧数据，再决定是否导入</h2><span>扫描只读取 metadata 与图片哈希，不会移动、重命名或删除旧目录中的任何文件。</span></div>
        <button
          type="button"
          :disabled="scanningLegacy"
          @click="scanLegacy"
        >
          {{ scanningLegacy ? '正在扫描…' : '选择旧版目录并扫描' }}
        </button>
      </header>
      <template v-if="legacyReport">
        <dl class="migration-stats">
          <div><dt>学习档案</dt><dd>{{ legacyReport.members }}</dd></div>
          <div><dt>元数据记录</dt><dd>{{ legacyReport.metadataRecords }}</dd></div>
          <div><dt>可读图片</dt><dd>{{ legacyReport.existingAssets }}</dd></div>
          <div><dt>训练记录</dt><dd>{{ legacyReport.trainingRecords }}</dd></div>
          <div><dt>重复图片</dt><dd>{{ legacyReport.duplicateAssets }}</dd></div>
          <div><dt>待处理问题</dt><dd>{{ legacyReport.issues.length }}</dd></div>
        </dl>
        <p class="preflight-note">
          当前阶段只生成预检报告，尚未写入新题库。
        </p>
        <p
          v-if="legacyReport.truncated"
          class="preflight-note warning"
        >
          数据量超过安全扫描上限；下面是截断报告，请先处理标记项再导入。
        </p>
        <ol
          v-if="legacyReport.issues.length"
          class="issue-list"
        >
          <li
            v-for="(issue, index) in legacyReport.issues.slice(0, 50)"
            :key="`${issue.member}-${issue.recordId}-${index}`"
          >
            <strong>{{ issueLabel(issue.code) }}</strong>
            <span>{{ issue.member }}<template v-if="issue.recordId"> · {{ issue.recordId }}</template></span>
            <small>{{ issue.detail }}</small>
          </li>
        </ol>
        <p
          v-if="legacyReport.issues.length > 50"
          class="preflight-note"
        >
          当前页面仅展示前 50 项；完整扫描共发现 {{ legacyReport.issues.length }} 项问题。
        </p>
        <p
          v-if="legacyReport.issues.length === 0"
          class="preflight-note ready"
        >
          未发现缺失、损坏或越界记录，可以进入复制与校验阶段。
        </p>
      </template>
    </section>

    <section class="roadmap-panel">
      <div><p>安全底座</p><h2>恢复已接通，设备与同步继续建立在真实状态上</h2></div>
      <ol>
        <li><strong>已接通 · 恢复与回滚</strong><span>备份经两次校验后自动重启切换；新资料库打不开时恢复原资料。</span></li>
        <li><strong>可信设备</strong><span>显示设备、最后同步时间，并支持撤销离线解锁。</span></li>
        <li><strong>冲突中心</strong><span>只呈现同字段真冲突；不同字段自动合并。</span></li>
      </ol>
    </section>

    <BackupRestoreDialog
      v-if="restoreDialogOpen && restoreCandidate"
      :candidate="restoreCandidate"
      :busy="restoring"
      @cancel="closeRestoreDialog"
      @confirm="confirmRestore"
    />
  </main>
</template>

<style scoped>
.settings-page { min-height: 100vh; padding: 42px clamp(24px,5vw,72px) 72px; background: radial-gradient(circle at 85% 0,rgba(33,51,45,.08),transparent 32%); }
header { display: flex; justify-content: space-between; gap: 24px; align-items: flex-start; margin-bottom: 28px; } header p, .roadmap-panel p, .migration-panel header p { margin: 0 0 8px; color: var(--cinnabar); font-size: 12px; font-weight: 800; letter-spacing: .14em; text-transform: uppercase; }
h1, h2 { margin: 0; font-family: Georgia,'Microsoft YaHei',serif; color: var(--green-deep); } h1 { font-size: clamp(28px,4vw,42px); } h2 { font-size: 21px; } header span { display: block; margin-top: 9px; color: var(--ink-muted); }
button { display: inline-flex; gap: 7px; align-items: center; padding: 10px 14px; border: 1px solid var(--line); border-radius: 10px; background: var(--paper-raised); cursor: pointer; } button:disabled { opacity: .5; }
.error-banner { padding: 12px; border-radius: 10px; background: rgba(185,88,63,.08); color: #843d2c; }
.state-copy { padding: 28px; border: 1px solid var(--line); border-radius: 14px; background: rgba(255,253,247,.7); color: var(--ink-muted); text-align: center; }
.settings-grid { display: grid; grid-template-columns: repeat(2,minmax(0,1fr)); gap: 15px; }.setting-card, .roadmap-panel, .migration-panel, .backup-panel, .subject-panel { border: 1px solid var(--line); border-radius: 17px; background: rgba(255,253,247,.78); box-shadow: 0 16px 48px rgba(34,48,43,.05); }
.setting-card { position: relative; display: grid; grid-template-columns: 48px 1fr; gap: 14px; min-height: 150px; padding: 22px; }.setting-card .icon { display: grid; width: 44px; height: 44px; place-items: center; border-radius: 13px; background: var(--green-soft); color: var(--green-deep); }.setting-card p { margin: 1px 0 8px; color: var(--ink-muted); font-size: 11px; font-weight: 800; letter-spacing: .08em; text-transform: uppercase; }.setting-card span { display: flex; gap: 5px; align-items: center; margin-top: 10px; color: var(--ink-muted); font-size: 12px; line-height: 1.7; }.setting-card > strong { position: absolute; right: 18px; bottom: 16px; display: flex; gap: 5px; align-items: center; color: #557263; font-size: 10px; }
.encryption-card { border-color: rgba(33,51,45,.28); }.roadmap-panel { margin-top: 16px; padding: 26px; }.roadmap-panel ol { display: grid; grid-template-columns: repeat(3,1fr); gap: 12px; margin: 22px 0 0; padding: 0; list-style: none; }.roadmap-panel li { padding: 17px; border-radius: 12px; background: rgba(232,221,199,.34); }.roadmap-panel strong, .roadmap-panel span { display: block; }.roadmap-panel span { margin-top: 7px; color: var(--ink-muted); font-size: 12px; line-height: 1.65; }
.subject-panel { margin-top: 16px; padding: 26px; }.subject-panel>header { align-items: center; margin-bottom: 18px; }.subject-panel>header p { margin: 0 0 8px; color: var(--cinnabar); font-size: 12px; font-weight: 800; letter-spacing: .14em; }.builtin-subjects { display: flex; gap: 9px; flex-wrap: wrap; }.builtin-subjects label { position: relative; cursor: pointer; }.builtin-subjects input { position: absolute; opacity: 0; pointer-events: none; }.builtin-subjects span { display: grid; min-width: 58px; min-height: 38px; padding: 0 13px; place-items: center; color: var(--ink-muted); border: 1px solid var(--line); border-radius: 999px; background: var(--paper); transition: transform var(--motion-feedback), color var(--motion-feedback), background var(--motion-feedback); }.builtin-subjects label.selected span { color: var(--paper); border-color: var(--green-deep); background: var(--green-deep); }.builtin-subjects label:hover span { transform: translateY(-1px); }.builtin-subjects input:focus-visible+span { outline: 3px solid rgba(185,88,63,.24); outline-offset: 2px; }.custom-subjects { display: flex; gap: 8px; flex-wrap: wrap; min-height: 8px; margin-top: 12px; }.custom-subjects>span { display: inline-flex; gap: 5px; align-items: center; padding: 6px 7px 6px 11px; color: #7e412f; border-radius: 999px; background: rgba(185,88,63,.1); font-size: 11px; }.custom-subjects button { display: grid; width: 25px; height: 25px; padding: 0; place-items: center; border: 0; border-radius: 50%; background: transparent; }.subject-controls { display: grid; grid-template-columns: minmax(280px,1fr) minmax(250px,1fr) auto; gap: 12px; align-items: center; margin-top: 17px; }.subject-controls form { display: flex; gap: 8px; }.subject-controls form input { flex: 1; min-width: 0; padding: 10px 12px; border: 1px solid var(--line); border-radius: 10px; background: var(--paper); }.sound-toggle { display: flex; gap: 9px; align-items: center; padding: 8px 11px; border: 1px solid var(--line); border-radius: 11px; cursor: pointer; }.sound-toggle>span { display: grid; }.sound-toggle small { color: var(--ink-muted); font-size: 9px; }.save-subjects { color: var(--paper); border-color: var(--green-deep); background: var(--green-deep); }.subject-message { margin: 12px 0 0; color: #557263; font-size: 12px; }
.migration-panel { margin-top: 16px; padding: 26px; }.migration-panel header { margin-bottom: 20px; }.migration-panel header span { max-width: 680px; }.migration-panel header button { flex: 0 0 auto; }.migration-stats { display: grid; grid-template-columns: repeat(6,minmax(0,1fr)); gap: 10px; margin: 0; }.migration-stats div { padding: 15px; border-radius: 12px; background: rgba(232,221,199,.34); }.migration-stats dt { color: var(--ink-muted); font-size: 11px; }.migration-stats dd { margin: 6px 0 0; color: var(--green-deep); font-family: Georgia,serif; font-size: 25px; font-weight: 700; }.preflight-note { margin: 16px 0 0; color: var(--ink-muted); font-size: 12px; }.preflight-note.ready { color: #557263; }.issue-list { display: grid; gap: 8px; max-height: 330px; margin: 16px 0 0; padding: 0; overflow: auto; list-style: none; }.issue-list li { display: grid; grid-template-columns: 108px minmax(120px,.6fr) 1fr; gap: 12px; align-items: start; padding: 12px 14px; border-radius: 10px; background: rgba(185,88,63,.06); }.issue-list strong { color: #843d2c; font-size: 12px; }.issue-list span, .issue-list small { color: var(--ink-muted); font-size: 11px; overflow-wrap: anywhere; }
.backup-panel { margin-top: 16px; padding: 26px; }.backup-panel header { margin-bottom: 16px; }.backup-panel header p { margin: 0 0 8px; color: var(--cinnabar); font-size: 12px; font-weight: 800; letter-spacing: .14em; text-transform: uppercase; }.backup-panel header span { max-width: 690px; }.backup-actions { display: flex; flex: 0 0 auto; gap: 8px; }.backup-boundary { margin: 0; padding: 13px 15px; border-left: 3px solid var(--cinnabar); border-radius: 7px; background: rgba(232,221,199,.28); color: var(--ink-muted); font-size: 12px; line-height: 1.7; }.backup-results { display: grid; grid-template-columns: repeat(2,minmax(0,1fr)); gap: 10px; margin-top: 14px; }.backup-results article { display: grid; gap: 6px; padding: 15px; border-radius: 12px; background: rgba(33,51,45,.055); }.backup-results strong { display: flex; gap: 6px; align-items: center; color: #557263; font-size: 13px; }.backup-results span { color: var(--green-deep); font-size: 12px; overflow-wrap: anywhere; }.backup-results small { color: var(--ink-muted); line-height: 1.6; }
.backup-results .restore-ready-card button { width: fit-content; margin-top: 5px; color: #fffdf7; border-color: var(--cinnabar); background: var(--cinnabar); transition: transform var(--motion-feedback), box-shadow var(--motion-feedback); }.backup-results .restore-ready-card button:hover { transform: translateY(-1px); box-shadow: 0 8px 20px rgba(185,88,63,.18); }
.preflight-note.warning { color: #843d2c; font-weight: 700; }
.builtin-subjects input { width: 1px; height: 1px; pointer-events: auto; }
@media (max-width: 980px) { .migration-stats { grid-template-columns: repeat(3,minmax(0,1fr)); }.subject-controls { grid-template-columns: 1fr; } }
@media (max-width: 760px) { .settings-page { padding: 24px 16px 92px; } .settings-grid { grid-template-columns: 1fr; } .roadmap-panel ol, .migration-stats, .backup-results { grid-template-columns: 1fr; } .migration-panel header, .backup-panel header { flex-direction: column; }.backup-actions { width: 100%; flex-direction: column; }.backup-actions button { justify-content: center; }.issue-list li { grid-template-columns: 1fr; gap: 4px; } }
</style>
