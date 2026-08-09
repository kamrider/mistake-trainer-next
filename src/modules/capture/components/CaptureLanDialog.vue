<script setup lang="ts">
import { LockKeyhole, QrCode, X } from '@lucide/vue'
import { computed, nextTick, onBeforeUnmount, onMounted, ref, watch } from 'vue'
import { acquireDialogDocumentBoundary } from '../../../app/dialog-document-boundary'
import { trapDialogFocus } from '../../../app/dialog-focus'
import type {
  CaptureLanAddress,
  CaptureLanPreflight,
  CaptureLanSession,
} from '../../../shared/api/bindings'

const props = defineProps<{
  addresses: CaptureLanAddress[]
  preflight: CaptureLanPreflight | undefined
  preflightBusy: boolean
  session: CaptureLanSession | undefined
  busy: boolean
}>()

const emit = defineEmits<{
  close: []
  start: [selectedAddress: string | null]
  refreshAddresses: []
  refreshPreflight: []
  stop: []
}>()

const dialog = ref<HTMLElement>()
const closeButton = ref<HTMLButtonElement>()
const selectedAddress = ref('')
let releaseDialogBoundary: (() => void) | undefined
const minutesRemaining = computed(() => props.session
  ? Math.max(0, Math.ceil(((props.session.expiresAtUtcMs ?? Date.now()) - Date.now()) / 60_000))
  : 0)
const needsRepair = computed(() => props.preflight?.needsFirewallRepair === true)
const ready = computed(() => props.preflight?.canStart === true)

watch(() => props.addresses, (addresses) => {
  if (!addresses.some(address => address.address === selectedAddress.value)) {
    selectedAddress.value = addresses[0]?.address ?? ''
  }
}, { immediate: true })

watch(
  () => [
    props.session?.sessionId,
    props.preflight?.needsFirewallRepair,
    props.preflight?.canStart,
  ],
  async () => {
    await nextTick()
    const activeElement = document.activeElement
    if (!(activeElement instanceof Node) || !dialog.value?.contains(activeElement)) {
      closeButton.value?.focus()
    }
  },
  { flush: 'post' },
)

onMounted(async () => {
  if (dialog.value) releaseDialogBoundary = acquireDialogDocumentBoundary(dialog.value)
  await nextTick()
  closeButton.value?.focus()
})

onBeforeUnmount(() => {
  releaseDialogBoundary?.()
})

function start() {
  emit('start', selectedAddress.value || props.addresses[0]?.address || null)
}

function formatBytes(value: number) {
  if (value < 1024 * 1024) return `${Math.round(value / 1024)} KB`
  return `${(value / (1024 * 1024)).toFixed(1)} MB`
}

function handleKeydown(event: KeyboardEvent) {
  if (event.key === 'Escape') {
    event.preventDefault()
    emit('close')
    return
  }
  trapDialogFocus(event, dialog.value)
}
</script>

