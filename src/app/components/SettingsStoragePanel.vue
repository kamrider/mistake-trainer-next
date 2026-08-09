<script setup lang="ts">
import { CheckCircle2, Database, FolderCheck, TriangleAlert } from '@lucide/vue'
import { computed, ref } from 'vue'
import type { StorageLocationStatus } from '../../shared/api/bindings'
import { formatSettingsBytes } from '../settings-formatters'

export interface SettingsStorageReceiptCopy {
  kind: 'success' | 'warning'
  title: string
  detail: string
}

const props = defineProps<{
  status: StorageLocationStatus | undefined
  statusMessage: string
  receipt: SettingsStorageReceiptCopy | undefined
  migrating: boolean
}>()

const emit = defineEmits<{
  migrate: []
}>()

const migrationAction = ref<HTMLButtonElement>()
const totalBytes = computed(() => {
  const status = props.status
  if (!status || status.databaseBytes === null || status.assetBytes === null) return null
  const total = status.databaseBytes + status.assetBytes
  return Number.isFinite(total) && total >= 0 ? total : null
})

function focusMigrationAction() {
  migrationAction.value?.focus()
}

defineExpose({ focusMigrationAction })
</script>

<template>
  <section
    id="settings-storage"
    class="storage-panel"
    aria-labelledby="storage-settings-title"
  >
    <header>
      <div>
        <p>本机加密资料 · 可迁移</p>
        <h2 id="storage-settings-title">
          资料库存储位置
        </h2>
        <span>显示经过边界处理的位置名称和真实密文容量；页面不会读取、保存或传递任意文件路径。</span>
      </div>
      <Database :size="24" />
    </header>

    <aside
      v-if="receipt"
      :class="['storage-receipt', receipt.kind]"
      :role="receipt.kind === 'success' ? 'status' : 'alert'"
      aria-label="存储迁移结果"
      :aria-live="receipt.kind === 'success' ? 'polite' : 'assertive'"
    >
      <component
        :is="receipt.kind === 'success' ? CheckCircle2 : TriangleAlert"
        :size="19"
      />
      <span><strong>{{ receipt.title }}</strong><small>{{ receipt.detail }}</small></span>
    </aside>

    <div
      v-if="status"
      class="storage-summary"
    >
      <article class="storage-location">
        <span class="storage-location-icon"><FolderCheck :size="21" /></span>
        <span>
          <small>{{ status.kind === 'custom' ? '当前使用自定义位置' : '当前使用默认位置' }}</small>
          <strong>{{ status.locationLabel }}</strong>
        </span>
        <span
          v-if="status.migrationPending"
          class="storage-pending"
        >等待重启校验</span>
      </article>
      <dl class="storage-usage">
        <div><dt>加密数据库</dt><dd>{{ formatSettingsBytes(status.databaseBytes) }}</dd></div>
        <div><dt>加密图片</dt><dd>{{ formatSettingsBytes(status.assetBytes) }}</dd></div>
        <div><dt>合计占用</dt><dd>{{ formatSettingsBytes(totalBytes) }}</dd></div>
      </dl>
      <footer class="storage-actions">
        <span>选择新的本机磁盘或文件夹；校验失败会自动保留原位置。</span>
        <button
          ref="migrationAction"
          type="button"
          :disabled="migrating || status.migrationPending"
          @click="emit('migrate')"
        >
          <FolderCheck :size="16" />
          {{ status.migrationPending ? '等待安全重启' : '迁移资料库' }}
        </button>
      </footer>
    </div>
    <p
      v-else
      class="storage-unavailable"
      role="status"
    >
      {{ statusMessage || '正在读取资料库容量…' }}
    </p>
  </section>
</template>

