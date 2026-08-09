<script setup lang="ts">
import { CheckCircle2, Cpu, ShieldCheck, Sparkles, Square } from '@lucide/vue'
import { computed, nextTick, ref } from 'vue'
import type {
  CaptureBatchState,
  CaptureRecognitionJob,
  OcrRecognitionFeatureStatus,
} from '../../../shared/api/bindings'
import { recognitionPrimaryAction } from '../domain/recognitionFlow'

const props = defineProps<{
  batchState: CaptureBatchState
  unassignedCount: number
  feature: OcrRecognitionFeatureStatus
  job?: CaptureRecognitionJob | undefined
  busy?: boolean | undefined
}>()

const emit = defineEmits<{
  start: []
  cancel: [jobId: string]
  resume: [jobId: string]
  openSetup: []
}>()

const preflightOpen = ref(false)
const primaryAction = ref<HTMLButtonElement>()
const action = computed(() => recognitionPrimaryAction({
  batchState: props.batchState,
  unassignedCount: props.unassignedCount,
  featureState: props.feature.state,
  ...(props.job ? { activeJobState: props.job.state } : {}),
}))
const progress = computed(() => props.job && ['queued', 'running'].includes(props.job.state))
const enhanced = computed(() => props.feature.requiredComponentId === 'ppocrv6_small')

function start() {
  preflightOpen.value = false
  emit('start')
}

async function focusPrimaryAction() {
  await nextTick()
  primaryAction.value?.focus()
}

defineExpose({ focusPrimaryAction })
</script>

<template>
  <section
    v-if="action !== 'hidden'"
    class="recognition-entry"
    :class="`is-${action}`"
    aria-labelledby="recognition-title"
  >
    <template v-if="progress && job">
      <div
        class="recognition-progress-copy"
      >
        <span class="recognition-icon"><Cpu :size="20" /></span>
        <div>
          <h2 id="recognition-title">
            正在切图 {{ job.processedItems }} / {{ job.totalItems }}
          </h2>
          <p>{{ enhanced ? '正在本机定位连续题号；OCR 文字不会保存，切图结果也不会自动应用。' : '正在分析版面；你可以继续整理题卡，切图结果不会自动应用。' }}</p>
        </div>
      </div>
      <progress
        :value="job.processedItems"
        :max="job.totalItems"
        :aria-label="`智能切图进度 ${job.processedItems} / ${job.totalItems}`"
      />
      <p
        class="sr-only"
        role="status"
        aria-live="polite"
        aria-atomic="true"
      >
        已分析 {{ job.processedItems }} / {{ job.totalItems }}
      </p>
      <button
        type="button"
        class="secondary-action"
        :disabled="busy"
        aria-label="停止识别"
        @click="emit('cancel', job.id)"
      >
        <Square :size="14" /> 停止
      </button>
    </template>

    <template v-else-if="action === 'resume' && job">
      <div class="recognition-copy">
        <span class="recognition-icon"><CheckCircle2 :size="20" /></span>
        <div>
          <p class="eyebrow">
            {{ enhanced ? '智能切图 · 题号增强' : '智能切图 · 基础预切' }}
          </p>
          <h2 id="recognition-title">
            先确认，再放入素材牌库
          </h2>
          <p>一页多题会显示为多个切图框；确认后才生成素材图片，原图和题卡保持不变。</p>
        </div>
      </div>
      <button
        ref="primaryAction"
        type="button"
        class="primary-action"
        @click="emit('resume', job.id)"
      >
        查看切图建议
      </button>
    </template>

    <template v-else-if="action === 'explain_gate'">
      <div class="recognition-copy">
        <span class="recognition-icon"><ShieldCheck :size="20" /></span>
        <div>
          <p class="eyebrow">
            智能切图
          </p>
          <h2 id="recognition-title">
            准确率验证完成后开放
          </h2>
          <p>当前不会运行识别，也不会改动原图；你可以继续用顺序模板或手工拖拽。</p>
        </div>
      </div>
      <a
        class="secondary-link"
        href="#capture-layout-templates"
      >使用顺序模板</a>
    </template>

    <template v-else-if="action === 'explain_runtime'">
      <div class="recognition-copy">
        <span class="recognition-icon"><ShieldCheck :size="20" /></span>
        <div>
          <p class="eyebrow">
            智能切图
          </p>
          <h2 id="recognition-title">
            本机识别组件还没准备好
          </h2>
          <p>{{ feature.detail }}</p>
        </div>
      </div>
      <a
        class="secondary-link"
        href="#capture-layout-templates"
      >使用顺序模板</a>
    </template>

    <template v-else-if="action === 'open_setup'">
      <div class="recognition-copy">
        <span class="recognition-icon"><Cpu :size="20" /></span>
        <div>
          <p class="eyebrow">
            智能切图
          </p>
          <h2 id="recognition-title">
            本地切图组件暂不可用
          </h2>
          <p>{{ feature.detail }} 这不会影响手工裁剪和拖拽整理。</p>
        </div>
      </div>
      <a
        class="secondary-link"
        href="#capture-layout-templates"
      >使用顺序模板</a>
    </template>

    <template v-else>
      <div class="recognition-copy">
        <span class="recognition-icon"><Sparkles :size="20" /></span>
        <div>
          <p class="eyebrow">
            {{ enhanced ? '智能切图 · 题号增强' : '智能切图 · 基础预切' }}
          </p>
          <h2 id="recognition-title">
            把一页多题拆成素材图片
          </h2>
          <p>{{ enhanced ? '在本机读取题号锚点来切分，不保存 OCR 正文；原图和现有题卡不会改变。' : '按版面给出预切建议；原图和现有题卡不会改变。' }}</p>
        </div>
      </div>
      <button
        v-if="!preflightOpen"
        type="button"
        class="primary-action"
        :disabled="busy"
        @click="preflightOpen = true"
      >
        智能切分 {{ unassignedCount }} 张素材
      </button>
      <div
        v-else
        class="recognition-preflight"
      >
        <strong>将切分 {{ unassignedCount }} 张未分配素材</strong>
        <ul>
          <li>只在这台电脑运行</li>
          <li>{{ enhanced ? '只使用已安装的本地题号模型，不发起网络请求' : '基础预切不使用 OCR，也不发起下载' }}</li>
          <li>不会新建或改动题卡</li>
          <li>原图始终保留</li>
          <li>切图继承原素材的题图 / 答案角色</li>
          <li>一页多题会建议拆成多张素材图片</li>
          <li>低可信页面保持整页并提示手工整理</li>
          <li>确认后的切图全部进入素材牌库</li>
        </ul>
        <div class="recognition-actions">
          <button
            type="button"
            class="primary-action"
            :disabled="busy"
            @click="start"
          >
            开始切图
          </button>
          <button
            type="button"
            class="text-action"
            @click="preflightOpen = false"
          >
            暂不用
          </button>
        </div>
      </div>
    </template>
  </section>
