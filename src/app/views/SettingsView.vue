<script setup lang="ts">
import { isTauri } from '@tauri-apps/api/core'
import { Archive, ArchiveRestore, BookOpen, CloudOff, Database, FolderCheck, LockKeyhole, Plus, RotateCcw, ShieldCheck, Trash2, TriangleAlert, Volume2 } from '@lucide/vue'
import { nextTick, onMounted, ref } from 'vue'
import type { AppResult } from '../../shared/api/app-result'
import { commands, type BackupRestoreCandidate, type BackupSummary, type CloudBackendKind, type CloudBackendStatus, type ReviewFocusPolicy, type ReviewPreferences, type SettingsOverview, type SubjectPreferences } from '../../shared/api/bindings'
import { normalizeAppResult } from '../../shared/api/normalize-result'
import { backendKindLabel, backendStatusLabel, loadSyncBackendStatus, setSyncBackend } from '../../shared/api/sync-backend'
import LegacyImportPanel from '../../modules/legacy/components/LegacyImportPanel.vue'
import BackupRestoreDialog from '../BackupRestoreDialog.vue'

const overview = ref<SettingsOverview>()
const loading = ref(true)
const errorMessage = ref('')
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
const reviewPreferences = ref<ReviewPreferences>()
const savingReviewPreferences = ref(false)
const reviewPreferenceMessage = ref('')
const backendStatus = ref<AppResult<CloudBackendStatus>>()
const backendBusy = ref(false)
const backendMessage = ref('')
const backendOptions: Array<{ kind: CloudBackendKind; title: string; hint: string }> = [
  { kind: 'local-only', title: '仅本地（推荐）', hint: '训练、采集和图片都保存在这台 Windows 设备，不需要网络。' },
  { kind: 'supabase', title: 'Supabase', hint: '适合海外或开发环境；需要先配置服务地址和匿名密钥。' },
  { kind: 'tencent', title: '腾讯云（国内预留）', hint: '适合国内部署；当前版本只显示配置状态，尚未启用同步适配器。' },
]
const reviewFocusOptions: Array<{ value: ReviewFocusPolicy, title: string, hint: string }> = [
  { value: 'off', title: '关闭专注插曲', hint: '训练题之间不插入额外环节。' },
  { value: 'session_start', title: '每轮开始前 · 推荐', hint: '进入普通训练时先完成一次 1–25 视线热身，节奏最自然。' },
  { value: 'every_10', title: '每完成 10 题', hint: '保存第 10、20…题后短暂停一下，再继续下一题。' },
]

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

