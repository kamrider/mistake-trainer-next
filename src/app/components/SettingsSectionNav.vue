<script setup lang="ts">
import { computed, nextTick, onBeforeUnmount, onMounted, ref, watch } from 'vue'
import type { SettingsSectionLink } from '../settings-section-catalog'

const props = defineProps<{
  sections: SettingsSectionLink[]
}>()

const activeId = ref('')
const scroller = ref<HTMLElement>()
const canScrollLeft = ref(false)
const canScrollRight = ref(false)
const visibleIds = new Set<string>()
let observer: IntersectionObserver | undefined
let bindVersion = 0

const sectionKey = computed(() => props.sections.map(section => section.id).join('|'))
const activeIndex = computed(() =>
  Math.max(0, props.sections.findIndex(section => section.id === activeId.value)),
)
const sectionGroups = computed(() => {
  const groups: Array<{ label: string, id: string, sections: SettingsSectionLink[] }> = []
  for (const section of props.sections) {
    let group = groups.find(candidate => candidate.label === section.group)
    if (!group) {
      group = {
        label: section.group,
        id: `settings-nav-group-${groups.length + 1}`,
        sections: [],
      }
      groups.push(group)
    }
    group.sections.push(section)
  }
  return groups
})

function prefersReducedMotion() {
  return window.matchMedia?.('(prefers-reduced-motion: reduce)').matches ?? false
}

function updateOverflow() {
  const container = scroller.value
  if (!container) return
  canScrollLeft.value = container.scrollLeft > 2
  canScrollRight.value = container.scrollLeft + container.clientWidth < container.scrollWidth - 2
}

