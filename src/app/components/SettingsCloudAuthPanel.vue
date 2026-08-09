<script setup lang="ts">
import { CloudOff } from '@lucide/vue'
import { ref } from 'vue'
import type { CloudAuthState } from '../../shared/api/bindings'

defineProps<{
  auth: CloudAuthState
  email: string
  password: string
  mode: 'signIn' | 'signUp'
  authBusy: boolean
  authMessage: string
  syncBusy: boolean
  syncMessage: string
  statusLabel: string
}>()

const emit = defineEmits<{
  updateEmail: [value: string]
  updatePassword: [value: string]
  updateMode: [mode: 'signIn' | 'signUp']
  submit: []
  sync: []
  signOut: []
}>()

const signOutAction = ref<HTMLButtonElement>()

function inputValue(event: Event) {
  return (event.target as HTMLInputElement).value
}

function focusSignOutAction() {
  signOutAction.value?.focus()
}

defineExpose({ focusSignOutAction })
</script>

<template>
  <section
    class="cloud-auth-panel"
    aria-labelledby="cloud-auth-title"
    :aria-busy="authBusy || syncBusy"
  >
    <header>
      <div>
        <p>账户与跨设备</p>
        <h2 id="cloud-auth-title">
          云端同步账户
        </h2>
        <span>登录只用于同步；题库仍先写入本机加密库。刷新、退出或网络中断都不会清空本地资料。</span>
      </div>
      <CloudOff :size="24" />
    </header>

    <aside
      class="regional-sync-note"
      role="note"
    >
      <strong>国内网络提示</strong>
      <span>Supabase 在中国大陆可能出现连接超时或验证邮件延迟。遇到这种情况请保持“仅本地”继续使用；待同步变更会保留，之后网络恢复再手动同步。</span>
    </aside>

    <div
      class="cloud-auth-status"
      role="status"
      aria-live="polite"
    >
      <span
        class="backend-status-dot"
        :class="{
          ready: auth.status.kind === 'connected',
          offline: auth.status.kind === 'offline',
        }"
        aria-hidden="true"
      />
      <strong>{{ statusLabel }}</strong>
      <small v-if="auth.status.emailHint">{{ auth.status.emailHint }}</small>
    </div>

    <form
      v-if="auth.configured && auth.status.kind !== 'connected'"
      class="cloud-auth-form"
      @submit.prevent="emit('submit')"
    >
      <label>
        邮箱
        <input
          :value="email"
          type="email"
          autocomplete="username"
          placeholder="you@example.com"
          required
          @input="emit('updateEmail', inputValue($event))"
        >
      </label>
      <label>
        密码
        <input
          :value="password"
          type="password"
          autocomplete="current-password"
          minlength="8"
          required
          @input="emit('updatePassword', inputValue($event))"
        >
      </label>
      <button
        type="submit"
        class="primary-action"
        :disabled="authBusy"
      >
        {{ authBusy ? '连接中…' : mode === 'signUp' ? '注册并连接' : '登录并连接' }}
      </button>
    </form>

    <button
      v-if="auth.configured && auth.status.kind !== 'connected'"
      type="button"
      class="auth-mode-toggle"
      @click="emit('updateMode', mode === 'signIn' ? 'signUp' : 'signIn')"
    >
      {{ mode === 'signIn' ? '还没有账户？注册' : '已有账户？返回登录' }}
    </button>

    <div
      v-else-if="auth.status.kind === 'connected'"
      class="cloud-auth-actions"
    >
      <button
        type="button"
        class="primary-action"
        :disabled="syncBusy"
        @click="emit('sync')"
      >
        {{ syncBusy ? '同步中…' : '立即同步' }}
      </button>
      <button
        ref="signOutAction"
        type="button"
        :disabled="authBusy"
        @click="emit('signOut')"
      >
        退出云端并锁定
      </button>
    </div>

    <p
      v-else
      class="backend-message warning"
    >
      当前构建没有云端地址，仍可正常使用全部本地功能。
    </p>

    <p
      v-if="authMessage"
      class="backend-message"
      role="status"
    >
      {{ authMessage }}
    </p>
    <p
      v-if="syncMessage"
      class="backend-message"
      role="status"
    >
      {{ syncMessage }}
    </p>
  </section>
