<script setup lang="ts">
defineProps<{
  eyebrow: string
  title: string
  description: string
}>()
</script>

<template>
  <main class="placeholder-view">
    <p>{{ eyebrow }}</p>
    <h1>{{ title }}</h1>
    <span>{{ description }}</span>
    <div
      class="placeholder-paper"
      aria-hidden="true"
    >
      <i /><i /><i />
    </div>
  </main>
</template>

<style scoped>
.placeholder-view { max-width: 1060px; margin: 0 auto; padding: 68px 48px; }
.placeholder-view > p { margin: 0 0 10px; color: var(--cinnabar); font-size: 12px; font-weight: 750; letter-spacing: .16em; }
h1 { margin: 0; font-size: clamp(38px,5vw,62px); letter-spacing: -.05em; }
.placeholder-view > span { display: block; max-width: 620px; margin-top: 14px; color: var(--ink-muted); font-size: 17px; line-height: 1.7; }
.placeholder-paper { display: grid; gap: 18px; min-height: 320px; margin-top: 46px; padding: 46px; border: 1px solid var(--line); border-radius: 3px 22px 22px 22px; background: rgba(255,253,247,.7); box-shadow: var(--shadow-soft); }
.placeholder-paper i { display: block; height: 18px; border-radius: 999px; background: linear-gradient(90deg, var(--sand), rgba(232,221,199,.3)); }
.placeholder-paper i:nth-child(2) { width: 74%; }
.placeholder-paper i:nth-child(3) { width: 48%; }
</style>
