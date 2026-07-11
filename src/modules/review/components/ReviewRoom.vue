<script setup lang="ts">
import { computed, ref, watch } from 'vue'
import { ArrowLeft, Eye, RotateCcw, Sparkles } from '@lucide/vue'
import type { SimpleRating } from '../domain/rating'

const props = defineProps<{
  subject: string
  prompt: string
  answer: string
  current: number
  total: number
}>()

const emit = defineEmits<{
  exit: []
  rate: [rating: SimpleRating]
}>()

const revealed = ref(false)
const progress = computed(() => `${Math.round((props.current / props.total) * 100)}%`)

watch(() => props.prompt, () => { revealed.value = false })

function submit(rating: SimpleRating) {
  emit('rate', rating)
}
</script>

<template>
  <main
    class="review-room"
    aria-labelledby="review-heading"
  >
    <header class="review-header">
      <button
        class="icon-action"
        type="button"
        aria-label="退出训练"
        @click="$emit('exit')"
      >
        <ArrowLeft :size="19" />
      </button>
      <div class="review-progress">
        <span>{{ current }} / {{ total }}</span>
        <div
          class="progress-track"
          aria-hidden="true"
        >
          <i :style="{ width: progress }" />
        </div>
      </div>
      <span class="subject-chip">{{ subject }}</span>
    </header>

    <section class="review-stage">
      <p class="stage-kicker">
        <Sparkles
          :size="15"
          aria-hidden="true"
        /> 回忆模式
      </p>
      <h1 id="review-heading">
        先想一遍，再看答案
      </h1>
      <article class="problem-paper">
        <span class="paper-label">题目</span>
        <p>{{ prompt }}</p>
      </article>

      <button
        v-if="!revealed"
        class="reveal-action"
        type="button"
        @click="revealed = true"
      >
        <Eye
          :size="19"
          aria-hidden="true"
        />显示答案
      </button>

      <div
        v-else
        class="answer-area"
      >
        <article
          class="answer-paper"
          aria-live="polite"
        >
          <span class="paper-label">答案</span>
          <p>{{ answer }}</p>
        </article>
        <p class="rating-prompt">
          这次你记得怎么样？
        </p>
        <div class="rating-actions">
          <button
            class="forgot-action"
            type="button"
            @click="submit('forgot')"
          >
            <RotateCcw :size="18" />忘记了
          </button>
          <button
            class="remember-action"
            type="button"
            @click="submit('remembered')"
          >
            <Sparkles :size="18" />记住了
          </button>
        </div>
        <small class="shortcut-hint">快捷键：1 忘记 · 2 记住</small>
      </div>
    </section>
  </main>
</template>

<style scoped>
.review-room { min-height: 100vh; padding: 24px 42px 60px; background: linear-gradient(180deg, rgba(232,221,199,.42), transparent 240px); }
.review-header { display: grid; grid-template-columns: 42px minmax(180px,460px) auto; gap: 20px; align-items: center; max-width: 920px; margin: 0 auto 42px; }
.icon-action { display: grid; width: 40px; height: 40px; place-items: center; border: 1px solid var(--line); border-radius: 50%; background: rgba(255,253,247,.72); cursor: pointer; }
.review-progress { display: flex; gap: 14px; align-items: center; color: var(--ink-muted); font-family: Georgia, serif; font-size: 13px; }
.progress-track { overflow: hidden; height: 4px; flex: 1; border-radius: 999px; background: var(--sand); }
.progress-track i { display: block; height: 100%; border-radius: inherit; background: var(--cinnabar); transition: width var(--motion-page) var(--ease-standard); }
.subject-chip { padding: 7px 12px; color: var(--ink-muted); border: 1px solid var(--line); border-radius: 999px; font-size: 12px; }
.review-stage { max-width: 780px; margin: 0 auto; text-align: center; }
.stage-kicker { display: inline-flex; gap: 7px; align-items: center; margin: 0; color: var(--cinnabar); font-size: 12px; font-weight: 700; letter-spacing: .1em; }
h1 { margin: 11px 0 28px; font-size: clamp(29px,4vw,43px); font-weight: 650; letter-spacing: -.04em; }
.problem-paper, .answer-paper { position: relative; min-height: 190px; padding: 42px; border: 1px solid var(--line); border-radius: 3px 18px 18px 18px; background: var(--paper-raised); box-shadow: var(--shadow-soft); text-align: left; }
.problem-paper p, .answer-paper p { margin: 0; font-family: Georgia, "Microsoft YaHei UI", serif; font-size: clamp(20px,2.6vw,28px); line-height: 1.8; }
.paper-label { position: absolute; top: 0; left: 0; padding: 7px 14px; color: var(--ink-muted); border-radius: 0 0 10px; background: var(--sand); font-size: 11px; font-weight: 700; letter-spacing: .12em; }
.reveal-action { display: inline-flex; gap: 9px; align-items: center; min-height: 48px; margin-top: 26px; padding: 0 24px; color: var(--white); border: 0; border-radius: 999px; background: var(--green-deep); cursor: pointer; font-weight: 700; transition: transform var(--motion-feedback) var(--ease-standard); }
.reveal-action:active, .rating-actions button:active { transform: scale(.985); }
.answer-area { margin-top: 18px; }
.answer-paper { min-height: 130px; border-color: rgba(185,88,63,.25); background: #fbf5ea; }
.answer-paper .paper-label { color: #8b4634; background: #efd8cd; }
.rating-prompt { margin: 26px 0 14px; color: var(--ink-muted); }
.rating-actions { display: grid; grid-template-columns: 1fr 1fr; gap: 12px; max-width: 430px; margin: 0 auto; }
.rating-actions button { display: inline-flex; gap: 9px; align-items: center; justify-content: center; min-height: 48px; border-radius: 999px; cursor: pointer; font-weight: 700; transition: transform var(--motion-feedback) var(--ease-standard), background var(--motion-standard) var(--ease-standard); }
.forgot-action { border: 1px solid var(--line); background: rgba(255,253,247,.75); }
.remember-action { color: var(--white); border: 1px solid var(--cinnabar); background: var(--cinnabar); }
.shortcut-hint { display: block; margin-top: 14px; color: var(--ink-muted); }
@media (max-width: 620px) { .review-room { padding: 18px 18px 36px; } .review-header { margin-bottom: 28px; } .problem-paper, .answer-paper { padding: 34px 24px 26px; } }
</style>
