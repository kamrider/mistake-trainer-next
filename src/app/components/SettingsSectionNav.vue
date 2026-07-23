<script setup lang="ts">
import { computed, nextTick, onBeforeUnmount, onMounted, ref, watch } from 'vue'

export interface SettingsSectionLink {
  id: string
  label: string
  hint: string
}

const props = defineProps<{
  sections: SettingsSectionLink[]
}>()

const activeId = ref('')
const scroller = ref<HTMLElement>()
const visibleIds = new Set<string>()
let observer: IntersectionObserver | undefined
let bindVersion = 0

const sectionKey = computed(() => props.sections.map(section => section.id).join('|'))
const activeIndex = computed(() =>
  Math.max(0, props.sections.findIndex(section => section.id === activeId.value)),
)
const trackStyle = computed(() => ({
  '--section-count': Math.max(1, props.sections.length),
  '--active-x': `${activeIndex.value * 100}%`,
}))

function prefersReducedMotion() {
  return window.matchMedia?.('(prefers-reduced-motion: reduce)').matches ?? false
}

async function revealActiveSection() {
  await nextTick()
  const container = scroller.value
  const button = container?.querySelectorAll<HTMLButtonElement>('button')[activeIndex.value]
  if (!container || !button || typeof container.scrollTo !== 'function') return
  container.scrollTo({
    left: Math.max(0, button.offsetLeft - (container.clientWidth - button.offsetWidth) / 2),
    behavior: prefersReducedMotion() ? 'auto' : 'smooth',
  })
}

function selectSection(id: string) {
  activeId.value = id
  document.getElementById(id)?.scrollIntoView({
    block: 'start',
    behavior: prefersReducedMotion() ? 'auto' : 'smooth',
  })
}

function chooseNearestVisible() {
  const candidate = props.sections
    .filter(section => visibleIds.has(section.id))
    .map(section => ({
      id: section.id,
      distance: Math.abs((document.getElementById(section.id)?.getBoundingClientRect().top ?? 120) - 120),
    }))
    .sort((left, right) => left.distance - right.distance)[0]
  if (candidate) activeId.value = candidate.id
}

async function bindObserver() {
  const version = ++bindVersion
  observer?.disconnect()
  observer = undefined
  visibleIds.clear()
  await nextTick()
  if (version !== bindVersion) return

  const availableIds = new Set(props.sections.map(section => section.id))
  if (!availableIds.has(activeId.value)) {
    activeId.value = props.sections[0]?.id ?? ''
  }
  if (typeof IntersectionObserver === 'undefined') return

  observer = new IntersectionObserver((entries) => {
    for (const entry of entries) {
      const id = (entry.target as HTMLElement).id
      if (entry.isIntersecting) visibleIds.add(id)
      else visibleIds.delete(id)
    }
    chooseNearestVisible()
  }, {
    rootMargin: '-120px 0px -55% 0px',
    threshold: [0, 0.25, 0.6],
  })

  for (const section of props.sections) {
    const element = document.getElementById(section.id)
    if (element) observer.observe(element)
  }
}

watch(sectionKey, () => void bindObserver(), { flush: 'post' })
watch(activeId, () => void revealActiveSection(), { flush: 'post' })
onMounted(() => void bindObserver())
onBeforeUnmount(() => {
  bindVersion += 1
  observer?.disconnect()
})
</script>

<template>
  <nav
    class="settings-section-nav"
    aria-label="设置目录"
  >
    <div
      ref="scroller"
      class="settings-section-scroller"
    >
      <div
        class="settings-section-track"
        :style="trackStyle"
      >
        <span
          class="settings-section-indicator"
          aria-hidden="true"
        />
        <button
          v-for="section in sections"
          :key="section.id"
          type="button"
          :class="{ active: section.id === activeId }"
          :aria-current="section.id === activeId ? 'location' : undefined"
          @click="selectSection(section.id)"
        >
          <strong>{{ section.label }}</strong>
          <small>{{ section.hint }}</small>
        </button>
      </div>
    </div>
  </nav>
</template>

<style scoped>
.settings-section-nav {
  position: sticky;
  z-index: 35;
  top: 12px;
  margin: -10px 0 20px;
  padding: 5px;
  border: 1px solid rgba(33,51,45,.15);
  border-radius: 16px;
  background: rgba(246,241,231,.9);
  box-shadow: 0 12px 34px rgba(34,48,43,.09);
  backdrop-filter: blur(13px);
}
.settings-section-scroller {
  overflow-x: auto;
  overscroll-behavior-inline: contain;
  scrollbar-width: none;
}
.settings-section-scroller::-webkit-scrollbar { display: none; }
.settings-section-track {
  --section-count: 1;
  --active-x: 0%;
  position: relative;
  isolation: isolate;
  display: grid;
  min-width: 690px;
  grid-template-columns: repeat(var(--section-count),minmax(108px,1fr));
}
.settings-section-indicator {
  position: absolute;
  z-index: -1;
  inset: 0 auto 0 0;
  width: calc(100% / var(--section-count));
  border: 1px solid rgba(33,51,45,.16);
  border-radius: 12px 4px 12px 12px;
  background: linear-gradient(145deg,rgba(220,228,220,.96),rgba(232,221,199,.72));
  box-shadow: 0 5px 14px rgba(33,51,45,.08);
  transform: translate3d(var(--active-x),0,0);
  transition: transform var(--motion-page) var(--ease-standard);
}
button {
  display: grid;
  min-width: 0;
  min-height: 54px;
  padding: 8px 11px;
  place-items: center;
  align-content: center;
  color: var(--ink-muted);
  border: 0;
  border-radius: 12px;
  background: transparent;
  cursor: pointer;
  text-align: center;
  transition: color var(--motion-standard) var(--ease-standard),transform var(--motion-feedback) var(--ease-standard);
}
button:hover { color: var(--green-deep); transform: translateY(-1px); }
button.active { color: var(--green-deep); }
button strong,button small {
  display: block;
  max-width: 100%;
  overflow: hidden;
  text-overflow: ellipsis;
  white-space: nowrap;
}
button strong { font-size: 12px; font-weight: 790; }
button small { margin-top: 3px; color: inherit; font-size: 9px; opacity: .72; }
button:focus-visible { outline: 3px solid rgba(185,88,63,.28); outline-offset: -2px; }
@media(max-width:760px) {
  .settings-section-nav { top: 8px; margin: -5px 0 16px; padding: 4px; border-radius: 14px; }
  .settings-section-track { min-width: 650px; }
  button { min-height: 50px; padding-inline: 8px; }
}
@media(forced-colors:active) {
  .settings-section-indicator { border-color: Highlight; background: Highlight; forced-color-adjust: auto; }
  button.active { color: HighlightText; }
}
@media(prefers-reduced-motion:reduce) {
  .settings-section-indicator,button { transition: none; }
  button:hover { transform: none; }
}
</style>
