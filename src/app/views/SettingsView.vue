<script setup lang="ts">
import { isTauri } from '@tauri-apps/api/core'
import { Archive, CloudOff, Database, LockKeyhole, RotateCcw, ShieldCheck, Trash2, TriangleAlert } from '@lucide/vue'
import { onMounted, ref } from 'vue'
import { commands, type SettingsOverview } from '../../shared/api/bindings'
import { normalizeAppResult } from '../../shared/api/normalize-result'

const overview = ref<SettingsOverview>()
const loading = ref(true)
const errorMessage = ref('')

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
header { display: flex; justify-content: space-between; gap: 24px; align-items: flex-start; margin-bottom: 28px; } header p, .roadmap-panel p { margin: 0 0 8px; color: var(--cinnabar); font-size: 12px; font-weight: 800; letter-spacing: .14em; text-transform: uppercase; }
h1, h2 { margin: 0; font-family: Georgia,'Microsoft YaHei',serif; color: var(--green-deep); } h1 { font-size: clamp(28px,4vw,42px); } h2 { font-size: 21px; } header span { display: block; margin-top: 9px; color: var(--ink-muted); }
button { display: inline-flex; gap: 7px; align-items: center; padding: 10px 14px; border: 1px solid var(--line); border-radius: 10px; background: var(--paper-raised); cursor: pointer; } button:disabled { opacity: .5; }
.error-banner { padding: 12px; border-radius: 10px; background: rgba(185,88,63,.08); color: #843d2c; }
.state-copy { padding: 28px; border: 1px solid var(--line); border-radius: 14px; background: rgba(255,253,247,.7); color: var(--ink-muted); text-align: center; }
.settings-grid { display: grid; grid-template-columns: repeat(2,minmax(0,1fr)); gap: 15px; }.setting-card, .roadmap-panel { border: 1px solid var(--line); border-radius: 17px; background: rgba(255,253,247,.78); box-shadow: 0 16px 48px rgba(34,48,43,.05); }
.setting-card { position: relative; display: grid; grid-template-columns: 48px 1fr; gap: 14px; min-height: 150px; padding: 22px; }.setting-card .icon { display: grid; width: 44px; height: 44px; place-items: center; border-radius: 13px; background: var(--green-soft); color: var(--green-deep); }.setting-card p { margin: 1px 0 8px; color: var(--ink-muted); font-size: 11px; font-weight: 800; letter-spacing: .08em; text-transform: uppercase; }.setting-card span { display: flex; gap: 5px; align-items: center; margin-top: 10px; color: var(--ink-muted); font-size: 12px; line-height: 1.7; }.setting-card > strong { position: absolute; right: 18px; bottom: 16px; display: flex; gap: 5px; align-items: center; color: #557263; font-size: 10px; }
.encryption-card { border-color: rgba(33,51,45,.28); }.roadmap-panel { margin-top: 16px; padding: 26px; }.roadmap-panel ol { display: grid; grid-template-columns: repeat(3,1fr); gap: 12px; margin: 22px 0 0; padding: 0; list-style: none; }.roadmap-panel li { padding: 17px; border-radius: 12px; background: rgba(232,221,199,.34); }.roadmap-panel strong, .roadmap-panel span { display: block; }.roadmap-panel span { margin-top: 7px; color: var(--ink-muted); font-size: 12px; line-height: 1.65; }
@media (max-width: 760px) { .settings-page { padding: 24px 16px 92px; } .settings-grid { grid-template-columns: 1fr; } .roadmap-panel ol { grid-template-columns: 1fr; } }
</style>
