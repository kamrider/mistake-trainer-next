<script setup lang="ts">
import { CheckCircle2, FileJson2, ShieldCheck, TriangleAlert } from '@lucide/vue'
import { ref } from 'vue'
import type { DiagnosticExportReceipt } from '../../shared/api/bindings'

defineProps<{
  receipt: DiagnosticExportReceipt | undefined
  exporting: boolean
  message: string
  nativeAvailable: boolean
  generatedAtLabel: string
}>()

const emit = defineEmits<{
  export: []
}>()

const primaryAction = ref<HTMLButtonElement>()

function focusPrimaryAction() {
  primaryAction.value?.focus()
}

defineExpose({ focusPrimaryAction })
</script>

<template>
  <section
    id="settings-diagnostics"
    class="diagnostic-panel"
    aria-labelledby="diagnostic-settings-title"
  >
    <header>
      <div>
        <p>问题排查 · 隐私优先</p>
        <h2 id="diagnostic-settings-title">
          安全诊断报告
        </h2>
        <span>当启动、存储、同步或采集出现问题时，生成一份可交给支持人员的固定格式报告。</span>
      </div>
      <FileJson2 :size="24" />
    </header>

    <div class="diagnostic-contract">
      <span class="diagnostic-contract-icon"><ShieldCheck :size="21" /></span>
      <div>
        <strong>先看清内容，再决定是否发送</strong>
        <p>不会包含题图、答案、笔记、账户信息或本机路径</p>
        <ul>
          <li>仅包含应用版本、Windows 架构和数据库结构版本</li>
          <li>只统计题目、资源、采集批次、训练、导出和同步数量</li>
          <li>完整性检查只写“通过 / 未通过”，不写数据库原始错误</li>
        </ul>
      </div>
      <button
        ref="primaryAction"
        type="button"
        :disabled="exporting || !nativeAvailable"
        @click="emit('export')"
      >
        <FileJson2 :size="16" />
        {{ exporting ? '正在检查并生成…' : '生成安全诊断报告' }}
      </button>
    </div>

    <Transition name="diagnostic-receipt">
      <article
        v-if="receipt"
        class="diagnostic-receipt"
        role="status"
        aria-label="诊断报告已生成"
        aria-live="polite"
      >
        <span class="diagnostic-receipt-seal"><CheckCircle2 :size="20" /></span>
        <div>
          <strong>诊断报告已安全生成</strong>
          <span>{{ receipt.fileLabel }}</span>
          <small>
            {{ generatedAtLabel }}
            · 报告编号 {{ receipt.reportId }}
            · {{ receipt.warningCount === 0 ? '所有检查通过' : `${receipt.warningCount} 项需留意` }}
          </small>
        </div>
      </article>
    </Transition>
    <p
      v-if="message"
      class="diagnostic-error"
      role="alert"
      aria-label="诊断报告未生成"
    >
      <TriangleAlert :size="16" />
      {{ message }}
    </p>
  </section>
</template>

<style scoped>
.diagnostic-panel { margin-top: 16px; padding: 26px; overflow: hidden; border: 1px solid var(--line); border-radius: 17px; background: rgba(255,253,247,.78); box-shadow: 0 16px 48px rgba(34,48,43,.05); }
header { display: flex; justify-content: space-between; gap: 24px; align-items: center; margin-bottom: 16px; }
header p { margin: 0 0 8px; color: var(--cinnabar); font-size: 12px; font-weight: 800; letter-spacing: .14em; text-transform: uppercase; }
header span { display: block; max-width: 690px; margin-top: 9px; color: var(--ink-muted); }
header > svg { flex: 0 0 auto; color: var(--green-deep); }
h2 { margin: 0; color: var(--green-deep); font-family: Georgia,'Microsoft YaHei',serif; font-size: 21px; }
button { display: inline-flex; min-height: 44px; gap: 7px; align-items: center; padding: 10px 14px; border: 1px solid var(--line); border-radius: 10px; background: var(--paper-raised); cursor: pointer; }
button:disabled { opacity: .5; }
.diagnostic-contract { display: grid; grid-template-columns: 44px minmax(0,1fr) auto; gap: 14px; align-items: start; padding: 17px; border: 1px solid rgba(33,51,45,.15); border-radius: 14px; background: linear-gradient(135deg,rgba(220,228,220,.4),rgba(255,253,247,.62)); }
.diagnostic-contract-icon { display: grid; width: 42px; height: 42px; place-items: center; color: var(--green-deep); border-radius: 13px 4px 13px 13px; background: rgba(33,51,45,.1); }
.diagnostic-contract strong { display: block; color: var(--green-deep); font-size: 13px; }
.diagnostic-contract p { margin: 5px 0 8px; color: #557263; font-size: 12px; font-weight: 750; }
.diagnostic-contract ul { display: grid; gap: 4px; margin: 0; padding-left: 17px; color: var(--ink-muted); font-size: 12px; line-height: 1.55; }
.diagnostic-contract button { align-self: center; justify-content: center; min-width: 176px; color: var(--paper); border-color: var(--green-deep); background: var(--green-deep); transition: transform var(--motion-feedback) var(--ease-standard), opacity var(--motion-feedback) var(--ease-standard); }
.diagnostic-contract button:hover:not(:disabled) { transform: translateY(-1px); }
.diagnostic-receipt { display: grid; grid-template-columns: 42px minmax(0,1fr); gap: 13px; align-items: center; margin-top: 13px; padding: 15px 17px; border: 1px solid rgba(85,114,99,.23); border-radius: 13px; background: rgba(85,114,99,.08); }
.diagnostic-receipt-seal { display: grid; width: 38px; height: 38px; place-items: center; color: var(--paper); border-radius: 12px 4px 12px 12px; background: #557263; }
.diagnostic-receipt div { display: grid; min-width: 0; gap: 4px; }
.diagnostic-receipt strong { color: var(--green-deep); font-size: 13px; }
.diagnostic-receipt div > span { color: #557263; font-size: 12px; font-weight: 750; overflow-wrap: anywhere; }
.diagnostic-receipt small { color: var(--ink-muted); font-size: 12px; line-height: 1.55; overflow-wrap: anywhere; }
.diagnostic-error { display: flex; gap: 8px; align-items: center; margin: 12px 0 0; padding: 11px 13px; color: #843d2c; border: 1px solid rgba(185,88,63,.23); border-radius: 11px; background: rgba(185,88,63,.08); font-size: 12px; }
.diagnostic-error svg { flex: 0 0 auto; }
.diagnostic-receipt-enter-active,.diagnostic-receipt-leave-active { transition: opacity var(--motion-standard) var(--ease-standard),transform var(--motion-standard) var(--ease-standard); }
.diagnostic-receipt-enter-from,.diagnostic-receipt-leave-to { opacity: 0; transform: translateY(8px); }
@media (max-width: 760px) {
  .diagnostic-error { font-size: 12px; }
  button { min-height: 44px; }
  .diagnostic-contract { grid-template-columns: 42px 1fr; }
  .diagnostic-contract button { grid-column: 1 / -1; width: 100%; }
}
@media (prefers-reduced-motion: reduce) {
  .diagnostic-contract button,.diagnostic-receipt-enter-active,.diagnostic-receipt-leave-active { transition: none; }
  .diagnostic-contract button:hover:not(:disabled),.diagnostic-receipt-enter-from,.diagnostic-receipt-leave-to { transform: none; }
}
</style>
