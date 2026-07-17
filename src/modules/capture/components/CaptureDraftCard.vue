<script setup lang="ts">
import { Check, Image as ImageIcon, Maximize2, RotateCw, Undo2, X } from '@lucide/vue'
import { computed, reactive, ref, watch } from 'vue'
import type { CaptureDraftSummary, CaptureItemSummary } from '../../../shared/api/bindings'
import CaptureThumbnail from './CaptureThumbnail.vue'

type CardRole = 'question' | 'answer'

const props = defineProps<{
  draft: CaptureDraftSummary
  draftIndex: number
  items: CaptureItemSummary[]
  previews: Record<string, string>
  selected: boolean
  busy: boolean
}>()

const emit = defineEmits<{
  select: [draftId: string]
  preview: [itemId: string]
  pointerStart: [itemId: string, event: PointerEvent]
  returnItem: [itemId: string]
}>()

const flipped = ref(false)
const indexes = reactive<Record<CardRole, number>>({ question: 0, answer: 0 })
const expanded = ref<{ item: CaptureItemSummary, role: CardRole }>()
const itemById = computed(() => new Map(props.items.map(item => [item.id, item])))
const questionItems = computed(() => mapItems(props.draft.questionItemIds))
const answerItems = computed(() => mapItems(props.draft.answerItemIds))

watch([questionItems, answerItems], ([questions, answers]) => {
  indexes.question = Math.min(indexes.question, Math.max(0, questions.length - 1))
  indexes.answer = Math.min(indexes.answer, Math.max(0, answers.length - 1))
  for (const item of [questions[indexes.question], answers[indexes.answer]]) {
    if (item) emit('preview', item.id)
  }
}, { immediate: true })

function mapItems(ids: string[]) {
  return ids.map(id => itemById.value.get(id)).filter((item): item is CaptureItemSummary => Boolean(item))
}

function activeItem(role: CardRole) {
  const items = role === 'question' ? questionItems.value : answerItems.value
  return items[indexes[role]]
}

function selectImage(role: CardRole, index: number) {
  indexes[role] = index
  const item = activeItem(role)
  if (item) emit('preview', item.id)
}

function flip() {
  emit('select', props.draft.id)
  flipped.value = !flipped.value
}

function openImage(role: CardRole) {
  const item = activeItem(role)
  if (item && props.previews[item.id]) expanded.value = { item, role }
}
</script>

