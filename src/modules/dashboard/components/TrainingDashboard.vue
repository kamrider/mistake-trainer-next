<script setup lang="ts">
import { computed } from 'vue'
import { ArrowRight, BookOpen, Check, Clock3, Flame, Inbox, RotateCcw, TrendingUp } from '@lucide/vue'
import type { DashboardOverview } from '@/shared/api/bindings'
import { useAnimatedNumber } from '@/shared/composables/useAnimatedNumber'

const props = defineProps<{
  overview: DashboardOverview | null
  loading?: boolean
  errorMessage?: string
}>()

const emit = defineEmits<{
  'start-review': []
  'open-quick': []
  'open-inbox': []
  'open-library': []
  'open-report': []
  retry: []
}>()

const hasProblems = computed(() => (props.overview?.activeProblemCount ?? 0) > 0)
const hasDueProblems = computed(() => (props.overview?.dueProblemCount ?? 0) > 0)
const isAllClear = computed(() => hasProblems.value && !hasDueProblems.value)
const dueCount = computed(() => props.overview?.dueProblemCount ?? 0)
const reviewedToday = computed(() => props.overview?.reviewedTodayCount ?? 0)
const streakDays = computed(() => props.overview?.currentStreakDays ?? 0)
const retentionPercent = computed(() => Math.round((props.overview?.rememberedRate30Days ?? 0) * 100))
const animatedDueCount = useAnimatedNumber(dueCount)
const animatedReviewedToday = useAnimatedNumber(reviewedToday)
const animatedStreakDays = useAnimatedNumber(streakDays)
const animatedRetention = useAnimatedNumber(retentionPercent)

const headline = computed(() => {
  if (!props.overview) return ''
  if (!hasProblems.value) return `${props.overview.profileName}，先收下第一道值得重看的题。`
  if (hasDueProblems.value) return `${props.overview.profileName}，今天从 ${props.overview.dueProblemCount} 道到期题开始。`
  return `${props.overview.profileName}，今天的到期题已经清空。`
})

const primaryAction = computed(() => {
  if (!hasProblems.value) return '录入第一道错题'
  if (hasDueProblems.value) return `开始复习 ${props.overview?.dueProblemCount ?? 0} 道`
  return '查看题库'
})

const captureSummary = computed(() => {
  const batchCount = props.overview?.pendingCaptureBatchCount ?? 0
  const itemCount = props.overview?.pendingCaptureItemCount ?? 0
  if (batchCount === 0) return '采集箱已整理完毕'
  return `${batchCount} 个批次 · ${itemCount} 张图片待整理`
})

function activatePrimaryAction() {
  if (!hasProblems.value) emit('open-inbox')
  else if (hasDueProblems.value) emit('start-review')
  else emit('open-library')
}

function openFirstRoute() {
  if (hasDueProblems.value) emit('start-review')
  else emit('open-library')
}
</script>

