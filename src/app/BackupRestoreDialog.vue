<script setup lang="ts">
import { FolderCheck, RefreshCw, ShieldCheck, TriangleAlert, X } from '@lucide/vue'
import { nextTick, onMounted, ref } from 'vue'
import type { BackupRestoreCandidate } from '../shared/api/bindings'

const props = defineProps<{ candidate: BackupRestoreCandidate, busy: boolean }>()
const emit = defineEmits<{ cancel: [], confirm: [] }>()

const panel = ref<HTMLElement>()
const acknowledged = ref(false)
const cancelButton = ref<HTMLButtonElement>()

onMounted(async () => {
  await nextTick()
  cancelButton.value?.focus()
})

function close() {
  emit('cancel')
}

function handleKeydown(event: KeyboardEvent) {
  if (event.key === 'Escape') {
    event.preventDefault()
    if (!props.busy) close()
    return
  }
  if (event.key !== 'Tab') return
  const focusable = [...(panel.value?.querySelectorAll<HTMLElement>('button:not(:disabled), input:not(:disabled)') ?? [])]
  if (!focusable.length) return
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
    class="restore-backdrop"
    @mousedown.self="!busy && close()"
  >
    <section
      ref="panel"
      class="restore-dialog"
      role="dialog"
      aria-modal="true"
      aria-labelledby="restore-dialog-title"
      aria-describedby="restore-dialog-description"
      @keydown="handleKeydown"
    >
      <button
        class="close-button"
        type="button"
        aria-label="关闭恢复确认"
        :disabled="busy"
        @click="close"
      >
        <X :size="18" />
      </button>
      <div class="dialog-mark">
        <ShieldCheck :size="26" />
      </div>
      <p class="eyebrow">
        安全恢复 · 最后确认
      </p>
      <h2 id="restore-dialog-title">
        用这份备份替换当前资料库？
      </h2>
      <p
        id="restore-dialog-description"
        class="description"
      >
        应用会自动重启。当前资料库会先完整保留为回滚副本，只有恢复后的加密数据库成功打开，旧副本才会清理。
      </p>

      <article class="candidate-card">
        <FolderCheck :size="20" />
        <div><strong>{{ candidate.summary.label }}</strong><span>{{ candidate.summary.assetCount }} 个加密图片资源</span></div>
        <small>已复制并通过两次完整性校验</small>
      </article>

      <ol class="restore-steps">
        <li><ShieldCheck :size="17" /><span><strong>保留旧资料库</strong><small>先创建同磁盘回滚副本</small></span></li>
        <li><RefreshCw :size="17" /><span><strong>自动重启并验证</strong><small>重启前不会热替换数据库</small></span></li>
        <li><TriangleAlert :size="17" /><span><strong>异常自动回退</strong><small>恢复库打不开时立即换回原资料</small></span></li>
      </ol>

      <label class="acknowledge">
        <input
          v-model="acknowledged"
          type="checkbox"
          :disabled="busy"
        >
        <span>我明白：确认后当前题库会由上述备份替换，未备份的本机改动将不再保留。</span>
      </label>

      <div class="dialog-actions">
        <button
          ref="cancelButton"
          type="button"
          :disabled="busy"
          @click="close"
        >
          取消，保持现状
        </button>
        <button
          class="confirm-button"
          type="button"
          :disabled="busy || !acknowledged"
          @click="emit('confirm')"
        >
          <RefreshCw
            :size="16"
            :class="{ spinning: busy }"
          />{{ busy ? '正在准备重启…' : '确认恢复并重启' }}
        </button>
      </div>
    </section>
  </div>
</template>

<style scoped>
.restore-backdrop { position: fixed; inset: 0; z-index: 80; display: grid; padding: 24px; place-items: center; background: rgba(24,34,30,.54); backdrop-filter: blur(7px); animation: backdrop-in var(--motion-standard) ease both; }
.restore-dialog { position: relative; width: min(620px,100%); max-height: calc(100vh - 48px); padding: 30px; overflow: auto; border: 1px solid rgba(185,88,63,.3); border-radius: 22px; background: #fffdf7; box-shadow: 0 28px 80px rgba(22,32,28,.28); animation: dialog-in var(--motion-page) cubic-bezier(.2,.8,.2,1) both; }
.close-button { position: absolute; top: 18px; right: 18px; display: grid; width: 36px; height: 36px; padding: 0; place-items: center; border: 0; border-radius: 50%; background: rgba(33,51,45,.06); cursor: pointer; }
.dialog-mark { display: grid; width: 50px; height: 50px; margin-bottom: 16px; place-items: center; color: #fffdf7; border-radius: 16px; background: var(--green-deep); }
.eyebrow { margin: 0 0 7px; color: var(--cinnabar); font-size: 11px; font-weight: 800; letter-spacing: .15em; }
h2 { margin: 0; color: var(--green-deep); font-family: Georgia,'Microsoft YaHei',serif; font-size: 27px; }
.description { margin: 12px 0 0; color: var(--ink-muted); line-height: 1.75; }
.candidate-card { display: grid; grid-template-columns: auto 1fr; gap: 4px 12px; align-items: center; margin-top: 20px; padding: 16px; color: #557263; border: 1px solid rgba(33,51,45,.16); border-radius: 14px; background: rgba(33,51,45,.055); }
.candidate-card svg { grid-row: 1 / 3; }.candidate-card div { display: grid; gap: 3px; min-width: 0; }.candidate-card strong { color: var(--green-deep); overflow-wrap: anywhere; }.candidate-card span,.candidate-card small { color: var(--ink-muted); font-size: 11px; }.candidate-card>small { grid-column: 2; }
.restore-steps { display: grid; grid-template-columns: repeat(3,1fr); gap: 9px; margin: 14px 0 0; padding: 0; list-style: none; }.restore-steps li { display: flex; gap: 8px; padding: 13px; color: var(--cinnabar); border-radius: 12px; background: rgba(232,221,199,.3); }.restore-steps span { display: grid; gap: 5px; }.restore-steps strong { color: var(--green-deep); font-size: 11px; }.restore-steps small { color: var(--ink-muted); font-size: 9px; line-height: 1.5; }
.acknowledge { display: flex; gap: 10px; align-items: flex-start; margin-top: 18px; padding: 14px; color: #713d30; border: 1px solid rgba(185,88,63,.25); border-radius: 12px; background: rgba(185,88,63,.06); font-size: 12px; line-height: 1.65; cursor: pointer; }.acknowledge input { margin-top: 3px; accent-color: var(--cinnabar); }
.dialog-actions { display: flex; justify-content: flex-end; gap: 9px; margin-top: 20px; }.dialog-actions button { display: inline-flex; gap: 7px; align-items: center; padding: 11px 16px; border: 1px solid var(--line); border-radius: 11px; background: var(--paper-raised); cursor: pointer; }.dialog-actions .confirm-button { color: #fffdf7; border-color: var(--cinnabar); background: var(--cinnabar); }.dialog-actions button:disabled { opacity: .48; cursor: default; }
.spinning { animation: spin .85s linear infinite; }
@keyframes backdrop-in { from { opacity: 0; } }
@keyframes dialog-in { from { opacity: 0; transform: translateY(12px) scale(.975); } }
@keyframes spin { to { transform: rotate(360deg); } }
@media (max-width: 620px) { .restore-backdrop { padding: 12px; }.restore-dialog { max-height: calc(100vh - 24px); padding: 24px 18px; }.restore-steps { grid-template-columns: 1fr; }.dialog-actions { flex-direction: column-reverse; }.dialog-actions button { justify-content: center; } }
@media (prefers-reduced-motion: reduce) { .restore-backdrop,.restore-dialog,.spinning { animation: none; } }
</style>