<template>
  <div
    class="lan-overlay"
    role="presentation"
    @mousedown.self="emit('close')"
  >
    <section
      ref="dialog"
      class="lan-dialog"
      tabindex="-1"
      role="dialog"
      aria-modal="true"
      aria-labelledby="lan-dialog-title"
      @keydown="handleKeydown"
    >
      <button
        ref="closeButton"
        class="lan-close"
        type="button"
        aria-label="关闭手机采集"
        @click="emit('close')"
      >
        <X :size="18" />
      </button>

      <template v-if="session">
        <p class="eyebrow">
          手机局域网采集
        </p>
        <h2 id="lan-dialog-title">
          扫码开始连拍
        </h2>
        <p class="lan-intro">
          手机与电脑需要在同一个家庭 Wi‑Fi 或个人热点中。
        </p>
        <div class="lan-session-grid">
          <div class="qr-paper">
            <img
              :src="session.qrSvgDataUrl"
              alt="手机采集二维码"
            >
          </div>
          <div class="lan-session-copy">
            <div><span>网络地址</span><strong>{{ session.selectedAddress }}</strong></div>
            <div><span>电脑已收到</span><strong>{{ session.receivedItemCount }} 张 · {{ formatBytes(session.receivedBytes ?? 0) }}</strong></div>
            <div><span>自动过期</span><strong>约 {{ minutesRemaining }} 分钟后</strong></div>
            <p>上传时请保持 Windows 应用运行。结束或退出应用会立即关闭端口。</p>
            <button
              class="lan-stop"
              type="button"
              :disabled="busy"
              @click="emit('stop')"
            >
              停止手机采集
            </button>
          </div>
        </div>
      </template>

      <template v-else>
        <p class="eyebrow">
          手机局域网采集
        </p>
        <div
          class="lan-preflight"
          aria-live="polite"
        >
          <template v-if="busy || preflightBusy">
            <h2 id="lan-dialog-title">
              正在准备手机连接
            </h2>
            <p class="lan-intro">
              首次使用时 Windows 会请求一次管理员授权。请在系统弹窗中确认应用是 Mistake Trainer Next，然后点击“是”。
            </p>
            <div class="lan-progress">
              <span />检查持久权限并启动二维码
            </div>
          </template>

          <template v-else-if="!preflight">
            <h2 id="lan-dialog-title">
              暂时没有读到连接状态
            </h2>
            <p class="lan-intro">
              应用没有修改任何系统设置。重新检测后仍可继续。
            </p>
            <button
              class="lan-secondary"
              type="button"
              @click="emit('refreshPreflight')"
            >
              重新检测
            </button>
          </template>

          <template v-else-if="!preflight.supported">
            <h2 id="lan-dialog-title">
              当前系统暂不支持自动修复
            </h2>
            <p class="lan-intro">
              手机扫码采集的权限引导目前只支持 Windows 桌面版。
            </p>
          </template>

          <template v-else-if="needsRepair">
            <span class="lan-state is-attention">Windows 授权尚未完成</span>
            <h2 id="lan-dialog-title">
              下次扫码会再次请求授权
            </h2>
            <p class="lan-intro">
              点击下面的按钮会重新弹出 Windows 管理员确认。授权成功后规则会保留，以后点击“手机扫码”会直接生成二维码。
            </p>
            <div class="lan-permission-summary">
              <LockKeyhole :size="18" /><p><strong>授权范围</strong><span>当前应用 · TCP 入站 · 本地子网；不关闭防火墙，不改动其他软件。</span></p>
            </div>
            <div class="lan-actions">
              <button
                class="lan-primary"
                type="button"
                :disabled="busy"
                @click="start"
              >
                再次授权并生成二维码
              </button>
              <button
                class="lan-secondary"
                type="button"
                :disabled="busy"
                @click="emit('refreshPreflight')"
              >
                重新检测
              </button>
            </div>
            <details class="lan-troubleshooting">
              <summary>没有弹窗，或者我没有管理员权限</summary>
              <ul>
                <li>如果弹窗要求管理员密码，请输入这台电脑管理员账户的密码；不知道密码时请让电脑管理员协助。</li>
                <li>如果点了“否”或关闭弹窗，不会记录为成功；下一次扫码会再次请求。</li>
                <li>授权不会关闭 Windows 防火墙，也不会修改其他软件的网络规则。</li>
                <li>本规则覆盖 Windows 网络类型，但只允许当前程序接收来自本地子网的 TCP 连接。</li>
              </ul>
            </details>
          </template>

          <template v-else-if="ready">
            <span class="lan-state is-ready">持久连接权限已就绪</span>
            <h2 id="lan-dialog-title">
              选择手机所在的网络
            </h2>
            <p class="lan-intro">
              权限只允许本地子网访问当前应用。仍建议在家庭 Wi‑Fi 或个人热点中使用，不要在陌生公共网络上传题目。
            </p>
            <label class="lan-address-label">网络接口
              <select
                v-model="selectedAddress"
                :disabled="busy || !addresses.length"
              >
                <option
                  v-for="address in addresses"
                  :key="address.address"
                  :value="address.address"
                >{{ address.label }} · {{ address.address }}</option>
              </select>
            </label>
            <p
              v-if="!addresses.length"
              class="lan-empty"
            >
              没有检测到家庭网络地址，请先连接 Wi‑Fi 或个人热点。
            </p>
            <button
              v-if="!addresses.length"
              class="lan-secondary"
              type="button"
              :disabled="busy"
              @click="emit('refreshAddresses')"
            >
              重新检测网络
            </button>
            <button
              class="lan-start"
              type="button"
              :disabled="busy || !addresses.length"
              @click="start"
            >
              <QrCode :size="17" /> 生成二维码
            </button>
          </template>

          <template v-else>
            <h2 id="lan-dialog-title">
              连接权限状态异常
            </h2>
            <p class="lan-intro">
              二维码尚未生成，应用也没有开放端口。请重新检测后再试。
            </p>
            <button
              class="lan-secondary"
              type="button"
              @click="emit('refreshPreflight')"
            >
              重新检测
            </button>
          </template>
        </div>
      </template>
    </section>
  </div>