<template>
  <main
    class="training-dashboard"
    aria-labelledby="dashboard-title"
  >
    <section
      v-if="loading"
      class="dashboard-state loading-state"
      aria-label="正在读取训练台"
    >
      <div class="skeleton skeleton-kicker" />
      <div class="skeleton skeleton-title" />
      <div class="skeleton skeleton-copy" />
      <div class="skeleton skeleton-button" />
      <span class="sr-only">正在读取训练台</span>
    </section>

    <section
      v-else-if="errorMessage"
      class="dashboard-state error-state"
      role="alert"
    >
      <span class="state-icon"><RotateCcw
        :size="24"
        aria-hidden="true"
      /></span>
      <p class="eyebrow">
        训练台暂时没有打开
      </p>
      <h1 id="dashboard-title">
        真实数据没有读到，我们没有用旧数字替代。
      </h1>
      <p>{{ errorMessage }}</p>
      <button
        type="button"
        class="primary-action"
        @click="$emit('retry')"
      >
        重新读取
        <RotateCcw
          :size="17"
          aria-hidden="true"
        />
      </button>
    </section>

    <div
      v-else-if="overview"
      class="dashboard-content"
    >
      <section
        class="hero-panel"
        :class="{ 'is-all-clear': isAllClear, 'is-empty': !hasProblems }"
      >
        <div class="hero-copy">
          <p class="eyebrow">
            今日训练台
          </p>
          <h1 id="dashboard-title">
            {{ headline }}
          </h1>
          <p class="hero-note">
            <template v-if="!hasProblems">
              从一道真实错题开始，后面的复习节奏会自然长出来。
            </template>
            <template v-else-if="hasDueProblems">
              不求一口气学完，只把需要重看的题认真看一遍。
            </template>
            <template v-else>
              今天没有到期任务，可以整理新题，也可以看看学习节奏。
            </template>
          </p>
          <div class="hero-actions">
            <button
              class="primary-action"
              type="button"
              @click="activatePrimaryAction"
            >
              {{ primaryAction }}
              <ArrowRight
                class="action-arrow"
                :size="18"
                aria-hidden="true"
              />
            </button>
            <button
              v-if="hasProblems"
              class="secondary-action"
              type="button"
              @click="$emit('open-quick')"
            >
              快速训练
              <Clock3
                :size="17"
                aria-hidden="true"
              />
            </button>
          </div>
        </div>
        <div
          class="progress-seal"
          :class="{ complete: isAllClear }"
          aria-label="今日到期题进度"
        >
          <Check
            v-if="isAllClear"
            :size="46"
            stroke-width="1.7"
            aria-hidden="true"
          />
          <span
            v-else
            class="seal-number"
          >{{ animatedDueCount }}</span>
          <span class="seal-label">{{ isAllClear ? '今日完成' : hasProblems ? '今日到期' : '题库待启' }}</span>
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
          <div>
            <p>近 30 天记住率</p>
            <strong v-if="overview.rememberedRate30Days !== null">{{ animatedRetention }}%</strong>
            <strong
              v-else
              class="text-value"
            >暂无复习数据</strong>
          </div>
        </article>
        <article class="metric-card">
          <Flame
            :size="21"
            aria-hidden="true"
          />
          <div>
            <p>训练节奏</p>
            <strong v-if="overview.currentStreakDays > 0">连续 {{ animatedStreakDays }} 天</strong>
            <strong
              v-else
              class="text-value"
            >从今天开始</strong>
          </div>
        </article>
        <article class="metric-card">
          <Clock3
            :size="21"
            aria-hidden="true"
          />
          <div><p>今日已复习</p><strong>{{ animatedReviewedToday }} 道</strong></div>
        </article>
      </section>

      <section
        class="day-plan"
        aria-labelledby="plan-title"
      >
        <div>
          <p class="eyebrow">
            接下来
          </p>
          <h2 id="plan-title">
            按真实进度继续
          </h2>
        </div>
        <div class="route-list">
          <button
            type="button"
            @click="openFirstRoute"
          >
            <span>01</span><BookOpen
              :size="19"
              aria-hidden="true"
            />
            <div><strong>{{ hasDueProblems ? '到期复习' : '浏览题库' }}</strong><small>{{ hasDueProblems ? `${overview.dueProblemCount} 道 · 先做最该看的` : `${overview.activeProblemCount} 道活动错题` }}</small></div>
            <ArrowRight
              :size="17"
              aria-hidden="true"
            />
          </button>
          <button
            type="button"
            @click="$emit('open-inbox')"
          >
            <span>02</span><Inbox
              :size="19"
              aria-hidden="true"
            />
            <div><strong>采集整理</strong><small>{{ captureSummary }}</small></div>
            <ArrowRight
              :size="17"
              aria-hidden="true"
            />
          </button>
          <button
            type="button"
            @click="$emit('open-report')"
          >
            <span>03</span><TrendingUp
              :size="19"
              aria-hidden="true"
            />
            <div><strong>学习复盘</strong><small>查看近期记住率与训练节奏</small></div>
            <ArrowRight
              :size="17"
              aria-hidden="true"
            />
          </button>
        </div>
      </section>
    </div>
  </main>
</template>

