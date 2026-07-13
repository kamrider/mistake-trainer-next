<script setup lang="ts">
import { isTauri } from '@tauri-apps/api/core'
import { Archive, CloudOff, Database, LockKeyhole, RotateCcw, ShieldCheck, Trash2, TriangleAlert } from '@lucide/vue'
import { onMounted, ref } from 'vue'
import { commands, type LegacyScanReport, type SettingsOverview } from '../../shared/api/bindings'
import { normalizeAppResult } from '../../shared/api/normalize-result'

const overview = ref<SettingsOverview>()
const loading = ref(true)
const errorMessage = ref('')
const legacyReport = ref<LegacyScanReport>()
const scanningLegacy = ref(false)

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
      <div><p>下一步接线</p><h2>备份、设备与同步会建立在同一套真实状态上</h2></div>
      <ol>
        <li><strong>备份恢复</strong><span>加密数据库 + blob 清单，恢复前先完整校验。</span></li>
        <li><strong>可信设备</strong><span>显示设备、最后同步时间，并支持撤销离线解锁。</span></li>
        <li><strong>冲突中心</strong><span>只呈现同字段真冲突；不同字段自动合并。</span></li>
      </ol>
    </section>
  </main>
</template>

<style scoped>
.settings-page { min-height: 100vh; padding: 42px clamp(24px,5vw,72px) 72px; background: radial-gradient(circle at 85% 0,rgba(33,51,45,.08),transparent 32%); }
header { display: flex; justify-content: space-between; gap: 24px; align-items: flex-start; margin-bottom: 28px; } header p, .roadmap-panel p, .migration-panel header p { margin: 0 0 8px; color: var(--cinnabar); font-size: 12px; font-weight: 800; letter-spacing: .14em; text-transform: uppercase; }
h1, h2 { margin: 0; font-family: Georgia,'Microsoft YaHei',serif; color: var(--green-deep); } h1 { font-size: clamp(28px,4vw,42px); } h2 { font-size: 21px; } header span { display: block; margin-top: 9px; color: var(--ink-muted); }
button { display: inline-flex; gap: 7px; align-items: center; padding: 10px 14px; border: 1px solid var(--line); border-radius: 10px; background: var(--paper-raised); cursor: pointer; } button:disabled { opacity: .5; }
.error-banner { padding: 12px; border-radius: 10px; background: rgba(185,88,63,.08); color: #843d2c; }
.state-copy { padding: 28px; border: 1px solid var(--line); border-radius: 14px; background: rgba(255,253,247,.7); color: var(--ink-muted); text-align: center; }
.settings-grid { display: grid; grid-template-columns: repeat(2,minmax(0,1fr)); gap: 15px; }.setting-card, .roadmap-panel, .migration-panel { border: 1px solid var(--line); border-radius: 17px; background: rgba(255,253,247,.78); box-shadow: 0 16px 48px rgba(34,48,43,.05); }
.setting-card { position: relative; display: grid; grid-template-columns: 48px 1fr; gap: 14px; min-height: 150px; padding: 22px; }.setting-card .icon { display: grid; width: 44px; height: 44px; place-items: center; border-radius: 13px; background: var(--green-soft); color: var(--green-deep); }.setting-card p { margin: 1px 0 8px; color: var(--ink-muted); font-size: 11px; font-weight: 800; letter-spacing: .08em; text-transform: uppercase; }.setting-card span { display: flex; gap: 5px; align-items: center; margin-top: 10px; color: var(--ink-muted); font-size: 12px; line-height: 1.7; }.setting-card > strong { position: absolute; right: 18px; bottom: 16px; display: flex; gap: 5px; align-items: center; color: #557263; font-size: 10px; }
.encryption-card { border-color: rgba(33,51,45,.28); }.roadmap-panel { margin-top: 16px; padding: 26px; }.roadmap-panel ol { display: grid; grid-template-columns: repeat(3,1fr); gap: 12px; margin: 22px 0 0; padding: 0; list-style: none; }.roadmap-panel li { padding: 17px; border-radius: 12px; background: rgba(232,221,199,.34); }.roadmap-panel strong, .roadmap-panel span { display: block; }.roadmap-panel span { margin-top: 7px; color: var(--ink-muted); font-size: 12px; line-height: 1.65; }
.migration-panel { margin-top: 16px; padding: 26px; }.migration-panel header { margin-bottom: 20px; }.migration-panel header span { max-width: 680px; }.migration-panel header button { flex: 0 0 auto; }.migration-stats { display: grid; grid-template-columns: repeat(6,minmax(0,1fr)); gap: 10px; margin: 0; }.migration-stats div { padding: 15px; border-radius: 12px; background: rgba(232,221,199,.34); }.migration-stats dt { color: var(--ink-muted); font-size: 11px; }.migration-stats dd { margin: 6px 0 0; color: var(--green-deep); font-family: Georgia,serif; font-size: 25px; font-weight: 700; }.preflight-note { margin: 16px 0 0; color: var(--ink-muted); font-size: 12px; }.preflight-note.ready { color: #557263; }.issue-list { display: grid; gap: 8px; max-height: 330px; margin: 16px 0 0; padding: 0; overflow: auto; list-style: none; }.issue-list li { display: grid; grid-template-columns: 108px minmax(120px,.6fr) 1fr; gap: 12px; align-items: start; padding: 12px 14px; border-radius: 10px; background: rgba(185,88,63,.06); }.issue-list strong { color: #843d2c; font-size: 12px; }.issue-list span, .issue-list small { color: var(--ink-muted); font-size: 11px; overflow-wrap: anywhere; }
.preflight-note.warning { color: #843d2c; font-weight: 700; }
@media (max-width: 980px) { .migration-stats { grid-template-columns: repeat(3,minmax(0,1fr)); } }
@media (max-width: 760px) { .settings-page { padding: 24px 16px 92px; } .settings-grid { grid-template-columns: 1fr; } .roadmap-panel ol, .migration-stats { grid-template-columns: 1fr; } .migration-panel header { flex-direction: column; }.issue-list li { grid-template-columns: 1fr; gap: 4px; } }
</style>
