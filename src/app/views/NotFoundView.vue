<script setup lang="ts">
import { SearchX } from '@lucide/vue'
import { useRouter } from 'vue-router'

const router = useRouter()

function goToDashboard() {
  void router.push({ name: 'dashboard' })
}
</script>

<template>
  <section
    class="not-found"
    role="status"
    aria-live="polite"
  >
    <SearchX
      :size="32"
      aria-hidden="true"
    />
    <p class="not-found-kicker">
      页面暂时找不到
    </p>
    <h1>这条路径已经失效</h1>
    <p class="not-found-copy">
      内容没有被删除。返回训练台后，可以从侧边栏继续使用错题库和采集整理。
    </p>
    <button
      type="button"
      @click="goToDashboard"
    >
      回到训练台
    </button>
  </section>
</template>

<style scoped>
.not-found {
  display: grid;
  min-height: 54vh;
  padding: 56px 24px;
  place-items: center;
  align-content: center;
  gap: 10px;
  color: var(--ink-muted);
  text-align: center;
}

.not-found svg { color: var(--cinnabar); }
.not-found-kicker { margin: 4px 0 0; color: var(--cinnabar); font-size: 12px; font-weight: 700; letter-spacing: .08em; }
.not-found h1 { margin: 0; color: var(--ink); font-family: var(--font-serif); font-size: clamp(26px, 4vw, 36px); }
.not-found-copy { max-width: 440px; margin: 0; line-height: 1.7; }
.not-found button { min-height: 42px; margin-top: 8px; padding: 0 18px; color: var(--paper); border: 0; border-radius: 999px; background: var(--green-deep); cursor: pointer; transition: transform var(--motion-feedback) var(--ease-standard), box-shadow var(--motion-feedback) var(--ease-standard); }
.not-found button:hover { transform: translateY(-1px); box-shadow: 0 8px 18px rgba(33, 51, 45, .18); }
.not-found button:active { transform: translateY(0); }
@media (prefers-reduced-motion: reduce) { .not-found button { transition: none; } }
</style>
