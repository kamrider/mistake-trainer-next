<script setup lang="ts">
import { Archive, CloudOff, Database, Laptop, LockKeyhole, ShieldCheck, Trash2, TriangleAlert } from '@lucide/vue'
import { computed, ref } from 'vue'
import type { CloudAuthState, LibraryAccessStatus, SettingsOverview, WindowsCompatibilityStatus } from '../../shared/api/bindings'
import { formatSettingsAuthStatus } from '../settings-formatters'

const props = defineProps<{
  overview: SettingsOverview
  accessStatus: LibraryAccessStatus | undefined
  accessError: string
  cloudAuth: CloudAuthState | undefined
  windowsCompatibility: WindowsCompatibilityStatus | undefined
  loading: boolean
}>()

const emit = defineEmits<{
  requestLock: []
}>()

const lockAction = ref<HTMLButtonElement>()
const deviceStatusKey = computed(() => [
  props.accessStatus?.trustedWindowsAccount ? 'trusted' : 'unavailable',
  props.cloudAuth?.status.kind ?? 'checking',
  props.accessError ? 'error' : 'ready',
].join(':'))

function focusLockAction() {
  lockAction.value?.focus()
}

defineExpose({ focusLockAction })
</script>

<template>
  <section
    id="settings-overview"
    class="settings-grid"
    :aria-busy="loading"
  >
    <article class="setting-card encryption-card device-security-card">
      <div class="icon">
        <LockKeyhole :size="22" />
      </div>
      <div class="device-security-copy">
        <p>当前设备保护</p>
        <h2>这台 Windows 电脑</h2>
        <span>这里只显示当前设备的真实保护状态，不暴露设备编号、密钥或本机路径。</span>
        <Transition
          name="device-status"
          mode="out-in"
        >
          <dl
            :key="deviceStatusKey"
            class="device-security-list"
          >
            <div>
              <dt>本地资料库</dt>
              <dd>{{ overview.localEncryptionReady ? 'SQLCipher 与原图独立加密' : '正在检查加密状态' }}</dd>
            </div>
            <div>
              <dt>离线解锁</dt>
              <dd>{{ accessStatus?.trustedWindowsAccount ? '当前 Windows 账户可解锁' : '状态暂不可用' }}</dd>
            </div>
            <div>
              <dt>云端会话</dt>
              <dd>{{ cloudAuth ? formatSettingsAuthStatus(cloudAuth.status.kind) : '正在检查' }}</dd>
            </div>
          </dl>
        </Transition>
        <span
          v-if="cloudAuth?.status.kind === 'connected' || cloudAuth?.status.kind === 'offline'"
          class="device-scope-note"
        >退出云端只影响这台电脑，其他设备保持登录。</span>
        <span
          v-if="accessError"
          class="device-access-error"
          role="status"
          aria-label="当前设备保护状态"
        >{{ accessError }}</span>
        <button
          ref="lockAction"
          type="button"
          class="lock-now"
          @click="emit('requestLock')"
        >
          <LockKeyhole :size="15" />立即锁定资料库
        </button>
      </div>
      <strong><ShieldCheck :size="15" />本机安全边界</strong>
    </article>

    <article class="setting-card">
      <div class="icon">
        <Database :size="22" />
      </div>
      <div>
        <p>题库状态</p>
        <h2>{{ overview.activeProblemCount }} 道活动题</h2>
        <span><Archive :size="13" /> {{ overview.archivedProblemCount }} 道归档 · <Trash2 :size="13" /> {{ overview.trashedProblemCount }} 道回收站</span>
      </div>
    </article>

    <article
      v-if="windowsCompatibility"
      class="setting-card windows-compatibility-card"
      aria-label="Windows 兼容性"
    >
      <div class="icon">
        <Laptop :size="22" />
      </div>
      <div>
        <p>Windows 兼容性</p>
        <h2>
          {{ windowsCompatibility.supportLevel === 'supported'
            ? '完整支持'
            : windowsCompatibility.supportLevel === 'extended' ? '扩展兼容' : '不受支持' }}
        </h2>
        <span>
          {{ windowsCompatibility.osName }}
          · Build {{ windowsCompatibility.buildNumber }}.{{ windowsCompatibility.updateBuildRevision }}
          · {{ windowsCompatibility.nativeArchitecture }}
        </span>
        <small>
          WebView2 {{ windowsCompatibility.webview2Version ?? '未检测到' }}
          · 最低 Build {{ windowsCompatibility.minimumWindowsBuild }}
        </small>
      </div>
    </article>

    <article class="setting-card">
      <div class="icon">
        <CloudOff :size="22" />
      </div>
      <div>
        <p>云端同步</p>
        <h2>{{ overview.cloudSyncConfigured ? '已配置' : '尚未配置' }}</h2>
        <span>待同步变更 {{ overview.pendingOperationCount }} 项；配置账户后可从上方手动同步。</span>
      </div>
    </article>

    <article class="setting-card">
      <div class="icon">
        <TriangleAlert :size="22" />
      </div>
      <div>
        <p>需要关注</p>
        <h2>{{ overview.failedOperationCount + overview.unresolvedConflictCount }} 项</h2>
        <span>{{ overview.failedOperationCount }} 项失败操作 · {{ overview.unresolvedConflictCount }} 项未解决冲突</span>
      </div>
    </article>
  </section>
