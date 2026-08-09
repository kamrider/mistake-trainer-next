<script setup lang="ts">
import { CloudOff } from '@lucide/vue'
import type { AppResult } from '../../shared/api/app-result'
import type { CloudBackendKind, CloudBackendStatus } from '../../shared/api/bindings'
import { backendStatusLabel } from '../../shared/api/sync-backend'

export interface SettingsBackendOption {
  kind: CloudBackendKind
  title: string
  hint: string
  available: boolean
  badge: string | undefined
}

defineProps<{
  status: AppResult<CloudBackendStatus> | undefined
  options: readonly SettingsBackendOption[]
  busy: boolean
  message: string
}>()

const emit = defineEmits<{
  select: [kind: CloudBackendKind]
}>()
</script>

<template>
  <section
    id="settings-sync"
    class="backend-panel"
    aria-labelledby="backend-settings-title"
    :aria-busy="busy"
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
      v-if="status?.ok"
      class="backend-current"
      role="status"
      aria-live="polite"
    >
      <span
        class="backend-status-dot"
        :class="{ ready: status.data.ready }"
        aria-hidden="true"
      />
      <span>
        <strong>{{ backendStatusLabel(status.data) }}</strong>
        <small>{{ status.data.syncEnabled ? '云端同步已启用' : '云端同步未启用，待同步变更会安全保留' }}</small>
      </span>
    </div>

    <p
      v-else-if="status && !status.ok"
      class="backend-message warning"
      role="alert"
    >
      {{ status.error.userMessage }}
    </p>

    <div class="backend-options">
      <button
        v-for="option in options"
        :key="option.kind"
        type="button"
        :class="[
          'backend-option',
          {
            selected: option.available && status?.ok && status.data.kind === option.kind,
            unavailable: !option.available,
          },
        ]"
        :disabled="busy || !option.available"
        @click="emit('select', option.kind)"
      >
        <span
          class="backend-option-mark"
          aria-hidden="true"
        />
        <span>
          <strong>
            {{ option.title }}
            <em
              v-if="option.badge"
              class="capability-badge"
            >{{ option.badge }}</em>
          </strong>
          <small>{{ option.hint }}</small>
        </span>
      </button>
    </div>

    <p
      v-if="message"
      class="backend-message"
      role="status"
    >
      {{ message }}
    </p>
  </section>
</template>

<style scoped>
.backend-panel {
  margin-bottom: 16px;
  padding: 24px 26px;
  border: 1px solid rgba(33, 51, 45, .22);
  border-radius: 17px;
  background: linear-gradient(135deg, rgba(255, 253, 247, .94), rgba(220, 228, 220, .34));
  box-shadow: 0 16px 48px rgba(34, 48, 43, .05);
}

.backend-panel > header {
  display: flex;
  gap: 24px;
  align-items: center;
  justify-content: space-between;
  margin-bottom: 16px;
}

.backend-panel > header > svg {
  flex: 0 0 auto;
  color: var(--green-deep);
}

.backend-panel > header p {
  margin: 0 0 8px;
  color: var(--cinnabar);
  font-size: 12px;
  font-weight: 800;
  letter-spacing: .14em;
}

.backend-panel h2 {
  margin: 0;
  color: var(--green-deep);
  font-family: Georgia, 'Microsoft YaHei', serif;
  font-size: 21px;
}

.backend-panel > header span {
  display: block;
  margin-top: 8px;
  color: var(--ink-muted);
  font-size: 12px;
  line-height: 1.7;
}

button {
  display: inline-flex;
  min-height: 44px;
  gap: 7px;
  align-items: center;
  padding: 10px 14px;
  border: 1px solid var(--line);
  border-radius: 10px;
  background: var(--paper-raised);
  cursor: pointer;
}

button:disabled {
  opacity: .5;
}

button:focus-visible {
  outline: 3px solid rgba(185, 88, 63, .24);
  outline-offset: 2px;
}

