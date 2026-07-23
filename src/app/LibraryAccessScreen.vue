<script setup lang="ts">
import { BookOpenCheck, KeyRound, LoaderCircle, LockKeyhole, RotateCcw, ShieldCheck } from '@lucide/vue'
import { computed } from 'vue'

type LibraryAccessPhase = 'checking' | 'locked' | 'error' | 'unlocking' | 'restarting'

const props = defineProps<{
  phase: LibraryAccessPhase
  message?: string
}>()

const emit = defineEmits<{
  unlock: []
  retry: []
}>()

const content = computed(() => {
  switch (props.phase) {
    case 'checking':
      return {
        eyebrow: '本地安全检查',
        title: '正在确认资料库状态',
        detail: '应用会先确认本机访问凭据，再打开加密数据库。',
      }
    case 'unlocking':
      return {
        eyebrow: '可信设备解锁',
        title: '正在解锁资料库',
        detail: '已使用当前 Windows 账户确认访问权限，请稍候。',
      }
    case 'restarting':
      return {
        eyebrow: '访问已恢复',
        title: '正在安全重启',
        detail: '重启后才会载入加密数据库、图片和学习档案。',
      }
    case 'error':
      return {
        eyebrow: '访问检查未完成',
        title: '暂时无法确认资料库状态',
        detail: props.message || 'Windows 凭据管理器暂时不可用，请重试或使用当前账户重新解锁。',
      }
    default:
      return {
        eyebrow: '纸墨学堂 · 本地保护',
        title: '本地资料库已锁定',
        detail: 'SQLCipher 数据库和原图密钥尚未载入，学习内容仍完整保存在这台电脑上。',
      }
  }
})

const isBusy = computed(() => ['checking', 'unlocking', 'restarting'].includes(props.phase))
</script>

<template>
  <main class="access-screen">
    <div
      class="access-orbit orbit-one"
      aria-hidden="true"
    />
    <div
      class="access-orbit orbit-two"
      aria-hidden="true"
    />

    <section
      class="access-card"
      :aria-busy="isBusy"
      aria-labelledby="library-access-title"
    >
      <header class="access-brand">
        <span class="brand-mark"><BookOpenCheck :size="19" /></span>
        <span>错题训练台</span>
      </header>

      <div
        :class="['access-seal', { busy: isBusy, unlocked: phase === 'restarting' }]"
        aria-hidden="true"
      >
        <LoaderCircle
          v-if="isBusy && phase !== 'restarting'"
          :size="34"
          class="spinner"
        />
        <ShieldCheck
          v-else-if="phase === 'restarting'"
          :size="34"
        />
        <LockKeyhole
          v-else
          :size="34"
        />
      </div>

      <p class="access-eyebrow">
        {{ content.eyebrow }}
      </p>
      <h1 id="library-access-title">
        {{ content.title }}
      </h1>
      <p
        class="access-detail"
        :role="phase === 'error' ? 'alert' : 'status'"
        :aria-live="phase === 'error' ? 'assertive' : 'polite'"
      >
        {{ content.detail }}
      </p>

      <div
        v-if="phase === 'locked'"
        class="access-explanation"
      >
        <KeyRound :size="18" />
        <span>
          <strong>无需额外密码</strong>
          当前登录的 Windows 账户就是这台设备的可信凭据。
        </span>
      </div>

      <button
        v-if="phase === 'locked'"
        type="button"
        class="access-primary"
        @click="emit('unlock')"
      >
        <KeyRound :size="18" />
        使用当前 Windows 账户解锁
      </button>

      <div
        v-else-if="phase === 'error'"
        class="access-actions"
      >
        <button
          type="button"
          class="access-secondary"
          @click="emit('retry')"
        >
          <RotateCcw :size="17" />
          重新检查
        </button>
        <button
          type="button"
          class="access-primary"
          @click="emit('unlock')"
        >
          <KeyRound :size="17" />
          重新解锁
        </button>
      </div>

      <p class="access-footnote">
        锁定只阻止应用进程读取资料，不会删除题目、图片或训练记录。
      </p>
    </section>
  </main>
</template>

<style scoped>
.access-screen {
  position: relative;
  display: grid;
  min-height: 100vh;
  min-height: 100dvh;
  padding: 32px 20px;
  place-items: center;
  overflow: hidden;
  background:
    linear-gradient(rgba(34, 48, 43, .025) 1px, transparent 1px),
    radial-gradient(circle at 50% 12%, rgba(232, 221, 199, .9), transparent 34rem),
    var(--paper);
  background-size: 100% 32px, auto, auto;
}

.access-orbit {
  position: absolute;
  border: 1px solid rgba(185, 88, 63, .13);
  border-radius: 50%;
  pointer-events: none;
}

.orbit-one {
  width: min(72vw, 740px);
  aspect-ratio: 1;
  transform: translate(-39vw, -34vh);
}

.orbit-two {
  right: -190px;
  bottom: -260px;
  width: 560px;
  height: 560px;
  border-color: rgba(33, 51, 45, .1);
}

