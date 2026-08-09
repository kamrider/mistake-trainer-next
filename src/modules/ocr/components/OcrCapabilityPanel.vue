<script setup lang="ts">
import { CheckCircle2, Download, Layers3, LockKeyhole, ScanText, Trash2 } from '@lucide/vue'
import { computed } from 'vue'
import type { OcrCapabilityStatus, OcrComponentId } from '../../../shared/api/bindings'

const props = defineProps<{
  status?: OcrCapabilityStatus | undefined
  busy?: boolean | undefined
  message?: string | undefined
}>()

const emit = defineEmits<{
  install: [componentId: OcrComponentId]
  remove: [componentId: OcrComponentId]
}>()

const small = computed(() =>
  props.status?.components.find(component => component.id === 'ppocrv6_small'))
const enhanced = computed(() =>
  props.status?.automaticRecognitionEnabled === true && small.value?.state === 'installed')

function megabytes(bytes: number | null) {
  return `${Math.max(1, Math.round((bytes ?? 0) / 1024 / 1024))} MB`
}
</script>

<template>
  <section
    class="intelligence-panel"
    role="region"
    aria-labelledby="intelligence-title"
  >
    <header>
      <span class="panel-icon"><Layers3 :size="21" /></span>
      <div>
        <p>LOCAL · CLEAR BOUNDARIES</p>
        <h2 id="intelligence-title">
          智能功能模式
        </h2>
        <span>切题、答案匹配和内容理解是三层独立能力；当前只开放可复核、可撤销的本地切题。</span>
      </div>
    </header>

    <div class="mode-grid">
      <article aria-label="智能切图（已开放）">
        <div class="mode-heading">
          <span class="mode-icon active"><CheckCircle2 :size="19" /></span>
          <div>
            <p>模式一</p>
            <h3>智能切图</h3>
          </div>
          <strong class="mode-badge active">{{ enhanced ? '题号增强已启用' : '基础版已开放' }}</strong>
        </div>
        <p class="mode-summary">
          {{ enhanced
            ? '优先用本地 small 模型定位连续题号；识别文字不保存，低置信度页面保留整页。'
            : '先用本地版面分析给出预切建议；复杂试卷建议启用题号定位增强。' }}
        </p>
        <ul>
          <li>{{ enhanced ? '只读取题号锚点，不保存 OCR 正文' : '基础预切无需模型，也不会联网' }}</li>
          <li>small 是切题主力；medium 更大但不代表切题更准</li>
          <li>切图确认后只进入素材牌库</li>
          <li>原图、现有题卡和加密存储保持不变</li>
        </ul>
        <div
          v-if="small"
          class="enhancement-control"
          :aria-busy="busy"
        >
          <span>
            <strong>{{ small.displayName }}</strong>
            <small>{{ small.statusDetail }}</small>
            <small>{{ megabytes(small.downloadBytes) }} 下载 · {{ small.sourceLabel }} · {{ small.licenseLabel }}</small>
          </span>
          <button
            v-if="small.state !== 'installed'"
            type="button"
            :disabled="busy || !small.installAllowed"
            @click="emit('install', small.id)"
          >
            <Download :size="15" />
            {{ busy ? '处理中…' : small.state === 'corrupt' ? '重新安装' : '启用更准切题' }}
          </button>
          <button
            v-else
            type="button"
            class="remove-action"
            :disabled="busy"
            @click="emit('remove', small.id)"
          >
            <Trash2 :size="15" />移除模型
          </button>
        </div>
        <p
          v-if="message"
          class="component-message"
          role="status"
          aria-live="polite"
        >
          {{ message }}
        </p>
      </article>

      <article
        class="future-mode"
        aria-label="全自动识题（未开放）"
      >
        <div class="mode-heading">
          <span class="mode-icon"><ScanText :size="19" /></span>
          <div>
            <p>模式二</p>
            <h3>全自动识题</h3>
          </div>
          <strong class="mode-badge">未开放</strong>
        </div>
        <p class="mode-summary">
          后续识别科目、题干与答案，完成题答匹配、自动组卡和一键导出。
        </p>
        <ul>
          <li>届时才会评估 OCR 与内容理解模型</li>
          <li>模型档位会结合本机性能重新设计</li>
          <li>当前版本不会运行或发起下载</li>
          <li>未经过真实试卷验收前不会自动入库</li>
        </ul>
      </article>
    </div>

    <p class="privacy-note">
      <LockKeyhole :size="16" />
      所有切题都在本机完成并先进入复核；没有任何结果会绕过确认直接创建正式题目。
    </p>
  </section>
</template>