</template>

<style scoped>
.lan-overlay {
  position: fixed;
  z-index: 80;
  inset: 0;
  display: grid;
  padding: 24px;
  place-items: center;
  background: rgba(28, 38, 34, .46);
  backdrop-filter: blur(9px);
}

.lan-dialog {
  position: relative;
  width: min(760px, 100%);
  max-height: min(780px, calc(100vh - 48px));
  max-height: min(780px, calc(100dvh - 48px));
  padding: 34px;
  overflow: auto;
  border: 1px solid rgba(33, 51, 45, .14);
  border-radius: 8px 30px 30px;
  background: var(--paper);
  box-shadow: 0 30px 90px rgba(20, 30, 26, .28);
}

.lan-dialog h2 {
  margin: 4px 0 0;
  font-family: var(--font-serif);
  font-size: 34px;
}

.eyebrow {
  margin: 0;
  color: var(--cinnabar);
  font-size: 12px;
  font-weight: 800;
  letter-spacing: .14em;
}

.lan-intro {
  margin: 10px 0 24px;
  color: var(--ink-muted);
  font-size: 13px;
  line-height: 1.65;
}

.lan-close {
  position: absolute;
  top: 17px;
  right: 17px;
  display: grid;
  width: 44px;
  height: 44px;
  padding: 0;
  place-items: center;
  color: var(--ink);
  border: 0;
  border-radius: 50%;
  background: var(--sand);
  cursor: pointer;
}

.lan-session-grid {
  display: grid;
  grid-template-columns: minmax(250px, 320px) 1fr;
  gap: 30px;
  align-items: center;
}

.qr-paper {
  padding: 18px;
  border: 1px solid var(--line);
  border-radius: 18px;
  background: #fff;
}

.qr-paper img {
  display: block;
  width: 100%;
  aspect-ratio: 1;
}

.lan-session-copy {
  display: grid;
  gap: 13px;
}

.lan-session-copy div {
  display: grid;
  gap: 3px;
  padding-bottom: 12px;
  border-bottom: 1px solid var(--line);
}

.lan-session-copy span,
.lan-address-label {
  color: var(--ink-muted);
  font-size: 12px;
  font-weight: 720;
}

.lan-session-copy strong {
  font-size: 16px;
}

.lan-session-copy p,
.lan-empty {
  margin: 2px 0;
  color: var(--ink-muted);
  font-size: 12px;
  line-height: 1.65;
}

.lan-stop,
.lan-start {
  display: inline-flex;
  min-width: 44px;
  min-height: 44px;
  gap: 8px;
  align-items: center;
  justify-content: center;
  padding: 0 18px;
  color: var(--paper);
  border: 0;
  border-radius: 999px;
  background: var(--vermillion);
  font-weight: 760;
  cursor: pointer;
}

.lan-stop {
  justify-self: start;
}

.lan-start {
  margin-top: 20px;
  background: var(--green-deep);
}

.lan-address-label {
  display: grid;
  gap: 8px;
}