<template>
  <article
    class="draft-card"
    :class="{ 'is-ready': draft.ready, 'is-selected': selected }"
    :aria-label="`第 ${draftIndex + 1} 道错题卡`"
    data-capture-drop="card"
    :data-draft-id="draft.id"
  >
    <header class="draft-header">
      <button
        type="button"
        class="draft-target"
        :aria-pressed="selected"
        @click="emit('select', draft.id)"
      >
        <span class="draft-number">{{ String(draftIndex + 1).padStart(2, '0') }}</span>
        <span><strong>{{ draft.subject || '未填写科目' }}</strong><small>{{ selected ? '正在编辑这张卡' : '点击选择' }}</small></span>
      </button>
      <span class="ready-mark"><Check
        v-if="draft.ready"
        :size="13"
      />{{ draft.ready ? '就绪' : '未完成' }}</span>
    </header>

    <div class="card-perspective">
      <div
        class="card-inner"
        :class="{ 'is-flipped': flipped }"
        role="button"
        tabindex="0"
        :aria-label="flipped ? '当前显示答案，点击翻回题面' : '当前显示题面，点击翻到答案'"
        @click="flip"
        @keydown.enter.prevent="flip"
        @keydown.space.prevent="flip"
      >
        <section
          v-for="role in (['question', 'answer'] as CardRole[])"
          :key="role"
          class="card-face"
          :class="`is-${role}`"
          :aria-hidden="flipped !== (role === 'answer')"
          :inert="flipped !== (role === 'answer')"
        >
          <div class="face-label">
            <span>{{ role === 'question' ? '题面' : '答案' }}</span>
            <strong>{{ (role === 'question' ? questionItems : answerItems).length }} 张</strong>
          </div>

          <div
            v-if="activeItem(role)"
            class="main-image-wrap"
          >
            <img
              v-if="previews[activeItem(role)!.id]"
              :src="previews[activeItem(role)!.id]"
              :alt="activeItem(role)!.sourceName"
              draggable="false"
            >
            <div
              v-else
              class="image-loading"
            >
              <ImageIcon :size="34" /><span>正在加载清晰预览</span>
            </div>
            <button
              type="button"
              class="expand-image"
              :disabled="!previews[activeItem(role)!.id]"
              :aria-label="`放大查看 ${activeItem(role)!.sourceName}`"
              @click.stop="openImage(role)"
            >
              <Maximize2 :size="16" /> 大图
            </button>
          </div>
          <div
            v-else
            class="face-empty"
          >
            <ImageIcon :size="34" />
            <strong>还没有{{ role === 'question' ? '题图' : '答案图' }}</strong>
            <span>把左侧对应颜色的图片拖到这张卡上</span>
          </div>

          <div
            v-if="(role === 'question' ? questionItems : answerItems).length"
            class="face-filmstrip"
            @click.stop
          >
            <CaptureThumbnail
              v-for="(item, index) in (role === 'question' ? questionItems : answerItems)"
              :key="item.id"
              :item="item"
              :data-url="previews[item.id]"
              variant="filmstrip"
              :active="index === indexes[role]"
              :disabled="busy"
              @preview="emit('preview', $event)"
              @activate="selectImage(role, index)"
              @pointer-start="(itemId, event) => emit('pointerStart', itemId, event)"
            />
            <button
              type="button"
              class="return-image"
              :disabled="busy"
              :aria-label="`把当前${role === 'question' ? '题图' : '答案图'}移回待配对`"
              @click.stop="activeItem(role) && emit('returnItem', activeItem(role)!.id)"
            >
              <Undo2 :size="14" /> 移回
            </button>
          </div>
        </section>
      </div>
      <button
        type="button"
        class="flip-button"
        :aria-label="flipped ? '翻回题面' : '翻到答案'"
        @click="flip"
      >
        <RotateCw :size="17" />{{ flipped ? '翻回题面' : '翻到答案' }}
      </button>
    </div>

    <Teleport to="body">
      <div
        v-if="expanded && previews[expanded.item.id]"
        class="image-overlay"
        role="presentation"
        @click.self="expanded = undefined"
        @keydown.esc="expanded = undefined"
      >
        <section
          role="dialog"
          aria-modal="true"
          :aria-label="`大图查看 ${expanded.item.sourceName}`"
          tabindex="-1"
        >
          <button
            type="button"
            aria-label="关闭大图"
            @click="expanded = undefined"
          >
            <X :size="20" />
          </button>
          <img
            :src="previews[expanded.item.id]"
            :alt="expanded.item.sourceName"
          >
          <p>{{ expanded.role === 'question' ? '题面' : '答案' }} · {{ expanded.item.sourceName }}</p>
        </section>
      </div>
    </Teleport>
  </article>
</template>