.backend-current {
  display: flex;
  gap: 9px;
  align-items: center;
  margin-bottom: 13px;
  padding: 11px 13px;
  border-radius: 11px;
  background: rgba(33, 51, 45, .07);
}

.backend-current > span:last-child {
  display: grid;
  gap: 3px;
}

.backend-current strong {
  color: var(--green-deep);
  font-size: 13px;
}

.backend-current small {
  color: var(--ink-muted);
  font-size: 12px;
}

.backend-status-dot {
  width: 9px;
  height: 9px;
  border-radius: 50%;
  background: var(--cinnabar);
  box-shadow: 0 0 0 4px rgba(185, 88, 63, .13);
}

.backend-status-dot.ready {
  background: #557263;
  box-shadow: 0 0 0 4px rgba(85, 114, 99, .14);
}

.backend-options {
  display: grid;
  grid-template-columns: repeat(3, minmax(0, 1fr));
  gap: 10px;
}

.backend-option {
  display: grid;
  grid-template-columns: 17px 1fr;
  gap: 10px;
  align-items: start;
  min-height: 92px;
  padding: 14px;
  border: 1px solid var(--line);
  border-radius: 12px;
  background: rgba(255, 253, 247, .65);
  cursor: pointer;
  text-align: left;
  transition:
    transform var(--motion-feedback) var(--ease-standard),
    border-color var(--motion-standard) var(--ease-standard),
    background var(--motion-standard) var(--ease-standard),
    box-shadow var(--motion-standard) var(--ease-standard);
}

.backend-option:hover:not(:disabled) {
  transform: translateY(-2px);
  border-color: rgba(33, 51, 45, .38);
  box-shadow: 0 10px 24px rgba(34, 48, 43, .08);
}

.backend-option.selected {
  border-color: var(--green-deep);
  background: var(--green-soft);
  box-shadow: inset 0 0 0 1px var(--green-deep);
}

.backend-option:disabled {
  cursor: wait;
}

.backend-option.unavailable {
  opacity: .62;
  cursor: not-allowed;
}

.backend-option-mark {
  width: 15px;
  height: 15px;
  margin-top: 2px;
  border: 1px solid var(--sand-deep);
  border-radius: 50%;
  background: var(--paper-raised);
  box-shadow: inset 0 0 0 4px var(--paper-raised);
}

.backend-option.unavailable .backend-option-mark {
  border-style: dashed;
  background: transparent;
  box-shadow: none;
}

.backend-option.selected .backend-option-mark {
  border-color: var(--green-deep);
  background: var(--green-deep);
}

.backend-option > span:last-child {
  display: grid;
  gap: 5px;
}

.backend-option strong {
  color: var(--green-deep);
  font-size: 13px;
}

.backend-option small {
  color: var(--ink-muted);
  font-size: 12px;
  line-height: 1.55;
}

.capability-badge {
  display: inline-flex;
  margin-left: 5px;
  padding: 2px 6px;
  color: #7d5d45;
  border-radius: 999px;
  background: rgba(232, 221, 199, .7);
  font-size: 12px;
  font-style: normal;
  vertical-align: middle;
}

.backend-message {
  margin: 12px 0 0;
  color: #557263;
  font-size: 12px;
}

.backend-message.warning {
  color: #843d2c;
  font-weight: 700;
}

@media (max-width: 760px) {
  .backend-panel {
    padding: 22px 18px;
  }

  .backend-panel > header {
    gap: 16px;
  }

  .backend-panel small,
  .backend-message {
    font-size: 12px;
  }

  .backend-panel button {
    min-height: 44px;
  }

  .backend-options {
    grid-template-columns: 1fr;
  }
}

@media (prefers-reduced-motion: reduce) {
  .backend-option {
    transition: none;
  }

  .backend-option:hover:not(:disabled) {
    transform: none;
  }
}
</style>