.lan-address-label select {
  min-height: 48px;
  padding: 0 14px;
  color: var(--ink);
  border: 1px solid var(--line-strong);
  border-radius: 13px;
  background: #fffdf8;
}

.lan-preflight {
  min-height: 240px;
}

.lan-progress {
  display: flex;
  gap: 10px;
  align-items: center;
  color: var(--ink-muted);
  font-size: 12px;
}

.lan-progress span {
  width: 9px;
  height: 9px;
  border-radius: 50%;
  background: var(--cinnabar);
  animation: lan-pulse 1.1s ease-in-out infinite alternate;
}

.lan-state {
  display: inline-flex;
  margin-bottom: 11px;
  padding: 6px 10px;
  border-radius: 999px;
  font-size: 12px;
  font-weight: 800;
  letter-spacing: .04em;
}

.lan-state.is-attention {
  color: #87402f;
  background: rgba(185, 88, 63, .12);
}

.lan-state.is-ready {
  color: #3f6857;
  background: var(--green-soft);
}

.lan-actions {
  display: flex;
  gap: 10px;
  align-items: center;
  flex-wrap: wrap;
}

.lan-primary,
.lan-secondary {
  min-width: 44px;
  min-height: 44px;
  padding: 0 17px;
  border-radius: 999px;
  font-weight: 760;
  cursor: pointer;
}

.lan-primary {
  color: var(--paper);
  border: 1px solid var(--green-deep);
  background: var(--green-deep);
}

.lan-secondary {
  color: var(--green-deep);
  border: 1px solid var(--line-strong);
  background: transparent;
}

.lan-permission-summary {
  display: flex;
  gap: 10px;
  align-items: center;
  margin: 0 0 14px;
  padding: 12px 13px;
  color: var(--green-deep);
  border: 1px solid rgba(33, 51, 45, .12);
  border-radius: 12px;
  background: var(--green-soft);
}

.lan-permission-summary p {
  display: grid;
  gap: 2px;
  margin: 0;
}

.lan-permission-summary strong {
  font-size: 13px;
}

.lan-permission-summary span {
  color: var(--ink-muted);
  font-size: 12px;
  line-height: 1.5;
}

.lan-troubleshooting {
  margin-top: 13px;
  color: var(--ink-muted);
  border: 1px solid rgba(33, 51, 45, .12);
  border-radius: 11px;
  background: rgba(232, 221, 199, .18);
}

.lan-troubleshooting summary {
  min-height: 44px;
  padding: 11px 13px;
  color: var(--green-deep);
  font-size: 12px;
  font-weight: 760;
  cursor: pointer;
}

.lan-troubleshooting ul {
  display: grid;
  gap: 7px;
  margin: 0;
  padding: 0 18px 14px 31px;
  font-size: 12px;
  line-height: 1.55;
}

.lan-primary:focus-visible,
.lan-secondary:focus-visible,
.lan-start:focus-visible,
.lan-stop:focus-visible,
.lan-close:focus-visible,
.lan-troubleshooting summary:focus-visible {
  outline: 3px solid rgba(185, 88, 63, .28);
  outline-offset: 2px;
}

button:disabled,
select:disabled {
  cursor: not-allowed;
  opacity: .48;
}

@keyframes lan-pulse {
  to { transform: scale(1.35); opacity: .45; }
}

@media (max-width: 980px) {
  .lan-session-grid {
    grid-template-columns: 1fr;
  }

  .qr-paper {
    width: min(320px, 100%);
    margin: auto;
  }
}

@media (max-width: 720px) {
  .lan-overlay {
    padding: 12px;
  }

  .lan-dialog {
    max-height: calc(100vh - 24px);
    max-height: calc(100dvh - 24px);
    padding: 28px 20px;
  }

  .lan-dialog h2 {
    padding-right: 38px;
    font-size: 28px;
  }

  .lan-actions {
    align-items: stretch;
    flex-direction: column;
  }

  .lan-actions > *,
  .lan-start,
  .lan-stop {
    width: 100%;
  }
}

@media (prefers-reduced-motion: reduce) {
  .lan-progress span {
    animation: none;
  }
}
</style>
