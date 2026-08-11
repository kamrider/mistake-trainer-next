<script setup lang="ts">
import { CheckCircle2, ScanLine, TriangleAlert } from '@lucide/vue'
import { computed } from 'vue'
import type {
  CaptureQualityIssueCode,
  CaptureQualityReport,
} from '../../../shared/api/bindings'

const props = defineProps<{
  report: CaptureQualityReport | undefined
  busy: boolean
  errorMessage?: string | undefined
}>()

const emit = defineEmits<{
  dismiss: []
  reselect: []
  crop: []
  retry: []
}>()

const issueCopy: Record<CaptureQualityIssueCode, string> = {
  blurry: '画面可能不够清晰，题目细节容易看不清。',
  too_dark: '画面整体偏暗，建议在更亮的环境重新拍摄。',
  too_bright: '画面可能过曝，浅色笔迹容易丢失。',
  low_contrast: '文字与纸面反差较小，阅读可能费力。',
  possible_edge_cut: '有内容贴近图片边缘，请确认题目没有被截断。',
  skewed: '纸面有轻微倾斜，可以在裁剪修正中校正。',
}

const messages = computed(() => props.report?.issues.map(issue => issueCopy[issue]) ?? [])
const rotationCopy = computed(() => {
  const degrees = props.report?.suggestedRotationDegrees ?? 0
  return Math.abs(degrees) >= 1.5 ? `建议旋转 ${degrees > 0 ? '+' : ''}${degrees.toFixed(1)}°` : ''
})
</script>

<template>
  <section
    class="quality-panel"
    :class="{ 'has-issues': messages.length || errorMessage }"
    aria-live="polite"
  >
    <div
      v-if="busy"
      class="quality-state"
    >
      <ScanLine
        :size="18"
        aria-hidden="true"
      />
      <div><strong>正在本机检查图片质量</strong><span>只分析缩小后的画面，不会上传图片。</span></div>
    </div>
    <template v-else-if="errorMessage">
      <div class="quality-state">
        <TriangleAlert
          :size="18"
          aria-hidden="true"
        />
        <div><strong>暂时无法检查图片质量</strong><span>{{ errorMessage }}</span></div>
      </div>
      <button
        type="button"
        class="quality-link"
        @click="emit('retry')"
      >
        重新检查
      </button>
    </template>
    <template v-else-if="messages.length">
      <div class="quality-state">
        <TriangleAlert
          :size="18"
          aria-hidden="true"
        />
        <div>
          <strong>建议检查这张图片</strong>
          <ul>
            <li
              v-for="message in messages"
              :key="message"
            >
              {{ message }}
            </li>
          </ul>
          <span v-if="rotationCopy">{{ rotationCopy }}</span>
        </div>
      </div>
      <div class="quality-actions">
        <button
          type="button"
          @click="emit('dismiss')"
        >
          继续使用
        </button>
        <button
          type="button"
          @click="emit('reselect')"
        >
          重新选择
        </button>
        <button
          type="button"
          class="primary"
          @click="emit('crop')"
        >
          打开裁剪修正
        </button>
      </div>
    </template>
    <div
      v-else
      class="quality-state is-clear"
    >
      <CheckCircle2
        :size="18"
        aria-hidden="true"
      />
      <div><strong>图片质量看起来良好</strong><span>仍建议确认题面和答案边缘完整。</span></div>
    </div>
  </section>
</template>

<style scoped>
.quality-panel{display:grid;gap:10px;padding:11px;border:1px solid rgba(33,51,45,.13);border-radius:12px;background:rgba(225,235,229,.48)}
.quality-panel.has-issues{border-color:rgba(185,88,63,.25);background:rgba(247,225,216,.48)}
.quality-state{display:flex;gap:9px;align-items:flex-start;color:var(--cinnabar)}
.quality-state.is-clear{color:var(--green-deep)}
.quality-state div{display:grid;gap:3px;min-width:0}
.quality-state strong{font-size:12px}.quality-state span,.quality-state li{color:var(--ink-muted);font-size:12px;line-height:1.55}
.quality-state ul{display:grid;gap:2px;margin:2px 0 0;padding-left:18px}
.quality-actions{display:flex;flex-wrap:wrap;gap:7px}
.quality-actions button,.quality-link{min-height:40px;padding:0 11px;border:1px solid rgba(33,51,45,.16);border-radius:999px;background:var(--paper);color:var(--green-deep);font-size:12px;font-weight:750;cursor:pointer}
.quality-actions .primary{color:var(--paper);border-color:var(--cinnabar);background:var(--cinnabar)}
.quality-link{justify-self:start}
</style>
