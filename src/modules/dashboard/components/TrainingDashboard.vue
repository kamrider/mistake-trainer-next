<script setup lang="ts">
import { ArrowRight, Clock3, Flame, TrendingUp } from '@lucide/vue'

const props = defineProps<{
  learnerName: string
  dueCount: number
  streakDays: number
  retention: number
}>()

defineEmits<{
  'start-review': []
  'open-inbox': []
}>()

const retentionLabel = `${Math.round(props.retention * 100)}%`
</script>

<template>
  <main
    class="training-dashboard"
    aria-labelledby="dashboard-title"
  >
    <section class="hero-panel">
      <div class="hero-copy">
        <p class="eyebrow">
          今日训练台
        </p>
        <h1 id="dashboard-title">
          {{ learnerName }}，今天从 {{ dueCount }} 道到期题开始。
        </h1>
        <p class="hero-note">
          不求一口气学完，只把需要重看的题认真看一遍。
        </p>
        <div class="hero-actions">
          <button
            class="primary-action"
            type="button"
            @click="$emit('start-review')"
          >
            开始复习 {{ dueCount }} 道
            <ArrowRight
              :size="18"
              aria-hidden="true"
            />
          </button>
          <button
            class="quiet-action"
            type="button"
            @click="$emit('open-inbox')"
          >
            整理采集箱
          </button>
        </div>
      </div>
      <div
        class="progress-seal"
        aria-label="今日到期题进度"
      >
        <span class="seal-number">{{ dueCount }}</span>
        <span class="seal-label">今日到期</span>
      </div>
    </section>

    <section
      class="metrics"
      aria-label="学习概览"
    >
      <article class="metric-card">
        <TrendingUp
          :size="21"
          aria-hidden="true"
        />
        <div><p>近 30 天记住率</p><strong>{{ retentionLabel }}</strong></div>
      </article>
      <article class="metric-card">
        <Flame
          :size="21"
          aria-hidden="true"
        />
        <div><p>训练节奏</p><strong>连续 {{ streakDays }} 天</strong></div>
      </article>
      <article class="metric-card">
        <Clock3
          :size="21"
          aria-hidden="true"
        />
        <div><p>预计用时</p><strong>约 {{ Math.max(4, Math.ceil(dueCount * 0.7)) }} 分钟</strong></div>
      </article>
    </section>

    <section
      class="day-plan"
      aria-labelledby="plan-title"
    >
      <div>
        <p class="eyebrow">
          今天的路线
        </p><h2 id="plan-title">
          先复习，再整理
        </h2>
      </div>
      <ol>
        <li><span>01</span><div><strong>到期复习</strong><small>{{ dueCount }} 道 · 先做最该看的</small></div></li>
        <li><span>02</span><div><strong>采集整理</strong><small>4 组待配对图片</small></div></li>
        <li><span>03</span><div><strong>轻量复盘</strong><small>查看本周节奏</small></div></li>
      </ol>
    </section>
  </main>
</template>

<style scoped>
.training-dashboard { display: grid; gap: 22px; max-width: 1180px; margin: 0 auto; padding: 38px 40px 56px; }
.hero-panel { position: relative; overflow: hidden; display: flex; align-items: center; justify-content: space-between; min-height: 310px; padding: 48px 54px; color: var(--white); border-radius: var(--radius-lg); background: linear-gradient(120deg, rgba(255,255,255,.035) 0 1px, transparent 1px 14px), var(--green-deep); box-shadow: var(--shadow-soft); }
.hero-panel::after { position: absolute; right: -90px; bottom: -160px; width: 390px; height: 390px; content: ''; border: 1px solid rgba(255,253,248,.12); border-radius: 50%; }
.hero-copy { position: relative; z-index: 1; max-width: 700px; }
.eyebrow { margin: 0 0 12px; color: var(--cinnabar); font-size: 12px; font-weight: 750; letter-spacing: .18em; text-transform: uppercase; }
.hero-panel .eyebrow { color: #e7a892; }
h1 { max-width: 700px; margin: 0; font-size: clamp(32px, 3.6vw, 48px); font-weight: 680; line-height: 1.18; letter-spacing: -.045em; text-wrap: balance; }
.hero-note { margin: 18px 0 0; color: rgba(255,253,248,.67); line-height: 1.75; }
.hero-actions { display: flex; gap: 12px; margin-top: 30px; }
.hero-actions button { min-height: 46px; padding: 0 20px; border-radius: 999px; cursor: pointer; transition: transform var(--motion-feedback) var(--ease-standard), background var(--motion-standard) var(--ease-standard); }
.hero-actions button:active { transform: translateY(1px) scale(.985); }
.primary-action { display: inline-flex; align-items: center; gap: 12px; color: var(--white); border: 1px solid var(--cinnabar); background: var(--cinnabar); font-weight: 700; }
.primary-action:hover { background: var(--cinnabar-dark); }
.quiet-action { color: rgba(255,253,248,.84); border: 1px solid rgba(255,253,248,.22); background: transparent; }
.quiet-action:hover { background: rgba(255,255,255,.08); }
.progress-seal { position: relative; z-index: 1; display: grid; width: 148px; height: 148px; flex: 0 0 auto; place-content: center; border: 1px solid rgba(255,253,248,.22); border-radius: 50%; text-align: center; }
.seal-number { font-family: Georgia, "Times New Roman", serif; font-size: 56px; line-height: 1; }
.seal-label { margin-top: 8px; color: rgba(255,253,248,.62); font-size: 13px; }
.metrics { display: grid; grid-template-columns: repeat(3, minmax(0,1fr)); gap: 14px; }
.metric-card { display: flex; gap: 14px; align-items: center; min-height: 104px; padding: 22px; border: 1px solid var(--line); border-radius: var(--radius-md); background: rgba(255,253,247,.7); }
.metric-card > svg { color: var(--cinnabar); }
.metric-card p, .metric-card strong { margin: 0; }
.metric-card p { margin-bottom: 7px; color: var(--ink-muted); font-size: 13px; }
.metric-card strong { font-family: Georgia, "Microsoft YaHei UI", serif; font-size: 23px; font-weight: 620; }
.day-plan { display: grid; grid-template-columns: .65fr 1fr; gap: 40px; padding: 30px 34px; border-top: 1px solid var(--line); }
.day-plan h2 { margin: 0; font-size: 26px; }
.day-plan ol { display: grid; gap: 10px; margin: 0; padding: 0; list-style: none; }
.day-plan li { display: grid; grid-template-columns: 36px 1fr; gap: 12px; align-items: center; padding: 10px 0; }
.day-plan li > span { color: var(--cinnabar); font-family: Georgia, serif; font-size: 13px; }
.day-plan strong, .day-plan small { display: block; }
.day-plan small { margin-top: 4px; color: var(--ink-muted); }
@media (max-width: 800px) { .training-dashboard { padding: 24px 18px 40px; } .hero-panel { align-items: flex-start; padding: 34px 28px; } .progress-seal { display: none; } .metrics, .day-plan { grid-template-columns: 1fr; } }
@media (max-width: 520px) { .hero-actions { align-items: stretch; flex-direction: column; } .metrics { gap: 9px; } }
</style>
