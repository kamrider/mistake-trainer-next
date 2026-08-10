<script setup lang="ts">
import { ArchiveRestore, Laptop, RotateCcw, ShieldCheck } from '@lucide/vue'
import { ref } from 'vue'
import type { WindowsUpdateCheckReport, WindowsUpdateStatus } from '../../shared/api/bindings'

defineProps<{
  status: WindowsUpdateStatus | undefined
  report: WindowsUpdateCheckReport | undefined
  checking: boolean
  installing: boolean
  message: string
  publicationLabel: string
}>()

const emit = defineEmits<{
  check: []
  install: []
}>()

const primaryAction = ref<HTMLButtonElement>()

function focusPrimaryAction() {
  primaryAction.value?.focus()
}

defineExpose({ focusPrimaryAction })
</script>

<template>
  <section
    id="settings-updates"
    class="update-panel"
    aria-labelledby="update-settings-title"
  >
    <header>
      <div>
        <p>版本维护 · 用户确认</p>
        <h2 id="update-settings-title">
          应用更新
        </h2>
        <span>只有签名验证通过的 Windows 安装包才会进入安装流程；应用不会静默自动更新。</span>
      </div>
      <ShieldCheck :size="24" />
    </header>

    <div
      v-if="status"
      class="update-contract"
    >
      <span class="update-contract-icon"><Laptop :size="21" /></span>
      <div>
        <strong>
          {{ status.enabled ? '已接入签名更新通道' : '当前安装包未接入自动更新' }}
        </strong>
        <p>当前版本 {{ status.currentVersion }}</p>
        <small v-if="status.enabled">
          启动后每天最多静默检查一次，也可随时手动检查；发现新版本后仍需再次确认安装。
        </small>
        <small v-else>
          本地开发版或未配置发行版不会访问更新服务器，请通过新的正式安装包升级。
        </small>
      </div>
      <div
        v-if="status.enabled"
        class="update-actions"
      >
        <button
          ref="primaryAction"
          type="button"
          :disabled="checking || installing"
          @click="emit('check')"
        >
          <RotateCcw :size="16" />
          {{ checking ? '正在检查…' : '检查更新' }}
        </button>
        <button
          v-if="report?.available && report.version"
          type="button"
          class="primary"
          :disabled="checking || installing"
          @click="emit('install')"
        >
          <ArchiveRestore :size="16" />
          {{ installing ? '正在下载并验证…' : `安装 ${report.version}` }}
        </button>
      </div>
    </div>
    <p
      v-if="message"
      class="update-message"
      role="status"
      aria-label="应用更新状态"
      aria-live="polite"
    >
      {{ message }}
      <small v-if="report?.publishedAt && publicationLabel">
        发布时间 {{ publicationLabel }}
      </small>
    </p>
  </section>
</template>

<style scoped>
.update-panel { margin-top: 16px; padding: 26px; border: 1px solid var(--line); border-radius: 17px; background: rgba(255,253,247,.78); box-shadow: 0 16px 48px rgba(34,48,43,.05); }
header { display: flex; justify-content: space-between; gap: 24px; align-items: center; margin-bottom: 16px; }
header p { margin: 0 0 8px; color: var(--cinnabar); font-size: 12px; font-weight: 800; letter-spacing: .14em; text-transform: uppercase; }
header span { display: block; max-width: 690px; margin-top: 9px; color: var(--ink-muted); font-size: 12px; }
header > svg { flex: 0 0 auto; color: var(--green-deep); }
h2 { margin: 0; color: var(--green-deep); font-family: Georgia,'Microsoft YaHei',serif; font-size: 21px; }
button { display: inline-flex; min-height: 44px; gap: 7px; align-items: center; padding: 10px 14px; border: 1px solid var(--line); border-radius: 10px; background: var(--paper-raised); cursor: pointer; }
button:disabled { opacity: .5; }
.update-contract { display: grid; grid-template-columns: 44px minmax(0,1fr) auto; gap: 14px; align-items: center; padding: 17px; border: 1px solid rgba(33,51,45,.15); border-radius: 14px; background: linear-gradient(135deg,rgba(220,228,220,.4),rgba(255,253,247,.62)); }
.update-contract-icon { display: grid; width: 42px; height: 42px; place-items: center; color: var(--green-deep); border-radius: 13px 4px 13px 13px; background: rgba(33,51,45,.1); }
.update-contract strong { display: block; color: var(--green-deep); font-size: 13px; }
.update-contract p { margin: 5px 0 4px; color: #557263; font-size: 12px; font-weight: 750; }
.update-contract small { color: var(--ink-muted); font-size: 10px; line-height: 1.55; }
.update-actions { display: flex; gap: 9px; flex-wrap: wrap; justify-content: flex-end; }
.update-actions button { justify-content: center; min-width: 126px; }
.update-actions .primary { color: var(--paper); border-color: var(--green-deep); background: var(--green-deep); }
.update-message { display: flex; gap: 8px; align-items: center; flex-wrap: wrap; margin: 12px 0 0; padding: 11px 13px; color: #557263; border: 1px solid rgba(85,114,99,.23); border-radius: 11px; background: rgba(85,114,99,.08); font-size: 11px; }
.update-message small { color: var(--ink-muted); }
@media (max-width: 760px) {
  .update-message { font-size: 12px; }
  button { min-height: 44px; }
  .update-contract { grid-template-columns: 42px 1fr; }
  .update-actions { grid-column: 1 / -1; flex-direction: column; }
  .update-actions button { width: 100%; }
}
</style>
