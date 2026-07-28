<script setup lang="ts">
import { Database, FolderCheck, RefreshCw, ShieldCheck, X } from '@lucide/vue'
import { nextTick, onMounted, ref, watch } from 'vue'

const props = defineProps<{
  busy: boolean
  errorMessage?: string
}>()

const emit = defineEmits<{
  cancel: []
  confirm: []
}>()

const panel = ref<HTMLElement>()
const cancelButton = ref<HTMLButtonElement>()

onMounted(async () => {
  await nextTick()
  cancelButton.value?.focus()
})

watch(() => props.busy, async (busy) => {
  if (!busy) return
  await nextTick()
  panel.value?.focus()
})

function close() {
  if (!props.busy) emit('cancel')
}

function confirm() {
  if (!props.busy) emit('confirm')
}

function handleKeydown(event: KeyboardEvent) {
  if (event.key === 'Escape') {
    event.preventDefault()
    close()
    return
  }
  if (event.key !== 'Tab') return
  const focusable = [...(panel.value?.querySelectorAll<HTMLElement>('button:not(:disabled)') ?? [])]
  if (!focusable.length) {
    event.preventDefault()
    panel.value?.focus()
    return
  }
  const first = focusable[0]
  const last = focusable.at(-1)
  if (event.shiftKey && document.activeElement === first) {
    event.preventDefault()
    last?.focus()
  }
  else if (!event.shiftKey && document.activeElement === last) {
    event.preventDefault()
    first?.focus()
  }
}
</script>

<template>
  <div
    class="migration-backdrop"
    @mousedown.self="close"
  >
    <section
      ref="panel"
      class="migration-dialog"
      tabindex="-1"
      role="dialog"
      aria-modal="true"
      aria-labelledby="storage-migration-title"
      aria-describedby="storage-migration-description"
      :aria-busy="busy"
      @keydown="handleKeydown"
    >
      <button
        class="close-button"
        type="button"
        aria-label="关闭存储迁移确认"
        :disabled="busy"
        @click="close"
      >
        <X :size="18" />
      </button>

      <div class="migration-mark">
        <Database :size="27" />
      </div>
      <p class="eyebrow">
        本地资料库 · 事务化迁移
      </p>
      <h2 id="storage-migration-title">
        把资料库移到另一个磁盘？
      </h2>
      <p
        id="storage-migration-description"
        class="description"
      >
        下一步会打开 Windows 文件夹选择器。应用只在选定位置创建自己的专用目录，不会把绝对路径显示在页面或写入诊断提示。
      </p>

      <ol class="migration-steps">
        <li>
          <FolderCheck :size="18" />
          <span><strong>复制完整加密快照</strong><small>SQLCipher 数据库与所有被引用的加密图片一起复制</small></span>
        </li>
        <li>
          <ShieldCheck :size="18" />
          <span><strong>逐项校验后再切换</strong><small>数据库、账户边界、资源大小和密文哈希全部通过才会提交</small></span>
        </li>
        <li>
          <RefreshCw :size="18" />
          <span><strong>校验成功后自动重启</strong><small>失败时原资料库保持不变，应用不会留下半迁移状态</small></span>
        </li>
      </ol>

      <aside class="migration-boundary">
        迁移期间会暂时停止手机扫码采集。不要拔出目标磁盘；如果意外中断，下次启动会自动完成提交或安全回滚。
      </aside>

      <p
        v-if="errorMessage"
        class="migration-error"
        role="alert"
      >
        {{ errorMessage }}
      </p>

      <div class="dialog-actions">
        <button
          ref="cancelButton"
          type="button"
          :disabled="busy"
          @click="close"
        >
          取消，保持原位置
        </button>
        <button
          class="confirm-button"
          type="button"
          :disabled="busy"
          @click="confirm"
        >
          <RefreshCw
            v-if="busy"
            :size="16"
            class="spinning"
          />
          <FolderCheck
            v-else
            :size="16"
          />
          {{ busy ? '正在复制并校验…' : '选择文件夹并开始迁移' }}
        </button>
      </div>
    </section>
  </div>
</template>

<style scoped>
.migration-backdrop {
  position: fixed;
  inset: 0;
  z-index: 86;
  display: grid;
  padding: 24px;
  place-items: center;
  background: rgba(24, 34, 30, .58);
  animation: migration-backdrop-in 180ms cubic-bezier(.2, .8, .2, 1) both;
}