</template>

<style scoped>
.cloud-auth-panel {
  margin-bottom: 16px;
  padding: 24px 26px;
  border: 1px solid rgba(185, 88, 63, .22);
  border-radius: 17px;
  background: linear-gradient(135deg, rgba(255, 253, 247, .96), rgba(247, 235, 220, .42));
  box-shadow: 0 16px 48px rgba(34, 48, 43, .05);
}

.cloud-auth-panel > header {
  display: flex;
  gap: 24px;
  align-items: center;
  justify-content: space-between;
  margin-bottom: 16px;
}

.cloud-auth-panel > header > svg {
  flex: 0 0 auto;
  color: var(--green-deep);
}

.cloud-auth-panel > header p {
  margin: 0 0 8px;
  color: var(--cinnabar);
  font-size: 12px;
  font-weight: 800;
  letter-spacing: .14em;
}

.cloud-auth-panel h2 {
  margin: 0;
  color: var(--green-deep);
  font-family: Georgia, 'Microsoft YaHei', serif;
  font-size: 21px;
}

.cloud-auth-panel > header span {
  display: block;
  max-width: 680px;
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

button:focus-visible,
input:focus-visible {
  outline: 3px solid rgba(185, 88, 63, .24);
  outline-offset: 2px;
}

.regional-sync-note {
  display: grid;
  gap: 4px;
  margin: 0 0 14px;
  padding: 11px 13px;
  color: #74594d;
  border: 1px solid rgba(185, 88, 63, .18);
  border-radius: 11px;
  background: rgba(247, 225, 216, .48);
  font-size: 12px;
  line-height: 1.55;
}

.regional-sync-note strong {
  color: #8d4635;
  font-size: 12px;
}

.cloud-auth-status {
  display: flex;
  gap: 9px;
  align-items: center;
  margin-bottom: 13px;
  padding: 11px 13px;
  border-radius: 11px;
  background: rgba(33, 51, 45, .07);
}

.cloud-auth-status small {
  color: var(--ink-muted);
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

.backend-status-dot.offline {
  background: #b07a42;
  box-shadow: 0 0 0 4px rgba(176, 122, 66, .14);
}

.cloud-auth-form {
  display: grid;
  grid-template-columns: 1fr 1fr auto;
  gap: 10px;
  align-items: end;
}

.cloud-auth-form label {
  display: grid;
  gap: 5px;
  color: var(--ink-muted);
  font-size: 12px;
}

.cloud-auth-form input {
  min-width: 0;
  min-height: 44px;
  padding: 10px 12px;
  border: 1px solid var(--line);
  border-radius: 10px;
  background: var(--paper);
}

.primary-action {
  color: var(--paper);
  border-color: var(--green-deep);
  background: var(--green-deep);
}

.cloud-auth-actions {
  display: flex;
  gap: 9px;
}

.auth-mode-toggle {
  min-height: 44px;
  margin-top: 11px;
  padding: 0;
  border: 0;
  color: var(--green-deep);
  background: transparent;
  font-size: 12px;
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
  .cloud-auth-panel {
    padding: 22px 18px;
  }

  .cloud-auth-panel > header {
    gap: 16px;
  }

  .cloud-auth-panel small,
  .cloud-auth-panel label,
  .backend-message,
  .regional-sync-note {
    font-size: 12px;
  }

  .cloud-auth-panel button,
  .cloud-auth-panel input {
    min-height: 44px;
  }

  .cloud-auth-form {
    grid-template-columns: 1fr;
  }

  .cloud-auth-actions {
    flex-direction: column;
  }

  .cloud-auth-actions button {
    justify-content: center;
  }
}
</style>
