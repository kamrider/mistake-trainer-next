<script setup lang="ts">
import { ArchiveRestore, FolderCheck, ShieldCheck } from '@lucide/vue'
import { ref, watch } from 'vue'
import type {
  AutomaticBackupStatus,
  BackupRestoreCandidate,
  BackupSummary,
  PortableBackupReceipt,
} from '../../shared/api/bindings'
import { formatSettingsBytes, formatSettingsTime } from '../settings-formatters'

const props = defineProps<{
  created: BackupSummary | undefined
  candidate: BackupRestoreCandidate | undefined
  portableReceipt: PortableBackupReceipt | undefined
  automaticStatus: AutomaticBackupStatus | undefined
  automaticBusy: boolean
  creating: boolean
  creatingPortable: boolean
  preparing: boolean
  restoring: boolean
  message: string
}>()

const emit = defineEmits<{
  create: []
  createPortable: []
  dismissPortable: []
  preparePortable: [recoveryKey: string]
  configureAutomatic: [intervalDays: number, retentionCount: number]
  disableAutomatic: []
  prepare: []
  openRestore: []
}>()

const restoreAction = ref<HTMLButtonElement>()
const recoveryKey = ref('')
const intervalDays = ref(props.automaticStatus?.intervalDays ?? 7)
const retentionCount = ref(props.automaticStatus?.retentionCount ?? 5)

watch(() => props.automaticStatus, (status) => {
  if (!status) return
  intervalDays.value = status.intervalDays
  retentionCount.value = status.retentionCount
})

function focusRestoreAction() {
  restoreAction.value?.focus()
}

function submitPortableRestore() {
  const key = recoveryKey.value
  recoveryKey.value = ''
  emit('preparePortable', key)
}

defineExpose({ focusRestoreAction })
</script>