function scrollDirectory(direction: -1 | 1) {
  const container = scroller.value
  if (!container) return
  container.scrollBy({
    left: direction * Math.max(180, container.clientWidth * 0.72),
    behavior: prefersReducedMotion() ? 'auto' : 'smooth',
  })
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
  window.requestAnimationFrame(updateOverflow)
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
function handleResize() {
  updateOverflow()
}

onMounted(async () => {
  await bindObserver()
  updateOverflow()
  window.addEventListener('resize', handleResize)
})
onBeforeUnmount(() => {
  bindVersion += 1
  observer?.disconnect()
  window.removeEventListener('resize', handleResize)
})
</script>

<template>
  <nav
    class="settings-section-nav"
    :class="{ 'can-scroll-left': canScrollLeft, 'can-scroll-right': canScrollRight }"
    aria-label="设置目录"
  >
    <button
      class="directory-scroll previous"
      type="button"
      aria-label="查看前面的设置"
      :disabled="!canScrollLeft"
      @click="scrollDirectory(-1)"
    >
      ‹
    </button>
    <div
      ref="scroller"
      class="settings-section-scroller"
      tabindex="0"
      aria-label="可横向滚动的设置目录"
      @scroll="updateOverflow"
    >
      <div class="settings-section-track">
        <span
          class="settings-section-indicator"
          aria-hidden="true"
        />
        <div
          v-for="group in sectionGroups"
          :key="group.label"
          class="settings-section-group"
          role="group"
          :aria-labelledby="group.id"
        >
          <span
            :id="group.id"
            class="settings-section-group-label"
          >{{ group.label }}</span>
          <div class="settings-section-group-items">
            <button
              v-for="section in group.sections"
              :key="section.id"
              type="button"
              :class="{ active: section.id === activeId }"
              :aria-current="section.id === activeId ? 'location' : undefined"
              :aria-controls="section.id"
              @click="selectSection(section.id)"
            >
              <strong>{{ section.label }}</strong>
              <small>{{ section.hint }}</small>
            </button>
          </div>
        </div>
      </div>
    </div>
    <button
      class="directory-scroll next"
      type="button"
      aria-label="查看更多设置"
      :disabled="!canScrollRight"
      @click="scrollDirectory(1)"
    >
      ›
    </button>
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
.directory-scroll {
  position: absolute;
  z-index: 3;
  top: 50%;
  display: grid;
  width: 44px;
  height: 44px;
  min-height: 0;
  padding: 0;
  place-items: center;
  color: var(--green-deep);
  border: 1px solid var(--line);
  border-radius: 999px;
  background: var(--paper);
  box-shadow: 0 5px 14px rgba(33,51,45,.12);
  font-size: 22px;
  opacity: .32;
  pointer-events: none;
  transform: translateY(-50%);
}
.directory-scroll.previous { left: 8px; }
.directory-scroll.next { right: 8px; }
.can-scroll-left .directory-scroll.previous,
.can-scroll-right .directory-scroll.next { opacity: 1; pointer-events: auto; }
.directory-scroll:disabled { opacity: .32; }
.settings-section-nav::before,
.settings-section-nav::after {
  position: absolute;
  z-index: 2;
  top: 5px;
  bottom: 5px;
  width: 52px;
  content: "";
  opacity: 0;
  pointer-events: none;
}
.settings-section-nav::before { left: 5px; background: linear-gradient(90deg,rgba(246,241,231,.98),transparent); }
.settings-section-nav::after { right: 5px; background: linear-gradient(-90deg,rgba(246,241,231,.98),transparent); }
.settings-section-nav.can-scroll-left::before,
.settings-section-nav.can-scroll-right::after { opacity: 1; }
.settings-section-scroller {
  overflow-x: auto;
  overscroll-behavior-inline: contain;
  scrollbar-width: none;
}
.settings-section-scroller::-webkit-scrollbar { display: none; }
.settings-section-track {
  position: relative;
  isolation: isolate;
  display: flex;
  min-width: max-content;
  gap: 4px;
  align-items: stretch;
}
.settings-section-indicator {
  position: absolute;
  width: 1px;
  height: 1px;
  overflow: hidden;
}
.settings-section-group {
  display: grid;
  min-width: max-content;
  grid-template-rows: auto 1fr;
  padding: 3px;
  border-right: 1px solid rgba(33,51,45,.1);
}
.settings-section-group:last-of-type { border-right: 0; }
.settings-section-group-label {
  padding: 2px 10px 3px;
  color: var(--ink-muted);
  font-size: 12px;
  font-weight: 800;
  letter-spacing: .06em;
}
.settings-section-group-items { display: flex; }
.settings-section-group-items button {
  display: grid;
  min-width: 108px;
  min-height: 48px;
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
.settings-section-group-items button:hover { color: var(--green-deep); background: rgba(255,253,247,.55); transform: translateY(-1px); }
.settings-section-group-items button.active { color: var(--green-deep); background: rgba(255,253,247,.9); box-shadow: inset 0 -2px var(--cinnabar); }
.settings-section-group-items button strong,.settings-section-group-items button small {
  display: block;
  max-width: 100%;
  overflow: hidden;
  text-overflow: ellipsis;
  white-space: nowrap;
}
.settings-section-group-items button strong { font-size: 13px; font-weight: 790; }
.settings-section-group-items button small { margin-top: 3px; color: inherit; font-size: 12px; opacity: .72; }
.settings-section-group-items button:focus-visible,.directory-scroll:focus-visible,.settings-section-scroller:focus-visible { outline: 3px solid rgba(185,88,63,.28); outline-offset: -2px; }
@media(max-width:760px) {
  .settings-section-nav { top: 8px; margin: -5px 0 16px; padding: 4px; border-radius: 14px; }
  .settings-section-group-items button { min-height: 48px; padding-inline: 8px; }
  .directory-scroll { width: 44px; height: 44px; }
}
@media(forced-colors:active) {
  .settings-section-group-items button.active { color: Highlight; border: 1px solid Highlight; }
}
@media(prefers-reduced-motion:reduce) {
  .settings-section-group-items button { transition: none; }
  .settings-section-group-items button:hover { transform: none; }
}
</style>