</template>

<style scoped>
.recognition-entry {
  display: grid;
  grid-template-columns: minmax(0, 1fr) auto;
  gap: 16px 22px;
  align-items: center;
  margin-top: 25px;
  padding: 20px;
  border: 1px solid rgba(72, 99, 88, .22);
  border-radius: 18px 6px 18px 18px;
  background:
    radial-gradient(circle at 92% 8%, rgba(198, 218, 207, .48), transparent 36%),
    rgba(252, 250, 243, .9);
  box-shadow: 0 12px 34px rgba(36, 55, 47, .07);
}

.recognition-copy,
.recognition-progress-copy {
  display: flex;
  gap: 13px;
  align-items: flex-start;
}

.recognition-icon {
  display: grid;
  flex: 0 0 38px;
  width: 38px;
  height: 38px;
  place-items: center;
  color: #315f50;
  border-radius: 12px;
  background: rgba(202, 222, 211, .62);
}

.eyebrow {
  color: #486b5d !important;
  font-size: 12px !important;
  font-weight: 850;
  letter-spacing: .1em;
  text-transform: uppercase;
}

h2,
p {
  margin: 0;
}

h2 {
  margin-top: 2px;
  color: var(--ink);
  font-size: 17px;
}

p {
  margin-top: 4px;
  color: var(--ink-muted);
  font-size: 12px;
}

.primary-action,
.secondary-action,
.text-action {
  min-height: 44px;
  padding: 0 16px;
  border-radius: 999px;
  font-weight: 760;
  cursor: pointer;
}

.primary-action {
  color: var(--paper);
  border: 0;
  background: var(--green-deep);
}

.secondary-action {
  display: inline-flex;
  gap: 7px;
  align-items: center;
  color: var(--ink);
  border: 1px solid rgba(33, 51, 45, .2);
  background: rgba(255, 253, 247, .72);
}

.text-action {
  color: var(--ink-muted);
  border: 0;
  background: transparent;
}

button:disabled {
  opacity: .55;
  cursor: wait;
}

.secondary-link {
  display: inline-flex;
  align-items: center;
  min-height: 44px;
  color: #315f50;
  font-size: 12px;
  font-weight: 780;
  text-underline-offset: 3px;
}

.recognition-actions {
  display: flex;
  gap: 10px;
  align-items: center;
}

.recognition-preflight {
  grid-column: 1 / -1;
  padding: 16px;
  border: 1px solid rgba(72, 99, 88, .16);
  border-radius: 14px;
  background: rgba(255, 253, 247, .72);
}

.recognition-preflight ul {
  display: grid;
  grid-template-columns: repeat(2, minmax(0, 1fr));
  gap: 7px 18px;
  margin: 10px 0 14px;
  padding-left: 20px;
  color: var(--ink-muted);
  font-size: 12px;
}

progress {
  width: min(320px, 32vw);
  height: 8px;
  accent-color: #4f806e;
}

.sr-only {
  position: absolute;
  width: 1px;
  height: 1px;
  padding: 0;
  overflow: hidden;
  clip: rect(0, 0, 0, 0);
  white-space: nowrap;
  border: 0;
}

@media (max-width: 760px) {
  .recognition-entry {
    grid-template-columns: 1fr;
  }

  .recognition-actions {
    align-items: stretch;
    flex-direction: column;
  }

  .recognition-preflight ul {
    grid-template-columns: 1fr;
  }

  progress {
    width: 100%;
  }
}
</style>