.access-card {
  position: relative;
  z-index: 1;
  display: grid;
  justify-items: center;
  width: min(100%, 510px);
  padding: 30px clamp(24px, 7vw, 48px) 28px;
  border: 1px solid rgba(34, 48, 43, .13);
  border-radius: 28px;
  background: rgba(255, 253, 247, .94);
  box-shadow: 0 28px 80px rgba(60, 48, 31, .13);
  text-align: center;
  animation: access-arrive var(--motion-page) var(--ease-standard) both;
}

.access-card::before {
  position: absolute;
  inset: 9px;
  border: 1px solid rgba(185, 88, 63, .1);
  border-radius: 21px;
  content: "";
  pointer-events: none;
}

.access-brand {
  display: inline-flex;
  align-items: center;
  gap: 9px;
  color: var(--green-deep);
  font-family: var(--font-serif);
  font-size: 15px;
  font-weight: 700;
  letter-spacing: .08em;
}

.brand-mark {
  display: grid;
  width: 34px;
  height: 34px;
  place-items: center;
  color: var(--paper-raised);
  border-radius: 11px;
  background: var(--green-deep);
}

.access-seal {
  position: relative;
  display: grid;
  width: 88px;
  height: 88px;
  margin: 28px 0 18px;
  place-items: center;
  color: var(--paper-raised);
  border: 7px double rgba(255, 253, 247, .5);
  border-radius: 50%;
  background: var(--cinnabar);
  box-shadow: 0 13px 30px rgba(185, 88, 63, .25);
  transform: rotate(-3deg);
  transition: transform var(--motion-page) var(--ease-standard);
}

.access-seal.busy {
  background: var(--green-deep);
  transform: rotate(0) scale(.96);
}

.access-seal.unlocked {
  background: #426c58;
  transform: rotate(0) scale(1.04);
}

.spinner { animation: access-spin var(--motion-page) var(--ease-standard) both; }

.access-eyebrow {
  margin: 0 0 8px;
  color: var(--cinnabar);
  font-size: 11px;
  font-weight: 700;
  letter-spacing: .16em;
  text-transform: uppercase;
}

h1 {
  margin: 0;
  color: var(--ink);
  font-family: var(--font-serif);
  font-size: clamp(28px, 6vw, 38px);
  line-height: 1.2;
}

.access-detail {
  max-width: 390px;
  margin: 14px 0 0;
  color: var(--ink-muted);
  font-size: 14px;
  line-height: 1.75;
}

.access-explanation {
  display: grid;
  grid-template-columns: auto minmax(0, 1fr);
  gap: 11px;
  width: 100%;
  margin-top: 22px;
  padding: 13px 15px;
  color: var(--ink-muted);
  border: 1px solid var(--line);
  border-radius: 14px;
  background: var(--sand-soft);
  font-size: 12px;
  line-height: 1.6;
  text-align: left;
}

.access-explanation svg { margin-top: 2px; color: var(--green-deep); }
.access-explanation span { display: grid; gap: 1px; }
.access-explanation strong { color: var(--ink); font-size: 13px; }

.access-primary,
.access-secondary {
  display: inline-flex;
  min-height: 46px;
  align-items: center;
  justify-content: center;
  gap: 8px;
  padding: 0 20px;
  border-radius: 999px;
  font-size: 13px;
  font-weight: 700;
  cursor: pointer;
  transition: transform var(--motion-feedback) var(--ease-standard);
}

.access-primary {
  margin-top: 18px;
  color: var(--paper-raised);
  border: 1px solid var(--green-deep);
  background: var(--green-deep);
  box-shadow: 0 10px 24px rgba(33, 51, 45, .18);
}

.access-primary:hover { transform: translateY(-2px); box-shadow: 0 14px 30px rgba(33, 51, 45, .23); }
.access-primary:active { transform: translateY(0) scale(.98); }

.access-secondary {
  color: var(--green-deep);
  border: 1px solid var(--line-strong);
  background: transparent;
}

.access-secondary:hover { background: var(--sand-soft); transform: translateY(-1px); }

.access-actions {
  display: flex;
  flex-wrap: wrap;
  gap: 10px;
  justify-content: center;
  margin-top: 20px;
}

.access-actions .access-primary { margin-top: 0; }

.access-footnote {
  margin: 22px 0 0;
  color: var(--ink-muted);
  font-size: 11px;
  line-height: 1.6;
}

@keyframes access-arrive {
  from { opacity: 0; transform: translateY(14px) scale(.985); }
  to { opacity: 1; transform: translateY(0) scale(1); }
}

@keyframes access-spin {
  to { transform: rotate(360deg); }
}

@media (max-width: 520px) {
  .access-screen { padding: 16px 12px; }
  .access-card { padding: 24px 20px 22px; border-radius: 22px; }
  .access-card::before { inset: 7px; border-radius: 16px; }
  .access-seal { width: 78px; height: 78px; margin-top: 24px; }
  .access-primary { width: 100%; }
  .access-actions { width: 100%; }
  .access-actions button { flex: 1 1 150px; }
}

@media (prefers-reduced-motion: reduce) {
  .access-card,
  .spinner { animation: none; }
  .access-seal,
  .access-primary,
  .access-secondary { transition: none; }
}

@media (forced-colors: active) {
  .access-card,
  .access-seal,
  .access-primary,
  .access-secondary { border: 1px solid ButtonText; }
}
</style>