</template>

<style scoped>
.settings-grid {
  display: grid;
  grid-template-columns: repeat(2, minmax(0, 1fr));
  gap: 15px;
}

.setting-card {
  position: relative;
  display: grid;
  grid-template-columns: 48px 1fr;
  gap: 14px;
  min-height: 150px;
  padding: 22px;
  border: 1px solid var(--line);
  border-radius: 17px;
  background: rgba(255, 253, 247, .78);
  box-shadow: 0 16px 48px rgba(34, 48, 43, .05);
}

.setting-card .icon {
  display: grid;
  width: 44px;
  height: 44px;
  color: var(--green-deep);
  border-radius: 13px;
  background: var(--green-soft);
  place-items: center;
}

.setting-card p {
  margin: 1px 0 8px;
  color: var(--ink-muted);
  font-size: 12px;
  font-weight: 800;
  letter-spacing: .08em;
  text-transform: uppercase;
}

h2 {
  margin: 0;
  color: var(--green-deep);
  font-family: Georgia, 'Microsoft YaHei', serif;
  font-size: 21px;
}

.setting-card span {
  display: flex;
  gap: 5px;
  align-items: center;
  margin-top: 10px;
  color: var(--ink-muted);
  font-size: 12px;
  line-height: 1.7;
}

.setting-card > strong {
  position: absolute;
  right: 18px;
  bottom: 16px;
  display: flex;
  gap: 5px;
  align-items: center;
  color: #557263;
  font-size: 12px;
}

.encryption-card {
  min-height: 192px;
  padding-bottom: 46px;
  border-color: rgba(33, 51, 45, .28);
}

.device-security-card {
  grid-column: 1 / -1;
  grid-template-columns: 48px minmax(0, 1fr);
}

.device-security-copy {
  min-width: 0;
}

.device-security-list {
  display: grid;
  grid-template-columns: repeat(3, minmax(0, 1fr));
  gap: 9px;
  margin: 15px 0 0;
}

.device-security-list div {
  min-width: 0;
  padding: 11px 12px;
  border: 1px solid rgba(33, 51, 45, .12);
  border-radius: 11px;
  background: rgba(232, 221, 199, .24);
}

.device-security-list dt {
  color: var(--ink-muted);
  font-size: 12px;
  font-weight: 800;
  letter-spacing: .06em;
}

.device-security-list dd {
  margin: 5px 0 0;
  overflow-wrap: anywhere;
  color: var(--green-deep);
  font-size: 12px;
  font-weight: 700;
}

.device-status-enter-active,
.device-status-leave-active {
  transition:
    opacity var(--motion-standard) var(--ease-standard),
    transform var(--motion-standard) var(--ease-standard);
}

.device-status-enter-from {
  opacity: 0;
  transform: translateY(5px);
}

.device-status-leave-to {
  opacity: 0;
  transform: translateY(-3px);
}

.device-security-copy .device-scope-note,
.device-security-copy .device-access-error {
  display: block;
  margin-top: 10px;
}

.device-security-copy .device-scope-note {
  color: #557263;
}

.device-security-copy .device-access-error {
  color: #843d2c;
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

button:focus-visible {
  outline: 3px solid rgba(185, 88, 63, .24);
  outline-offset: 2px;
}

.lock-now {
  width: fit-content;
  margin-top: 13px;
  padding: 8px 11px;
  color: var(--green-deep);
  border-color: rgba(33, 51, 45, .25);
  background: var(--green-soft);
  font-size: 12px;
  font-weight: 700;
  transition: transform var(--motion-feedback) var(--ease-standard);
}

.lock-now:hover {
  transform: translateY(-1px);
  box-shadow: 0 8px 18px rgba(33, 51, 45, .1);
}

@media (max-width: 760px) {
  .settings-grid {
    grid-template-columns: 1fr;
  }

  .device-security-list {
    grid-template-columns: 1fr;
  }

  .setting-card small,
  .setting-card dt {
    font-size: 12px;
  }

  .setting-card button {
    min-height: 44px;
  }
}

@media (prefers-reduced-motion: reduce) {
  .lock-now,
  .device-status-enter-active,
  .device-status-leave-active {
    transition: none;
  }

  .lock-now:hover,
  .device-status-enter-from,
  .device-status-leave-to {
    transform: none;
  }
}
</style>