<style scoped>
.intelligence-panel {
  margin-bottom: 16px;
  padding: 24px 26px;
  border: 1px solid rgba(33, 51, 45, .22);
  border-radius: 17px;
  background: linear-gradient(145deg, rgba(255, 253, 247, .96), rgba(220, 228, 220, .36));
  box-shadow: 0 16px 48px rgba(34, 48, 43, .05);
}

header,
.mode-heading,
.privacy-note {
  display: flex;
  align-items: flex-start;
}

header {
  gap: 13px;
  margin-bottom: 18px;
}

.panel-icon,
.mode-icon {
  display: grid;
  flex: 0 0 auto;
  place-items: center;
}

.panel-icon {
  width: 42px;
  height: 42px;
  color: var(--green-deep);
  border-radius: 12px;
  background: var(--green-soft);
}

header p,
.mode-heading p {
  margin: 0 0 5px;
  color: var(--cinnabar);
  font-size: 12px;
  font-weight: 820;
  letter-spacing: .13em;
}

header h2 {
  margin: 0;
  color: var(--green-deep);
  font-family: var(--font-serif);
  font-size: 22px;
}

header span:last-child {
  display: block;
  margin-top: 6px;
  color: var(--ink-muted);
  font-size: 12px;
  line-height: 1.6;
}

.mode-grid {
  display: grid;
  grid-template-columns: repeat(2, minmax(0, 1fr));
  gap: 12px;
}

article {
  padding: 17px;
  border: 1px solid rgba(69, 99, 84, .32);
  border-radius: 14px;
  background: rgba(255, 253, 247, .78);
  box-shadow: inset 3px 0 0 #557263;
}

article.future-mode {
  border-style: dashed;
  border-color: rgba(33, 51, 45, .18);
  background: rgba(244, 241, 232, .58);
  box-shadow: none;
}

.mode-heading {
  gap: 10px;
  align-items: center;
}

.mode-icon {
  width: 34px;
  height: 34px;
  color: var(--ink-muted);
  border-radius: 10px;
  background: rgba(33, 51, 45, .07);
}

.mode-icon.active {
  color: var(--green-deep);
  background: var(--green-soft);
}

.mode-heading div {
  min-width: 0;
}

.mode-heading p {
  margin-bottom: 1px;
  color: var(--ink-muted);
}

.mode-heading h3 {
  margin: 0;
  color: var(--green-deep);
  font-size: 15px;
}

.mode-badge {
  margin-left: auto;
  padding: 5px 8px;
  color: var(--ink-muted);
  border: 1px solid rgba(33, 51, 45, .16);
  border-radius: 999px;
  font-size: 12px;
  white-space: nowrap;
}

.mode-badge.active {
  color: #fff;
  border-color: #557263;
  background: #557263;
}

.mode-summary {
  margin: 13px 0 9px;
  color: var(--ink);
  font-size: 12px;
  line-height: 1.6;
}

ul {
  display: grid;
  gap: 5px;
  margin: 0;
  padding-left: 18px;
  color: var(--ink-muted);
  font-size: 12px;
  line-height: 1.5;
}

.enhancement-control {
  display: flex;
  gap: 12px;
  align-items: center;
  justify-content: space-between;
  margin-top: 14px;
  padding-top: 13px;
  border-top: 1px solid rgba(69, 99, 84, .18);
}

.enhancement-control span {
  display: grid;
  gap: 3px;
  min-width: 0;
}

.enhancement-control strong {
  color: var(--green-deep);
  font-size: 12px;
}

.enhancement-control small {
  color: var(--ink-muted);
  font-size: 12px;
  line-height: 1.45;
}

.enhancement-control button {
  display: inline-flex;
  flex: 0 0 auto;
  gap: 6px;
  align-items: center;
  min-height: 44px;
  padding: 0 13px;
  color: #fff;
  border: 0;
  border-radius: 999px;
  background: var(--green-deep);
  font-size: 12px;
  font-weight: 760;
  cursor: pointer;
}

.enhancement-control button.remove-action {
  color: var(--ink-muted);
  border: 1px solid rgba(33, 51, 45, .16);
  background: transparent;
}

.enhancement-control button:disabled {
  opacity: .55;
  cursor: not-allowed;
}

.component-message {
  margin: 10px 0 0;
  color: var(--green-deep);
  font-size: 12px;
  line-height: 1.5;
}

.privacy-note {
  gap: 8px;
  align-items: center;
  margin: 14px 0 0;
  padding: 11px 13px;
  color: var(--ink-muted);
  border-radius: 11px;
  background: rgba(33, 51, 45, .055);
  font-size: 12px;
}

@media (max-width: 760px) {
  .intelligence-panel {
    padding: 20px 18px;
  }

  .mode-grid {
    grid-template-columns: 1fr;
  }
}
</style>