<style scoped>
.draft-card{padding:18px;border:1px solid var(--line);border-radius:7px 22px 22px;background:rgba(255,253,247,.78);box-shadow:var(--shadow-soft);transition:transform var(--motion-standard) var(--ease-standard),border-color var(--motion-standard),box-shadow var(--motion-standard)}
.draft-card.is-selected{border-color:rgba(185,88,63,.5);box-shadow:0 18px 45px rgba(34,48,43,.13),inset 4px 0 0 var(--cinnabar);transform:translateY(-2px)}
.draft-header{display:flex;gap:14px;align-items:center;justify-content:space-between}.draft-target{display:flex;gap:10px;align-items:center;padding:0;color:var(--ink);border:0;background:transparent;text-align:left;cursor:pointer}.draft-target>span:last-child{display:grid}.draft-target strong{font-size:17px}.draft-target small{margin-top:2px;color:var(--ink-muted);font-size:10px}.draft-number{color:var(--cinnabar);font-family:serif;font-size:23px}.ready-mark{display:inline-flex;gap:5px;align-items:center;padding:6px 9px;color:var(--ink-muted);border-radius:999px;background:var(--green-soft);font-size:9px;font-weight:800}.is-ready .ready-mark{color:#416b5a}
.card-perspective{position:relative;margin-top:14px;perspective:1400px}.card-inner{position:relative;min-height:480px;transform-style:preserve-3d;transition:transform var(--motion-page) var(--ease-standard)}.card-inner.is-flipped{transform:rotateY(180deg)}.card-face{position:absolute;inset:0;display:grid;grid-template-rows:auto minmax(0,1fr) auto;min-height:440px;padding:14px;overflow:hidden;border:1px solid rgba(33,51,45,.13);border-radius:18px;background:rgba(246,241,231,.72);backface-visibility:hidden}.card-face.is-answer{background:linear-gradient(145deg,rgba(185,88,63,.08),rgba(255,253,247,.9));transform:rotateY(180deg)}
.face-label{display:flex;justify-content:space-between;align-items:center;margin-bottom:10px}.face-label span{color:var(--cinnabar);font-size:10px;font-weight:850;letter-spacing:.12em}.face-label strong{color:var(--ink-muted);font-size:10px}.main-image-wrap{position:relative;display:grid;min-height:340px;overflow:hidden;place-items:center;border-radius:13px;background:#fff;box-shadow:inset 0 0 0 1px rgba(33,51,45,.08)}.main-image-wrap>img{width:100%;height:100%;max-height:430px;object-fit:contain}.image-loading,.face-empty{display:grid;place-items:center;align-content:center;gap:8px;min-height:340px;color:var(--ink-muted);text-align:center}.face-empty span{font-size:11px}.expand-image{position:absolute;right:10px;bottom:10px;display:inline-flex;gap:5px;align-items:center;min-height:34px;padding:0 11px;color:var(--paper);border:0;border-radius:999px;background:rgba(33,51,45,.82);font-size:10px;cursor:pointer}.face-filmstrip{display:flex;gap:7px;align-items:center;min-width:0;margin-top:10px;padding:2px;overflow-x:auto}.return-image{display:inline-flex;flex:0 0 auto;gap:4px;align-items:center;min-height:32px;padding:0 9px;color:var(--ink-muted);border:1px solid var(--line);border-radius:9px;background:var(--paper);font-size:9px;cursor:pointer}.flip-button{position:absolute;z-index:3;right:18px;bottom:18px;display:flex;gap:7px;align-items:center;min-height:42px;padding:0 15px;color:var(--paper);border:0;border-radius:999px;background:var(--green-deep);font-weight:780;cursor:pointer;box-shadow:0 10px 24px rgba(33,51,45,.2)}
.image-overlay{position:fixed;z-index:120;inset:0;display:grid;padding:32px;place-items:center;background:rgba(20,28,25,.82);backdrop-filter:blur(14px)}.image-overlay section{position:relative;display:grid;width:min(1400px,100%);height:min(900px,calc(100vh - 64px));padding:16px;grid-template-rows:minmax(0,1fr) auto;border-radius:18px;background:var(--paper)}.image-overlay img{width:100%;height:100%;object-fit:contain}.image-overlay p{min-width:0;margin:10px 42px 0 0;overflow:hidden;text-overflow:ellipsis;white-space:nowrap}.image-overlay button{position:absolute;z-index:1;top:14px;right:14px;display:grid;width:40px;height:40px;place-items:center;color:var(--paper);border:0;border-radius:50%;background:var(--green-deep);cursor:pointer}button:disabled{cursor:not-allowed;opacity:.42}
@media(max-width:760px){.draft-card{padding:14px}.card-inner{min-height:420px}.card-face{min-height:390px}.main-image-wrap,.image-loading,.face-empty{min-height:290px}.image-overlay{padding:12px}.image-overlay section{height:calc(100vh - 24px)}}
@media(prefers-reduced-motion:reduce){.draft-card,.card-inner{transition:none}.draft-card.is-selected{transform:none}}
</style>