<style scoped>
.training-dashboard { max-width: 1180px; min-height: 640px; margin: 0 auto; padding: 38px 40px 56px; }
.dashboard-content { display: grid; gap: 22px; animation: dashboard-reveal var(--motion-page) var(--ease-standard) both; }
.dashboard-state { min-height: 420px; padding: 56px; border: 1px solid var(--line); border-radius: var(--radius-lg); background: rgba(255,253,247,.72); }
.loading-state { display: grid; align-content: center; gap: 18px; }
.skeleton { overflow: hidden; position: relative; border-radius: 10px; background: var(--sand); }
.skeleton::after { position: absolute; inset: 0; content: ''; background: linear-gradient(90deg, transparent, rgba(255,255,255,.65), transparent); transform: translateX(-100%); animation: skeleton-sweep 1.2s ease-in-out infinite; }
.skeleton-kicker { width: 110px; height: 14px; }.skeleton-title { width: min(680px, 85%); height: 52px; }.skeleton-copy { width: min(520px, 70%); height: 20px; }.skeleton-button { width: 170px; height: 46px; margin-top: 10px; border-radius: 999px; }
.error-state { display: grid; justify-items: start; align-content: center; max-width: 760px; gap: 14px; }
.error-state h1 { color: var(--ink); }.error-state > p:not(.eyebrow) { margin: 0 0 10px; color: var(--ink-muted); }.state-icon { display: grid; width: 48px; height: 48px; place-items: center; color: var(--cinnabar); border: 1px solid color-mix(in srgb, var(--cinnabar) 35%, transparent); border-radius: 50%; }
.hero-panel { position: relative; overflow: hidden; display: flex; align-items: center; justify-content: space-between; min-height: 310px; padding: 48px 54px; color: var(--white); border-radius: var(--radius-lg); background: linear-gradient(120deg, rgba(255,255,255,.035) 0 1px, transparent 1px 14px), var(--green-deep); box-shadow: var(--shadow-soft); }
.hero-panel::after { position: absolute; right: -90px; bottom: -160px; width: 390px; height: 390px; content: ''; border: 1px solid rgba(255,253,248,.12); border-radius: 50%; }
.hero-panel.is-all-clear { background: linear-gradient(120deg, rgba(255,255,255,.045) 0 1px, transparent 1px 14px), #2a453b; }.hero-panel.is-empty { background: linear-gradient(120deg, rgba(255,255,255,.035) 0 1px, transparent 1px 14px), #2e3b36; }
.hero-copy { position: relative; z-index: 1; max-width: 700px; }.eyebrow { margin: 0 0 12px; color: var(--cinnabar); font-size: 12px; font-weight: 750; letter-spacing: .18em; text-transform: uppercase; }.hero-panel .eyebrow { color: #e7a892; }
h1 { max-width: 700px; margin: 0; font-size: clamp(32px, 3.6vw, 48px); font-weight: 680; line-height: 1.18; letter-spacing: -.045em; text-wrap: balance; }.hero-note { margin: 18px 0 0; color: rgba(255,253,248,.67); line-height: 1.75; }
.hero-actions { display: flex; gap: 12px; margin-top: 30px; }.hero-actions button,.error-state button { min-height: 46px; padding: 0 20px; border-radius: 999px; cursor: pointer; transition: transform var(--motion-feedback) var(--ease-standard), background var(--motion-standard) var(--ease-standard); }.hero-actions button:active,.error-state button:active { transform: translateY(1px) scale(.985); }
.primary-action { display: inline-flex; align-items: center; gap: 12px; color: var(--white); border: 1px solid var(--cinnabar); background: var(--cinnabar); font-weight: 700; }.primary-action:hover { background: var(--cinnabar-dark); }.secondary-action { display: inline-flex; gap: 9px; align-items: center; color: rgba(255,253,248,.9); border: 1px solid rgba(255,253,248,.28); background: rgba(255,255,255,.06); font-weight: 700; }.secondary-action:hover { background: rgba(255,255,255,.12); }.action-arrow { transition: transform var(--motion-feedback) var(--ease-standard); }.primary-action:hover .action-arrow { transform: translateX(4px); }.quiet-action { color: rgba(255,253,248,.84); border: 1px solid rgba(255,253,248,.22); background: transparent; }.quiet-action:hover { background: rgba(255,255,255,.08); }
.progress-seal { position: relative; z-index: 1; display: grid; width: 148px; height: 148px; flex: 0 0 auto; place-content: center; justify-items: center; border: 1px solid rgba(255,253,248,.22); border-radius: 50%; text-align: center; }.progress-seal.complete { animation: seal-arrive 460ms var(--ease-standard) both; }.seal-number { font-family: Georgia, "Times New Roman", serif; font-size: 56px; line-height: 1; font-variant-numeric: tabular-nums; }.seal-label { margin-top: 8px; color: rgba(255,253,248,.62); font-size: 13px; }
.metrics { display: grid; grid-template-columns: repeat(3,minmax(0,1fr)); gap: 14px; }.metric-card { display: flex; gap: 14px; align-items: center; min-height: 104px; padding: 22px; border: 1px solid var(--line); border-radius: var(--radius-md); background: rgba(255,253,247,.7); transition: transform var(--motion-standard) var(--ease-standard), box-shadow var(--motion-standard) var(--ease-standard); }.metric-card:hover { transform: translateY(-2px); box-shadow: 0 10px 28px rgba(49,57,51,.07); }.metric-card > svg { color: var(--cinnabar); }.metric-card p,.metric-card strong { margin: 0; }.metric-card p { margin-bottom: 7px; color: var(--ink-muted); font-size: 13px; }.metric-card strong { font-family: Georgia,"Microsoft YaHei UI",serif; font-size: 23px; font-weight: 620; font-variant-numeric: tabular-nums; }.metric-card .text-value { font-family: inherit; font-size: 17px; }
.day-plan { display: grid; grid-template-columns: .6fr 1fr; gap: 40px; padding: 30px 34px; border-top: 1px solid var(--line); }.day-plan h2 { margin: 0; font-size: 26px; }.route-list { display: grid; gap: 6px; }.route-list button { display: grid; grid-template-columns: 30px 24px 1fr 18px; gap: 10px; align-items: center; width: 100%; padding: 12px 10px; color: var(--ink); border: 0; border-radius: 12px; background: transparent; text-align: left; cursor: pointer; transition: transform var(--motion-feedback) var(--ease-standard), background var(--motion-feedback) var(--ease-standard); }.route-list button:hover { background: rgba(232,221,199,.48); transform: translateX(3px); }.route-list button > span,.route-list button > svg:first-of-type { color: var(--cinnabar); }.route-list button > svg:last-child { color: var(--ink-muted); }.route-list strong,.route-list small { display: block; }.route-list small { overflow: hidden; margin-top: 4px; color: var(--ink-muted); text-overflow: ellipsis; white-space: nowrap; }
.sr-only { position: absolute; width: 1px; height: 1px; overflow: hidden; clip: rect(0,0,0,0); white-space: nowrap; }
@keyframes dashboard-reveal { from { opacity: 0; transform: translateY(8px); } to { opacity: 1; transform: none; } } @keyframes seal-arrive { 0% { opacity: 0; transform: scale(.82); } 70% { transform: scale(1.04); } 100% { opacity: 1; transform: scale(1); } } @keyframes skeleton-sweep { to { transform: translateX(100%); } }
@media (max-width: 800px) { .training-dashboard { padding: 24px 18px 40px; }.dashboard-state { padding: 32px 26px; }.hero-panel { align-items: flex-start; padding: 34px 28px; }.progress-seal { display: none; }.metrics,.day-plan { grid-template-columns: 1fr; } }
@media (max-width: 520px) { .hero-actions { align-items: stretch; flex-direction: column; }.metrics { gap: 9px; }.route-list button { grid-template-columns: 26px 22px 1fr 16px; padding-inline: 4px; } }
@media (prefers-reduced-motion: reduce) { .dashboard-content,.progress-seal.complete,.skeleton::after { animation: none; }.metric-card,.route-list button,.action-arrow,.hero-actions button,.error-state button { transition: none; }.metric-card:hover,.route-list button:hover,.primary-action:hover .action-arrow { transform: none; } }
</style>