<template>
  <section
    id="settings-backup"
    class="backup-panel"
    aria-labelledby="backup-settings-title"
  >
    <header>
      <div>
        <p>备份与恢复 · 本机加密</p>
        <h2 id="backup-settings-title">
          完整快照、双重校验与自动回滚
        </h2>
        <span>数据库和原图密文会一起备份；清单仅记录密文哈希与大小，不包含题图明文、本机绝对路径或原始账户标识。</span>
      </div>
      <div class="backup-actions">
        <button
          type="button"
          :disabled="automaticBusy || creating || creatingPortable || preparing || restoring"
          @click="emit('create')"
        >
          <ArchiveRestore :size="16" />{{ creating ? '正在创建…' : '创建加密备份' }}
        </button>
        <button
          type="button"
          :disabled="automaticBusy || creating || creatingPortable || preparing || restoring"
          @click="emit('createPortable')"
        >
          <ShieldCheck :size="16" />{{ creatingPortable ? '正在封装备份…' : '创建便携加密备份' }}
        </button>
        <button
          type="button"
          :disabled="automaticBusy || creating || creatingPortable || preparing || restoring"
          @click="emit('prepare')"
        >
          <FolderCheck :size="16" />{{ preparing ? '正在校验并暂存…' : '选择备份并准备恢复' }}
        </button>
      </div>
    </header>
    <p class="backup-boundary">
      普通备份依赖这台可信 Windows 设备保存的加密凭据；便携备份额外使用一次性高熵恢复密钥封装凭据，不使用弱口令派生方案。
    </p>
    <section
      class="automatic-backup"
      aria-labelledby="automatic-backup-title"
    >
      <div>
        <strong id="automatic-backup-title">自动备份</strong>
        <p v-if="automaticStatus?.enabled">
          已启用 · {{ automaticStatus.destinationLabel }} · 每 {{ automaticStatus.intervalDays }} 天 · 保留 {{ automaticStatus.retentionCount }} 份
        </p>
        <p v-else>
          应用启动时检查是否到期；自动备份保存在独立目录，不会清理手动或便携备份。
        </p>
      </div>
      <label>
        间隔天数
        <input
          v-model.number="intervalDays"
          type="number"
          min="1"
          max="30"
        >
      </label>
      <label>
        保留份数
        <input
          v-model.number="retentionCount"
          type="number"
          min="1"
          max="20"
        >
      </label>
      <button
        type="button"
        :disabled="automaticBusy || creating || creatingPortable || preparing || restoring"
        @click="emit('configureAutomatic', intervalDays, retentionCount)"
      >
        {{ automaticBusy ? '正在更新…' : automaticStatus?.enabled ? '更改自动备份目录/策略' : '选择目录并启用' }}
      </button>
      <button
        v-if="automaticStatus?.enabled"
        type="button"
        :disabled="automaticBusy || creating || creatingPortable || preparing || restoring"
        @click="emit('disableAutomatic')"
      >
        停用自动备份
      </button>
    </section>
    <article
      v-if="portableReceipt"
      class="portable-recovery"
      aria-labelledby="portable-recovery-title"
    >
      <strong id="portable-recovery-title">恢复密钥只显示这一次</strong>
      <p>请离线保存。密钥与备份包必须分开保管；丢失后无法找回，也无法在其他设备解密该备份。</p>
      <code>{{ portableReceipt.recoveryKey }}</code>
      <button
        type="button"
        @click="emit('dismissPortable')"
      >
        我已安全保存，隐藏密钥
      </button>
    </article>
    <form
      class="portable-restore"
      @submit.prevent="submitPortableRestore"
    >
      <label for="portable-recovery-key">跨设备恢复密钥</label>
      <input
        id="portable-recovery-key"
        v-model.trim="recoveryKey"
        type="password"
        autocomplete="off"
        spellcheck="false"
        placeholder="粘贴创建便携备份时显示的密钥"
      >
      <button
        type="submit"
        :disabled="!recoveryKey || automaticBusy || creating || creatingPortable || preparing || restoring"
      >
        {{ preparing ? '正在验证并转换…' : '选择便携备份并准备恢复' }}
      </button>
    </form>
    <p
      v-if="message"
      class="backup-message"
      role="alert"
    >
      {{ message }}
    </p>
    <div
      v-if="created || candidate"
      class="backup-results"
    >
      <article v-if="created">
        <strong><ShieldCheck :size="16" />加密备份已创建</strong>
        <span>{{ created.label }}</span>
        <small>{{ formatSettingsTime(created.createdAtUtcMs) }} · {{ created.assetCount }} 个资源 · {{ formatSettingsBytes(created.encryptedBytes) }}</small>
      </article>
      <article
        v-if="candidate"
        class="restore-ready-card"
      >
        <strong><FolderCheck :size="16" />安全恢复包已就绪</strong>
        <span>{{ candidate.summary.label }}</span>
        <small>{{ candidate.summary.assetCount }} 个资源 · {{ formatSettingsBytes(candidate.summary.encryptedBytes) }} · 已复制到隔离区并再次校验，当前资料库尚未改变</small>
        <button
          ref="restoreAction"
          type="button"
          :disabled="automaticBusy || creating || creatingPortable || preparing || restoring"
          @click="emit('openRestore')"
        >
          查看风险并确认恢复
        </button>
      </article>
    </div>
  </section>
</template>

