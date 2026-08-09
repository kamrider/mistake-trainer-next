<script setup lang="ts">
import { ArchiveRestore, ShieldCheck, TriangleAlert, X } from '@lucide/vue'
import { nextTick, onBeforeUnmount, onMounted, ref } from 'vue'
import { acquireDialogDocumentBoundary } from '../../../app/dialog-document-boundary'
import { trapDialogFocus } from '../../../app/dialog-focus'

const props = defineProps<{
  mode: 'import' | 'rollback'
  busy: boolean
  memberCount: number
  problemCount: number
}>()
const emit = defineEmits<{ cancel: [], confirm: [] }>()

const panel = ref<HTMLElement>()
const cancelButton = ref<HTMLButtonElement>()
const acknowledged = ref(false)
let releaseDialogBoundary: (() => void) | undefined

onMounted(async () => {
  if (panel.value) releaseDialogBoundary = acquireDialogDocumentBoundary(panel.value)
  await nextTick()
  cancelButton.value?.focus()
})

onBeforeUnmount(() => {
  releaseDialogBoundary?.()
})

function close() {
  if (!props.busy) emit('cancel')
}

function handleKeydown(event: KeyboardEvent) {
  if (event.key === 'Escape') {
    event.preventDefault()
    close()
    return
  }
  trapDialogFocus(event, panel.value)
}
</script>

<template>
  <div
    class="legacy-backdrop"
    @mousedown.self="close"
  >
    <section
      ref="panel"
      class="legacy-dialog"
      tabindex="-1"
      role="dialog"
      aria-modal="true"
      aria-labelledby="legacy-dialog-title"
      aria-describedby="legacy-dialog-description"
      @keydown="handleKeydown"
    >
      <button
        class="close-button"
        type="button"
        :aria-label="mode === 'import' ? '关闭导入确认' : '关闭撤销确认'"
        :disabled="busy"
        @click="close"
      >
        <X :size="18" />
      </button>
      <div
        class="dialog-mark"
        :class="{ warning: mode === 'rollback' }"
      >
        <ShieldCheck
          v-if="mode === 'import'"
          :size="26"
        />
        <ArchiveRestore
          v-else
          :size="26"
        />
      </div>
      <p class="eyebrow">
        {{ mode === 'import' ? '旧版迁移 · 最后确认' : '迁移记录 · 谨慎撤销' }}
      </p>
      <h2 id="legacy-dialog-title">
        {{ mode === 'import' ? `导入 ${problemCount} 道旧版错题？` : `撤销这次导入的 ${problemCount} 道题？` }}
      </h2>
      <p
        id="legacy-dialog-description"
        class="description"
      >
        {{ mode === 'import'
          ? `将新建 ${memberCount} 个独立学习档案，并复制、加密和校验图片。旧版目录始终保持只读。`
          : '只移除仍属于这次迁移的内容；已被其他题目复用或后来修改的数据会保留，并在结果中列出数量。' }}
      </p>

      <div class="safety-note">
        <ShieldCheck
          v-if="mode === 'import'"
          :size="18"
        />
        <TriangleAlert
          v-else
          :size="18"
        />
        <span>{{ mode === 'import' ? '任一步失败都会回滚，新题库不会留下半成品。' : '撤销不会读取、移动或删除原来的旧版目录。' }}</span>
      </div>

      <label class="acknowledge">
        <input
          v-model="acknowledged"
          type="checkbox"
          :disabled="busy"
        >
        <span>{{ mode === 'import' ? '我确认：导入只复制数据，不会修改旧目录' : '我确认：撤销本次导入，并保留已复用或修改的数据' }}</span>
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
          :class="{ danger: mode === 'rollback' }"
          type="button"
          :disabled="busy || !acknowledged"
          @click="emit('confirm')"
        >
          {{ busy ? '正在处理…' : mode === 'import' ? '确认并开始导入' : '确认撤销这次导入' }}
        </button>
      </div>
    </section>
  </div>
</template>

<style scoped>
.legacy-backdrop{position:fixed;z-index:90;inset:0;display:grid;padding:24px;place-items:center;background:rgba(24,34,30,.54);backdrop-filter:blur(7px);animation:backdrop-in var(--motion-standard) var(--ease-standard) both}.legacy-dialog{position:relative;width:min(590px,100%);max-height:calc(100vh - 48px);padding:30px;overflow:auto;border:1px solid rgba(33,51,45,.22);border-radius:22px;background:#fffdf7;box-shadow:0 28px 80px rgba(22,32,28,.28);animation:dialog-in var(--motion-page) var(--ease-standard) both}.close-button{position:absolute;top:18px;right:18px;display:grid;width:44px;height:44px;padding:0;place-items:center;border:0;border-radius:50%;background:rgba(33,51,45,.06);cursor:pointer}.dialog-mark{display:grid;width:52px;height:52px;margin-bottom:17px;place-items:center;color:#fffdf7;border-radius:17px 7px 17px 17px;background:var(--green-deep)}.dialog-mark.warning{background:var(--cinnabar)}.eyebrow{margin:0 0 7px;color:var(--cinnabar);font-size:12px;font-weight:850;letter-spacing:.15em}.legacy-dialog h2{margin:0;padding-right:45px;color:var(--green-deep);font-family:Georgia,'Microsoft YaHei',serif;font-size:27px}.description{margin:12px 0 0;color:var(--ink-muted);line-height:1.75}.safety-note{display:flex;gap:9px;align-items:flex-start;margin-top:19px;padding:14px;color:#557263;border-radius:12px;background:rgba(33,51,45,.06);font-size:12px;line-height:1.65}.safety-note svg{flex:0 0 auto}.acknowledge{display:flex;gap:10px;align-items:flex-start;margin-top:14px;padding:14px;color:#713d30;border:1px solid rgba(185,88,63,.25);border-radius:12px;background:rgba(185,88,63,.06);font-size:12px;line-height:1.65;cursor:pointer}.acknowledge input{margin-top:3px;accent-color:var(--cinnabar)}.dialog-actions{display:flex;justify-content:flex-end;gap:9px;margin-top:20px}.dialog-actions button{min-height:44px;padding:10px 16px;border:1px solid var(--line);border-radius:11px;background:var(--paper-raised);cursor:pointer}.dialog-actions .confirm-button{color:#fffdf7;border-color:var(--green-deep);background:var(--green-deep)}.dialog-actions .confirm-button.danger{border-color:var(--cinnabar);background:var(--cinnabar)}button:disabled{opacity:.48;cursor:default}@keyframes backdrop-in{from{opacity:0}}@keyframes dialog-in{from{opacity:0;transform:translateY(12px) scale(.975)}}@media(max-width:620px){.legacy-backdrop{padding:12px}.legacy-dialog{max-height:calc(100vh - 24px);padding:24px 18px}.dialog-actions{flex-direction:column-reverse}.dialog-actions button{width:100%}}@media(prefers-reduced-motion:reduce){.legacy-backdrop,.legacy-dialog{animation:none}}
</style>