async function saveReviewPreferences() {
  const preferences = reviewPreferences.value
  if (!preferences || savingReviewPreferences.value) return
  savingReviewPreferences.value = true
  reviewPreferenceMessage.value = ''
  try {
    const result = normalizeAppResult(await commands.reviewPreferencesSave({
      focusPolicy: preferences.focusPolicy,
    }))
    if (result.ok) {
      reviewPreferences.value = result.data
      reviewPreferenceMessage.value = '训练节奏已保存，将从下一轮普通训练开始生效。'
    }
    else reviewPreferenceMessage.value = result.error.userMessage
  }
  catch {
    reviewPreferenceMessage.value = '训练节奏没有保存成功，原有配置保持不变。'
  }
  finally {
    savingReviewPreferences.value = false
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

async function load() {
  loading.value = true
  errorMessage.value = ''
  try {
    backendStatus.value = await loadSyncBackendStatus()
    if (!isTauri()) return
    const result = normalizeAppResult(await commands.settingsOverview())
    if (result.ok) overview.value = result.data
    else errorMessage.value = result.error.userMessage
    const preferenceResult = normalizeAppResult(await commands.subjectPreferencesGet())
    if (preferenceResult.ok) subjectPreferences.value = preferenceResult.data
    else errorMessage.value = preferenceResult.error.userMessage
    const reviewPreferenceResult = normalizeAppResult(await commands.reviewPreferencesGet())
    if (reviewPreferenceResult.ok) reviewPreferences.value = reviewPreferenceResult.data
    else errorMessage.value = reviewPreferenceResult.error.userMessage
  }
  catch {
    errorMessage.value = '设置状态暂时无法读取，请重新打开应用后再试。'
  }
  finally {
    loading.value = false
  }
}

async function chooseBackend(kind: CloudBackendKind) {
  const current = backendStatus.value
  if (backendBusy.value || (current?.ok && current.data.kind === kind)) return
  backendBusy.value = true
  backendMessage.value = ''
  try {
    const result = await setSyncBackend(kind)
    if (result.ok) {
      backendStatus.value = result
      backendMessage.value = `已选择 ${backendKindLabel(kind)}`
    }
    else {
      backendMessage.value = result.error.userMessage
    }
  }
  catch {
    backendMessage.value = '同步后端设置暂时不可用，本地数据不会受到影响。'
  }
  finally {
    backendBusy.value = false
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
      class="backend-panel"
      aria-labelledby="backend-settings-title"
      :aria-busy="backendBusy"
    >
      <header>
        <div>
          <p>同步后端 · 可替换的云服务</p>
          <h2 id="backend-settings-title">
            数据始终先保存在本地
          </h2>
          <span>选择只影响未来的同步尝试；未配置或未启用的远程服务不会上传任何数据。</span>
        </div>
        <CloudOff :size="24" />
      </header>
      <div
        v-if="backendStatus?.ok"
        class="backend-current"
        role="status"
        aria-live="polite"
      >
        <span
          class="backend-status-dot"
          :class="{ ready: backendStatus.data.ready }"
          aria-hidden="true"
        />
        <span>
          <strong>{{ backendStatusLabel(backendStatus.data) }}</strong>
          <small>{{ backendStatus.data.syncEnabled ? '云端同步已启用' : '云端同步未启用，本地 outbox 会安全保留' }}</small>
        </span>
      </div>
      <p
        v-else-if="backendStatus && !backendStatus.ok"
        class="backend-message warning"
        role="alert"
      >
        {{ backendStatus.error.userMessage }}
      </p>
      <div class="backend-options">
        <button
          v-for="option in backendOptions"
          :key="option.kind"
          type="button"
          :class="['backend-option', { selected: backendStatus?.ok && backendStatus.data.kind === option.kind }]"
          :disabled="backendBusy"
          @click="chooseBackend(option.kind)"
        >
          <span
            class="backend-option-mark"
            aria-hidden="true"
          />
          <span>
            <strong>{{ option.title }}</strong>
            <small>{{ option.hint }}</small>
          </span>
        </button>
      </div>
      <p
        v-if="backendMessage"
        class="backend-message"
        role="status"
      >
        {{ backendMessage }}
      </p>
    </section>

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

    <section
      v-if="reviewPreferences"
      class="review-rhythm-panel"
      aria-labelledby="review-rhythm-title"
    >
      <header>
        <div>
          <p>训练偏好 · 当前学习档案</p>
          <h2 id="review-rhythm-title">
            训练间的专注插曲
          </h2>
          <span>舒尔特 5×5 只改变普通训练节奏；每轮都能跳过，模拟考试不会插入专注环节。</span>
        </div>
      </header>
      <div class="rhythm-options">
        <label
          v-for="option in reviewFocusOptions"
          :key="option.value"
          :class="{ selected: reviewPreferences.focusPolicy === option.value }"
        >
          <input
            v-model="reviewPreferences.focusPolicy"
            type="radio"
            name="review-focus-policy"
            :value="option.value"
          >
          <span
            class="rhythm-mark"
            aria-hidden="true"
          />
          <span><strong>{{ option.title }}</strong><small>{{ option.hint }}</small></span>
        </label>
      </div>
      <footer class="rhythm-actions">
        <p
          v-if="reviewPreferenceMessage"
          role="status"
        >
          {{ reviewPreferenceMessage }}
        </p>
        <span v-else>正在进行的训练保持原节奏，避免中途突然改变。</span>
        <button
          type="button"
          :disabled="savingReviewPreferences"
          @click="saveReviewPreferences"
        >
          {{ savingReviewPreferences ? '保存中…' : '保存训练节奏' }}
        </button>
      </footer>
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

    <LegacyImportPanel @changed="load" />

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
.backend-panel { margin-bottom: 16px; padding: 24px 26px; border: 1px solid rgba(33,51,45,.22); border-radius: 17px; background: linear-gradient(135deg,rgba(255,253,247,.94),rgba(220,228,220,.34)); box-shadow: 0 16px 48px rgba(34,48,43,.05); }.backend-panel>header { align-items: center; margin-bottom: 16px; }.backend-panel>header>svg { color: var(--green-deep); }.backend-panel>header p { margin: 0 0 8px; color: var(--cinnabar); font-size: 12px; font-weight: 800; letter-spacing: .14em; }.backend-panel>header span { display: block; margin-top: 8px; color: var(--ink-muted); font-size: 12px; }.backend-current { display: flex; gap: 9px; align-items: center; margin-bottom: 13px; padding: 11px 13px; border-radius: 11px; background: rgba(33,51,45,.07); }.backend-current>span:last-child { display: grid; gap: 3px; }.backend-current strong { color: var(--green-deep); font-size: 13px; }.backend-current small { color: var(--ink-muted); font-size: 11px; }.backend-status-dot { width: 9px; height: 9px; border-radius: 50%; background: var(--cinnabar); box-shadow: 0 0 0 4px rgba(185,88,63,.13); }.backend-status-dot.ready { background: #557263; box-shadow: 0 0 0 4px rgba(85,114,99,.14); }.backend-options { display: grid; grid-template-columns: repeat(3,minmax(0,1fr)); gap: 10px; }.backend-option { display: grid; grid-template-columns: 17px 1fr; gap: 10px; align-items: start; min-height: 92px; padding: 14px; border: 1px solid var(--line); border-radius: 12px; background: rgba(255,253,247,.65); cursor: pointer; text-align: left; transition: transform var(--motion-feedback) var(--ease-standard), border-color var(--motion-standard) var(--ease-standard), background var(--motion-standard) var(--ease-standard), box-shadow var(--motion-standard) var(--ease-standard); }.backend-option:hover:not(:disabled) { transform: translateY(-2px); border-color: rgba(33,51,45,.38); box-shadow: 0 10px 24px rgba(34,48,43,.08); }.backend-option.selected { border-color: var(--green-deep); background: var(--green-soft); box-shadow: inset 0 0 0 1px var(--green-deep); }.backend-option:disabled { cursor: wait; }.backend-option-mark { width: 15px; height: 15px; margin-top: 2px; border: 1px solid var(--sand-deep); border-radius: 50%; background: var(--paper-raised); box-shadow: inset 0 0 0 4px var(--paper-raised); }.backend-option.selected .backend-option-mark { border-color: var(--green-deep); background: var(--green-deep); }.backend-option>span:last-child { display: grid; gap: 5px; }.backend-option strong { color: var(--green-deep); font-size: 13px; }.backend-option small { color: var(--ink-muted); font-size: 10px; line-height: 1.55; }.backend-message { margin: 12px 0 0; color: #557263; font-size: 11px; }.backend-message.warning { color: #843d2c; font-weight: 700; }.settings-grid { display: grid; grid-template-columns: repeat(2,minmax(0,1fr)); gap: 15px; }.setting-card, .roadmap-panel, .migration-panel, .backup-panel, .subject-panel, .review-rhythm-panel { border: 1px solid var(--line); border-radius: 17px; background: rgba(255,253,247,.78); box-shadow: 0 16px 48px rgba(34,48,43,.05); }
.setting-card { position: relative; display: grid; grid-template-columns: 48px 1fr; gap: 14px; min-height: 150px; padding: 22px; }.setting-card .icon { display: grid; width: 44px; height: 44px; place-items: center; border-radius: 13px; background: var(--green-soft); color: var(--green-deep); }.setting-card p { margin: 1px 0 8px; color: var(--ink-muted); font-size: 11px; font-weight: 800; letter-spacing: .08em; text-transform: uppercase; }.setting-card span { display: flex; gap: 5px; align-items: center; margin-top: 10px; color: var(--ink-muted); font-size: 12px; line-height: 1.7; }.setting-card > strong { position: absolute; right: 18px; bottom: 16px; display: flex; gap: 5px; align-items: center; color: #557263; font-size: 10px; }
.encryption-card { border-color: rgba(33,51,45,.28); }.roadmap-panel { margin-top: 16px; padding: 26px; }.roadmap-panel ol { display: grid; grid-template-columns: repeat(3,1fr); gap: 12px; margin: 22px 0 0; padding: 0; list-style: none; }.roadmap-panel li { padding: 17px; border-radius: 12px; background: rgba(232,221,199,.34); }.roadmap-panel strong, .roadmap-panel span { display: block; }.roadmap-panel span { margin-top: 7px; color: var(--ink-muted); font-size: 12px; line-height: 1.65; }
.subject-panel { margin-top: 16px; padding: 26px; }.subject-panel>header { align-items: center; margin-bottom: 18px; }.subject-panel>header p { margin: 0 0 8px; color: var(--cinnabar); font-size: 12px; font-weight: 800; letter-spacing: .14em; }.builtin-subjects { display: flex; gap: 9px; flex-wrap: wrap; }.builtin-subjects label { position: relative; cursor: pointer; }.builtin-subjects input { position: absolute; opacity: 0; pointer-events: none; }.builtin-subjects span { display: grid; min-width: 58px; min-height: 38px; padding: 0 13px; place-items: center; color: var(--ink-muted); border: 1px solid var(--line); border-radius: 999px; background: var(--paper); transition: transform var(--motion-feedback), color var(--motion-feedback), background var(--motion-feedback); }.builtin-subjects label.selected span { color: var(--paper); border-color: var(--green-deep); background: var(--green-deep); }.builtin-subjects label:hover span { transform: translateY(-1px); }.builtin-subjects input:focus-visible+span { outline: 3px solid rgba(185,88,63,.24); outline-offset: 2px; }.custom-subjects { display: flex; gap: 8px; flex-wrap: wrap; min-height: 8px; margin-top: 12px; }.custom-subjects>span { display: inline-flex; gap: 5px; align-items: center; padding: 6px 7px 6px 11px; color: #7e412f; border-radius: 999px; background: rgba(185,88,63,.1); font-size: 11px; }.custom-subjects button { display: grid; width: 25px; height: 25px; padding: 0; place-items: center; border: 0; border-radius: 50%; background: transparent; }.subject-controls { display: grid; grid-template-columns: minmax(280px,1fr) minmax(250px,1fr) auto; gap: 12px; align-items: center; margin-top: 17px; }.subject-controls form { display: flex; gap: 8px; }.subject-controls form input { flex: 1; min-width: 0; padding: 10px 12px; border: 1px solid var(--line); border-radius: 10px; background: var(--paper); }.sound-toggle { display: flex; gap: 9px; align-items: center; padding: 8px 11px; border: 1px solid var(--line); border-radius: 11px; cursor: pointer; }.sound-toggle>span { display: grid; }.sound-toggle small { color: var(--ink-muted); font-size: 9px; }.save-subjects { color: var(--paper); border-color: var(--green-deep); background: var(--green-deep); }.subject-message { margin: 12px 0 0; color: #557263; font-size: 12px; }
.review-rhythm-panel { margin-top: 16px; padding: 26px; }.review-rhythm-panel>header { margin-bottom: 18px; }.review-rhythm-panel>header p { margin: 0 0 8px; color: var(--cinnabar); font-size: 12px; font-weight: 800; letter-spacing: .14em; }.review-rhythm-panel>header span { max-width: 720px; }.rhythm-options { display: grid; grid-template-columns: repeat(3,minmax(0,1fr)); gap: 10px; }.rhythm-options label { position: relative; display: grid; min-height: 104px; grid-template-columns: 18px 1fr; gap: 10px; align-content: start; padding: 16px; border: 1px solid var(--line); border-radius: 7px 17px 17px; background: rgba(255,253,247,.68); cursor: pointer; transition: transform var(--motion-feedback) var(--ease-standard), border-color var(--motion-standard), background var(--motion-standard), box-shadow var(--motion-standard); }.rhythm-options label:hover { transform: translateY(-2px); }.rhythm-options label.selected { border-color: rgba(33,51,45,.4); background: var(--green-soft); box-shadow: 0 12px 28px rgba(33,51,45,.08); }.rhythm-options input { position: absolute; width: 1px; height: 1px; opacity: 0; }.rhythm-options input:focus-visible~.rhythm-mark { outline: 3px solid rgba(185,88,63,.35); outline-offset: 3px; }.rhythm-mark { width: 16px; height: 16px; margin-top: 2px; border: 1px solid var(--sand-deep); border-radius: 50%; background: var(--paper-raised); box-shadow: inset 0 0 0 4px var(--paper-raised); }.selected .rhythm-mark { border-color: var(--green-deep); background: var(--green-deep); }.rhythm-options label>span:last-child { display: grid; }.rhythm-options strong { color: var(--green-deep); font-size: 13px; }.rhythm-options small { margin-top: 7px; color: var(--ink-muted); font-size: 10px; line-height: 1.6; }.rhythm-actions { display: flex; gap: 16px; align-items: center; justify-content: space-between; margin-top: 16px; }.rhythm-actions p,.rhythm-actions span { margin: 0; color: #557263; font-size: 11px; }.rhythm-actions button { color: var(--paper); border-color: var(--green-deep); background: var(--green-deep); }
.migration-panel { margin-top: 16px; padding: 26px; }.migration-panel header { margin-bottom: 20px; }.migration-panel header span { max-width: 680px; }.migration-panel header button { flex: 0 0 auto; }.migration-stats { display: grid; grid-template-columns: repeat(6,minmax(0,1fr)); gap: 10px; margin: 0; }.migration-stats div { padding: 15px; border-radius: 12px; background: rgba(232,221,199,.34); }.migration-stats dt { color: var(--ink-muted); font-size: 11px; }.migration-stats dd { margin: 6px 0 0; color: var(--green-deep); font-family: Georgia,serif; font-size: 25px; font-weight: 700; }.preflight-note { margin: 16px 0 0; color: var(--ink-muted); font-size: 12px; }.preflight-note.ready { color: #557263; }.issue-list { display: grid; gap: 8px; max-height: 330px; margin: 16px 0 0; padding: 0; overflow: auto; list-style: none; }.issue-list li { display: grid; grid-template-columns: 108px minmax(120px,.6fr) 1fr; gap: 12px; align-items: start; padding: 12px 14px; border-radius: 10px; background: rgba(185,88,63,.06); }.issue-list strong { color: #843d2c; font-size: 12px; }.issue-list span, .issue-list small { color: var(--ink-muted); font-size: 11px; overflow-wrap: anywhere; }
.backup-panel { margin-top: 16px; padding: 26px; }.backup-panel header { margin-bottom: 16px; }.backup-panel header p { margin: 0 0 8px; color: var(--cinnabar); font-size: 12px; font-weight: 800; letter-spacing: .14em; text-transform: uppercase; }.backup-panel header span { max-width: 690px; }.backup-actions { display: flex; flex: 0 0 auto; gap: 8px; }.backup-boundary { margin: 0; padding: 13px 15px; border-left: 3px solid var(--cinnabar); border-radius: 7px; background: rgba(232,221,199,.28); color: var(--ink-muted); font-size: 12px; line-height: 1.7; }.backup-results { display: grid; grid-template-columns: repeat(2,minmax(0,1fr)); gap: 10px; margin-top: 14px; }.backup-results article { display: grid; gap: 6px; padding: 15px; border-radius: 12px; background: rgba(33,51,45,.055); }.backup-results strong { display: flex; gap: 6px; align-items: center; color: #557263; font-size: 13px; }.backup-results span { color: var(--green-deep); font-size: 12px; overflow-wrap: anywhere; }.backup-results small { color: var(--ink-muted); line-height: 1.6; }
.backup-results .restore-ready-card button { width: fit-content; margin-top: 5px; color: #fffdf7; border-color: var(--cinnabar); background: var(--cinnabar); transition: transform var(--motion-feedback), box-shadow var(--motion-feedback); }.backup-results .restore-ready-card button:hover { transform: translateY(-1px); box-shadow: 0 8px 20px rgba(185,88,63,.18); }
.preflight-note.warning { color: #843d2c; font-weight: 700; }
.builtin-subjects input { width: 1px; height: 1px; pointer-events: auto; }
@media (max-width: 980px) { .migration-stats { grid-template-columns: repeat(3,minmax(0,1fr)); }.subject-controls { grid-template-columns: 1fr; } }
@media (max-width: 760px) { .settings-page { padding: 24px 16px 92px; } .settings-grid { grid-template-columns: 1fr; } .backend-options { grid-template-columns: 1fr; } .roadmap-panel ol, .migration-stats, .backup-results, .rhythm-options { grid-template-columns: 1fr; } .migration-panel header, .backup-panel header { flex-direction: column; }.backup-actions { width: 100%; flex-direction: column; }.backup-actions button { justify-content: center; }.issue-list li { grid-template-columns: 1fr; gap: 4px; }.rhythm-actions { align-items: stretch; flex-direction: column; }.rhythm-actions button { justify-content: center; } }
@media (prefers-reduced-motion: reduce) { .backend-option { transition: none; }.backend-option:hover:not(:disabled) { transform: none; } }
</style>