<style scoped>
.backup-panel { margin-top: 16px; padding: 26px; border: 1px solid var(--line); border-radius: 17px; background: rgba(255,253,247,.78); box-shadow: 0 16px 48px rgba(34,48,43,.05); }
header { display: flex; justify-content: space-between; gap: 24px; align-items: flex-start; margin-bottom: 16px; }
header p { margin: 0 0 8px; color: var(--cinnabar); font-size: 12px; font-weight: 800; letter-spacing: .14em; text-transform: uppercase; }
header span { display: block; max-width: 690px; margin-top: 9px; color: var(--ink-muted); }
h2 { margin: 0; color: var(--green-deep); font-family: Georgia,'Microsoft YaHei',serif; font-size: 21px; }
button { display: inline-flex; min-height: 44px; gap: 7px; align-items: center; padding: 10px 14px; border: 1px solid var(--line); border-radius: 10px; background: var(--paper-raised); cursor: pointer; }
button:disabled { opacity: .5; }
.backup-actions { display: flex; flex: 0 0 auto; gap: 8px; }
.backup-boundary { margin: 0; padding: 13px 15px; color: var(--ink-muted); border-left: 3px solid var(--cinnabar); border-radius: 7px; background: rgba(232,221,199,.28); font-size: 12px; line-height: 1.7; }
.backup-message { margin: 12px 0 0; padding: 11px 13px; color: #843d2c; border: 1px solid rgba(185,88,63,.25); border-radius: 11px; background: rgba(185,88,63,.08); font-size: 12px; line-height: 1.6; }
.portable-recovery { display: grid; gap: 9px; margin-top: 14px; padding: 15px; border: 1px solid rgba(185,88,63,.3); border-radius: 12px; background: rgba(185,88,63,.08); }
.portable-recovery p { margin: 0; color: var(--ink-muted); font-size: 12px; line-height: 1.6; }
.portable-recovery code { padding: 10px; border-radius: 8px; background: var(--paper-raised); overflow-wrap: anywhere; user-select: all; }
.portable-recovery button { width: fit-content; }
.automatic-backup { display: flex; flex-wrap: wrap; gap: 10px; align-items: end; margin-top: 14px; padding: 15px; border: 1px solid var(--line); border-radius: 12px; background: rgba(33,51,45,.035); }
.automatic-backup > div { flex: 1 1 260px; }
.automatic-backup strong { color: var(--green-deep); }
.automatic-backup p { margin: 5px 0 0; color: var(--ink-muted); font-size: 12px; line-height: 1.5; }
.automatic-backup label { display: grid; gap: 5px; color: var(--ink-muted); font-size: 12px; }
.automatic-backup input { width: 82px; min-height: 40px; padding: 7px 9px; border: 1px solid var(--line); border-radius: 8px; background: var(--paper-raised); }
.portable-restore { display: grid; grid-template-columns: minmax(180px,1fr) auto; gap: 8px; align-items: end; margin-top: 14px; }
.portable-restore label { grid-column: 1 / -1; color: var(--green-deep); font-size: 12px; font-weight: 700; }
.portable-restore input { min-height: 44px; padding: 9px 11px; border: 1px solid var(--line); border-radius: 9px; background: var(--paper-raised); }
.backup-results { display: grid; grid-template-columns: repeat(2,minmax(0,1fr)); gap: 10px; margin-top: 14px; }
.backup-results article { display: grid; gap: 6px; padding: 15px; border-radius: 12px; background: rgba(33,51,45,.055); }
.backup-results strong { display: flex; gap: 6px; align-items: center; color: #557263; font-size: 13px; }
.backup-results span { color: var(--green-deep); font-size: 12px; overflow-wrap: anywhere; }
.backup-results small { color: var(--ink-muted); line-height: 1.6; }
.restore-ready-card button { width: fit-content; margin-top: 5px; color: #fffdf7; border-color: var(--cinnabar); background: var(--cinnabar); transition: transform var(--motion-feedback), box-shadow var(--motion-feedback); }
.restore-ready-card button:hover { transform: translateY(-1px); box-shadow: 0 8px 20px rgba(185,88,63,.18); }
@media (max-width: 760px) {
  button { min-height: 44px; }
  header { flex-direction: column; }
  .backup-actions { width: 100%; flex-direction: column; }
  .backup-actions button { justify-content: center; }
  .backup-results { grid-template-columns: 1fr; }
  .portable-restore { grid-template-columns: 1fr; }
}
@media (prefers-reduced-motion: reduce) {
  .restore-ready-card button { transition: none; }
  .restore-ready-card button:hover { transform: none; }
}
</style>