.migration-dialog {
  position: relative;
  width: min(620px, 100%);
  max-height: calc(100vh - 48px);
  max-height: calc(100dvh - 48px);
  padding: 30px;
  overflow: auto;
  border: 1px solid rgba(185, 88, 63, .28);
  border-radius: 22px;
  background: var(--paper-raised);
  box-shadow: 0 30px 84px rgba(22, 32, 28, .3);
  animation: migration-dialog-in 240ms cubic-bezier(.2, .8, .2, 1) both;
}

.migration-dialog::before {
  position: absolute;
  inset: 9px;
  border: 1px solid rgba(185, 88, 63, .08);
  border-radius: 15px;
  content: "";
  pointer-events: none;
}

.close-button {
  position: absolute;
  z-index: 1;
  top: 18px;
  right: 18px;
  display: grid;
  width: 36px;
  height: 36px;
  padding: 0;
  place-items: center;
  border: 0;
  border-radius: 50%;
  background: rgba(33, 51, 45, .06);
  cursor: pointer;
}

.migration-mark {
  display: grid;
  width: 52px;
  height: 52px;
  margin-bottom: 17px;
  place-items: center;
  color: var(--paper-raised);
  border-radius: 16px;
  background: var(--green-deep);
  box-shadow: 0 10px 24px rgba(33, 51, 45, .2);
}

.eyebrow {
  margin: 0 0 7px;
  color: var(--cinnabar);
  font-size: 11px;
  font-weight: 800;
  letter-spacing: .15em;
}

h2 {
  margin: 0;
  color: var(--green-deep);
  font-family: var(--font-serif);
  font-size: 28px;
}

.description {
  margin: 12px 0 0;
  color: var(--ink-muted);
  line-height: 1.75;
}

.migration-steps {
  display: grid;
  gap: 9px;
  margin: 22px 0 0;
  padding: 0;
  list-style: none;
}

.migration-steps li {
  display: grid;
  grid-template-columns: 34px 1fr;
  gap: 10px;
  align-items: center;
  padding: 12px 14px;
  border: 1px solid rgba(34, 48, 43, .09);
  border-radius: 13px;
  background: rgba(232, 221, 199, .28);
}

.migration-steps li > svg {
  color: var(--cinnabar);
}

.migration-steps span {
  display: grid;
  gap: 3px;
}

.migration-steps strong {
  color: var(--ink);
  font-size: 13px;
}

.migration-steps small {
  color: var(--ink-muted);
  font-size: 11px;
  line-height: 1.55;
}

.migration-boundary {
  margin-top: 14px;
  padding: 11px 13px;
  color: #74594d;
  border-left: 3px solid rgba(185, 88, 63, .5);
  border-radius: 4px 10px 10px 4px;
  background: rgba(247, 225, 216, .48);
  font-size: 11px;
  line-height: 1.65;
}

.migration-error {
  margin: 14px 0 0;
  padding: 11px 13px;
  color: #843d2c;
  border-radius: 10px;
  background: rgba(185, 88, 63, .09);
  font-size: 12px;
  line-height: 1.55;
}

.dialog-actions {
  display: flex;
  justify-content: flex-end;
  gap: 10px;
  margin-top: 22px;
}

.dialog-actions button {
  display: inline-flex;
  gap: 7px;
  align-items: center;
  min-height: 42px;
  padding: 10px 15px;
  border: 1px solid var(--line);
  border-radius: 11px;
  background: var(--paper);
  cursor: pointer;
}

.dialog-actions button:disabled,
.close-button:disabled {
  cursor: wait;
  opacity: .56;
}

.dialog-actions .confirm-button {
  color: var(--paper-raised);
  border-color: var(--green-deep);
  background: var(--green-deep);
}

.spinning {
  animation: migration-spin 900ms linear infinite;
}

@keyframes migration-backdrop-in {
  from { opacity: 0; }
}

@keyframes migration-dialog-in {
  from { opacity: 0; transform: translateY(8px); }
}

@keyframes migration-spin {
  to { transform: rotate(360deg); }
}

@media (max-width: 560px) {
  .migration-backdrop { padding: 14px; }
  .migration-dialog { padding: 25px 20px 20px; }
  .dialog-actions { display: grid; }
  .dialog-actions button { justify-content: center; }
}

@media (prefers-reduced-motion: reduce) {
  .migration-backdrop,
  .migration-dialog,
  .spinning {
    animation: none;
  }
}
</style>