<style scoped>
.storage-panel { margin-top: 16px; padding: 26px; border: 1px solid var(--line); border-radius: 17px; background: rgba(255,253,247,.78); box-shadow: 0 16px 48px rgba(34,48,43,.05); }
header { display: flex; justify-content: space-between; gap: 24px; align-items: center; margin-bottom: 18px; }
header p { margin: 0 0 8px; color: var(--cinnabar); font-size: 12px; font-weight: 800; letter-spacing: .14em; text-transform: uppercase; }
header span { display: block; max-width: 720px; margin-top: 9px; color: var(--ink-muted); }
header > svg { color: var(--green-deep); }
h2 { margin: 0; color: var(--green-deep); font-family: Georgia,'Microsoft YaHei',serif; font-size: 21px; }
button { display: inline-flex; min-height: 44px; gap: 7px; align-items: center; padding: 10px 14px; border: 1px solid var(--line); border-radius: 10px; background: var(--paper-raised); cursor: pointer; }
button:disabled { opacity: .5; }
.storage-receipt { display: grid; grid-template-columns: 28px 1fr; gap: 10px; align-items: start; margin-bottom: 14px; padding: 13px 15px; color: #557263; border: 1px solid rgba(85,114,99,.18); border-radius: 12px; background: rgba(85,114,99,.08); }
.storage-receipt.warning { color: #843d2c; border-color: rgba(185,88,63,.2); background: rgba(185,88,63,.07); }
.storage-receipt span { display: grid; gap: 4px; }
.storage-receipt small { color: var(--ink-muted); line-height: 1.6; }
.storage-summary { display: grid; gap: 13px; }
.storage-location { display: grid; grid-template-columns: 46px 1fr auto; gap: 13px; align-items: center; padding: 15px; border: 1px solid rgba(33,51,45,.12); border-radius: 13px; background: linear-gradient(135deg,rgba(220,228,220,.4),rgba(255,253,247,.7)); }
.storage-location-icon { display: grid; width: 42px; height: 42px; place-items: center; color: var(--green-deep); border-radius: 12px; background: rgba(33,51,45,.1); }
.storage-location > span:nth-child(2) { display: grid; gap: 4px; min-width: 0; }
.storage-location small { color: var(--ink-muted); font-size: 10px; }
.storage-location strong { color: var(--green-deep); overflow-wrap: anywhere; }
.storage-pending { padding: 6px 9px; color: #8d4635; border-radius: 999px; background: rgba(185,88,63,.1); font-size: 10px; font-weight: 800; }
.storage-usage { display: grid; grid-template-columns: repeat(3,minmax(0,1fr)); gap: 10px; margin: 0; }
.storage-usage div { padding: 14px 15px; border-radius: 12px; background: rgba(232,221,199,.3); }
.storage-usage dt { color: var(--ink-muted); font-size: 10px; }
.storage-usage dd { margin: 7px 0 0; color: var(--green-deep); font-family: Georgia,serif; font-size: 22px; font-weight: 700; }
.storage-actions { display: flex; gap: 16px; align-items: center; justify-content: space-between; }
.storage-actions > span { color: var(--ink-muted); font-size: 11px; line-height: 1.6; }
.storage-actions button { flex: 0 0 auto; color: var(--paper); border-color: var(--green-deep); background: var(--green-deep); transition: transform var(--motion-feedback) var(--ease-standard), box-shadow var(--motion-feedback) var(--ease-standard); }
.storage-actions button:hover:not(:disabled) { transform: translateY(-1px); box-shadow: 0 8px 20px rgba(33,51,45,.14); }
.storage-unavailable { margin: 0; padding: 18px; color: var(--ink-muted); border: 1px dashed var(--line); border-radius: 12px; background: rgba(232,221,199,.2); text-align: center; }
@media (max-width: 760px) {
  button { min-height: 44px; }
  .storage-usage { grid-template-columns: 1fr; }
  .storage-actions { align-items: stretch; flex-direction: column; }
  .storage-actions button { justify-content: center; }
  .storage-location { grid-template-columns: 46px 1fr; }
  .storage-pending { grid-column: 1 / -1; width: fit-content; }
}
@media (prefers-reduced-motion: reduce) {
  .storage-actions button { transition: none; }
  .storage-actions button:hover:not(:disabled) { transform: none; }
}
</style>
