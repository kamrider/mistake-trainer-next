<script setup lang="ts">
import { ArrowRightLeft, Check, Crop, GripVertical, Image as ImageIcon, Maximize2, Plus, RotateCcw, RotateCw, X } from '@lucide/vue'
import { computed, nextTick, reactive, ref, watch } from 'vue'
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
  subjectOptions: string[]
  dropRole?: CardRole | null
  settled?: boolean
  revealItemId?: string | undefined
  revealRequestKey?: number
}>()

const emit = defineEmits<{
  select: [draftId: string]
  navigateDraft: [direction: 'previous' | 'next']
  preview: [itemId: string]
  pointerStart: [itemId: string, event: PointerEvent]
  returnItem: [itemId: string]
  changeItemRole: [itemId: string, targetRole: CardRole, targetPosition: number]
  changeSubject: [subject: string]
  crop: [itemId: string]
  revertCrop: [derivationId: string]
  requestAnswer: [draftId: string]
}>()

const flipped = ref(false)
const indexes = reactive<Record<CardRole, number>>({ question: 0, answer: 0 })
const expanded = ref<{ item: CaptureItemSummary, role: CardRole }>()
const itemById = computed(() => new Map(props.items.map(item => [item.id, item])))
const questionItems = computed(() => mapItems(props.draft.questionItemIds))
const answerItems = computed(() => mapItems(props.draft.answerItemIds))
const incompleteReason = computed(() => {
  if (props.draft.ready) return '就绪'
  const missing: string[] = []
  if (!questionItems.value.length) missing.push('缺题面')
  if (!answerItems.value.length) missing.push('缺答案')
  if (!props.draft.subject.trim()) missing.push('缺科目')
  return missing.join(' · ') || '未完成'
})

watch([questionItems, answerItems], ([questions, answers]) => {
  indexes.question = Math.min(indexes.question, Math.max(0, questions.length - 1))
  indexes.answer = Math.min(indexes.answer, Math.max(0, answers.length - 1))
  for (const item of [questions[indexes.question], answers[indexes.answer]]) {
    if (item) emit('preview', item.id)
  }
}, { immediate: true })

watch(() => props.revealRequestKey, () => {
  const itemId = props.revealItemId
  if (!itemId) return
  const questionIndex = questionItems.value.findIndex(item => item.id === itemId)
  const answerIndex = answerItems.value.findIndex(item => item.id === itemId)
  if (questionIndex >= 0) {
    indexes.question = questionIndex
    flipped.value = false
  } else if (answerIndex >= 0) {
    indexes.answer = answerIndex
    flipped.value = true
  } else {
    return
  }
  emit('select', props.draft.id)
  emit('preview', itemId)
}, { immediate: true, flush: 'post' })

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
  if (!item) return
  emit('preview', item.id)
  nextTick(() => document.querySelector<HTMLElement>(`[data-capture-item-id="${item.id}"]`)?.focus())
}

function flip() {
  emit('select', props.draft.id)
  if (!answerItems.value.length) {
    emit('requestAnswer', props.draft.id)
    return
  }
  flipped.value = !flipped.value
}

function openImage(role: CardRole) {
  const item = activeItem(role)
  if (item && props.previews[item.id]) expanded.value = { item, role }
}

function changeSubject(event: Event) {
  const select = event.target as HTMLSelectElement
  if (select.value && select.value !== props.draft.subject) emit('changeSubject', select.value)
}

function handleCardKeydown(event: KeyboardEvent) {
  if (!event.ctrlKey && !event.metaKey) return
  if (event.key === 'ArrowUp') {
    event.preventDefault()
    emit('navigateDraft', 'previous')
  } else if (event.key === 'ArrowDown') {
    event.preventDefault()
    emit('navigateDraft', 'next')
  }
}

function handleThumbnailKeydown(role: CardRole, index: number, item: CaptureItemSummary, event: KeyboardEvent) {
  if (event.currentTarget !== event.target) return
  const items = role === 'question' ? questionItems.value : answerItems.value
  if (event.shiftKey && (event.key === 'ArrowLeft' || event.key === 'ArrowRight')) {
    event.preventDefault()
    const delta = event.key === 'ArrowLeft' ? -1 : 1
    const targetPosition = Math.min(items.length - 1, Math.max(0, index + delta))
    if (targetPosition !== index) emit('changeItemRole', item.id, role, targetPosition)
    return
  }
  if (event.key === 'ArrowLeft' || event.key === 'ArrowUp') {
    event.preventDefault()
    selectImage(role, Math.max(0, index - 1))
  } else if (event.key === 'ArrowRight' || event.key === 'ArrowDown') {
    event.preventDefault()
    selectImage(role, Math.min(items.length - 1, index + 1))
  } else if (event.key.toLowerCase() === 'q' && role !== 'question') {
    event.preventDefault()
    emit('changeItemRole', item.id, 'question', questionItems.value.length)
  } else if (event.key.toLowerCase() === 'a' && role !== 'answer') {
    event.preventDefault()
    emit('changeItemRole', item.id, 'answer', answerItems.value.length)
  } else if (event.key === 'Backspace' || event.key === 'Delete') {
    event.preventDefault()
    emit('returnItem', item.id)
  }
}
</script>

<template>
  <article
    class="draft-card"
    :class="{
      'is-ready': draft.ready,
      'is-selected': selected,
      'is-drop-question': dropRole === 'question',
      'is-drop-answer': dropRole === 'answer',
      'is-settled': settled,
    }"
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
      <select
        class="card-subject"
        :value="draft.subject"
        :disabled="busy"
        :aria-label="`第 ${draftIndex + 1} 道题科目`"
        @click.stop
        @change="changeSubject"
      >
        <option
          v-if="!subjectOptions.includes(draft.subject)"
          :value="draft.subject"
        >
          {{ draft.subject || '选择科目' }}
        </option>
        <option
          v-for="subject in subjectOptions"
          :key="subject"
          :value="subject"
        >
          {{ subject }}
        </option>
      </select>
      <div class="draft-actions">
        <span
          class="ready-mark"
          :aria-label="draft.ready ? '题卡已完成' : incompleteReason"
          :title="draft.ready ? '题卡已完成' : incompleteReason"
        ><Check
          v-if="draft.ready"
          :size="13"
        /><template v-if="!draft.ready">{{ incompleteReason }}</template></span>
      </div>
    </header>

    <div class="card-perspective">
      <div
        class="card-inner"
        :class="{ 'is-flipped': flipped }"
        :role="answerItems.length ? 'button' : 'group'"
        tabindex="0"
        :aria-label="answerItems.length
          ? flipped ? '当前显示答案，点击翻回题面' : '当前显示题面，点击翻到答案'
          : '当前显示题面，尚未添加答案'"
        @click="answerItems.length && flip()"
        @keydown="handleCardKeydown"
        @keydown.enter.prevent="answerItems.length && flip()"
        @keydown.space.prevent="answerItems.length && flip()"
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
            :aria-label="`拖动 ${activeItem(role)!.sourceName}`"
            @pointerdown="!busy && emit('pointerStart', activeItem(role)!.id, $event)"
            @click.stop
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
              @pointerdown.stop
              @click.stop="openImage(role)"
            >
              <Maximize2 :size="16" /> 大图
            </button>
            <button
              type="button"
              class="crop-image"
              :disabled="busy"
              :aria-label="activeItem(role)!.cropDerivationId ? '恢复裁剪前原图' : `裁剪 ${activeItem(role)!.sourceName}`"
              @pointerdown.stop
              @click.stop="activeItem(role)!.cropDerivationId
                ? emit('revertCrop', activeItem(role)!.cropDerivationId!)
                : emit('crop', activeItem(role)!.id)"
            >
              <RotateCcw
                v-if="activeItem(role)!.cropDerivationId"
                :size="16"
              />
              <Crop
                v-else
                :size="16"
              />
              {{ activeItem(role)!.cropDerivationId ? '撤销裁剪' : '裁剪' }}
            </button>
            <div class="image-move-actions">
              <span class="drag-hint"><GripVertical :size="14" />可拖回素材区</span>
              <button
                type="button"
                class="return-image"
                :disabled="busy"
                @pointerdown.stop
                @click.stop="emit('returnItem', activeItem(role)!.id)"
              >
                将当前图片移回素材库
              </button>
            </div>
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
              cropable
              @preview="emit('preview', $event)"
              @crop="emit('crop', $event)"
              @revert-crop="emit('revertCrop', $event)"
              @activate="selectImage(role, index)"
              @pointer-start="(itemId, event) => emit('pointerStart', itemId, event)"
              @keydown="handleThumbnailKeydown(role, index, item, $event)"
            />
            <button
              type="button"
              class="change-role"
              :disabled="busy"
              :aria-label="`把当前${role === 'question' ? '题图转为答案' : '答案图转为题面'}`"
              :title="`转为${role === 'question' ? '答案' : '题面'}`"
              @click.stop="activeItem(role) && emit(
                'changeItemRole',
                activeItem(role)!.id,
                role === 'question' ? 'answer' : 'question',
                role === 'question' ? answerItems.length : questionItems.length,
              )"
            >
              <ArrowRightLeft :size="14" />
            </button>
          </div>
        </section>
      </div>
      <button
        v-if="answerItems.length"
        type="button"
        class="flip-button"
        :aria-label="flipped ? '翻回题面' : '翻到答案'"
        @click="flip"
      >
        <RotateCw :size="17" />{{ flipped ? '翻回题面' : '翻到答案' }}
      </button>
      <button
        v-else
        type="button"
        class="flip-button add-answer-button"
        :disabled="busy"
        @click="emit('requestAnswer', draft.id)"
      >
        <Plus :size="17" />添加答案
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
.draft-card.is-drop-question{border-color:rgba(33,51,45,.72);background:rgba(225,235,229,.9);transform:translateY(-3px) scale(1.012);box-shadow:0 22px 48px rgba(33,51,45,.18)}.draft-card.is-drop-answer{border-color:rgba(185,88,63,.72);background:rgba(247,225,216,.88);transform:translateY(-3px) scale(1.012);box-shadow:0 22px 48px rgba(185,88,63,.16)}.draft-card.is-settled{animation:card-settle 240ms var(--ease-standard)}
.draft-header{display:flex;gap:14px;align-items:center;justify-content:space-between}.draft-target{display:flex;flex:1;gap:10px;align-items:center;padding:0;color:var(--ink);border:0;background:transparent;text-align:left;cursor:pointer}.draft-target>span:last-child{display:grid}.draft-target strong{font-size:17px}.draft-target small{margin-top:2px;color:var(--ink-muted);font-size:10px}.draft-number{color:var(--cinnabar);font-family:serif;font-size:23px}.card-subject{min-height:34px;padding:0 28px 0 10px;color:var(--green-deep);border:1px solid rgba(33,51,45,.2);border-radius:999px;background:var(--green-soft);font-size:10px;font-weight:760;cursor:pointer}.draft-actions{display:flex;gap:7px;align-items:center}.ready-mark{display:inline-flex;gap:5px;align-items:center;min-width:28px;min-height:28px;justify-content:center;padding:6px 9px;color:#914b39;border-radius:999px;background:rgba(185,88,63,.1);font-size:9px;font-weight:800}.is-ready .ready-mark{color:#416b5a;background:var(--green-soft)}
.card-perspective{position:relative;margin-top:14px;perspective:1400px}.card-inner{position:relative;min-height:480px;transform-style:preserve-3d;transition:transform var(--motion-page) var(--ease-standard)}.card-inner.is-flipped{transform:rotateY(180deg)}.card-face{position:absolute;inset:0;display:grid;grid-template-rows:auto minmax(0,1fr) auto;min-height:440px;padding:14px;overflow:hidden;border:1px solid rgba(33,51,45,.13);border-radius:18px;background:rgba(246,241,231,.72);backface-visibility:hidden}.card-face.is-answer{background:linear-gradient(145deg,rgba(185,88,63,.08),rgba(255,253,247,.9));transform:rotateY(180deg)}
.face-label{display:flex;justify-content:space-between;align-items:center;margin-bottom:10px}.face-label span{color:var(--cinnabar);font-size:10px;font-weight:850;letter-spacing:.12em}.face-label strong{color:var(--ink-muted);font-size:10px}.main-image-wrap{position:relative;display:grid;min-height:340px;overflow:hidden;place-items:center;border-radius:13px;background:#fff;box-shadow:inset 0 0 0 1px rgba(33,51,45,.08);cursor:grab;touch-action:none}.main-image-wrap:active{cursor:grabbing}.main-image-wrap>img{width:100%;height:100%;max-height:430px;object-fit:contain}.image-loading,.face-empty{display:grid;place-items:center;align-content:center;gap:8px;min-height:340px;color:var(--ink-muted);text-align:center}.face-empty span{font-size:11px}.expand-image,.crop-image{position:absolute;bottom:10px;display:inline-flex;gap:5px;align-items:center;min-height:34px;padding:0 11px;color:var(--paper);border:0;border-radius:999px;background:rgba(33,51,45,.82);font-size:10px;cursor:pointer}.expand-image{right:10px}.crop-image{left:10px;background:rgba(185,88,63,.9)}.drag-hint{position:absolute;top:10px;right:10px;display:inline-flex;gap:4px;align-items:center;padding:5px 8px;color:var(--paper);border-radius:999px;background:rgba(33,51,45,.72);font-size:9px;font-weight:760;pointer-events:none}.face-filmstrip{display:flex;gap:7px;align-items:center;min-width:0;margin-top:10px;padding:2px;overflow-x:auto}.change-role{display:grid;flex:0 0 auto;width:32px;height:32px;place-items:center;color:var(--green-deep);border:1px solid rgba(33,51,45,.22);border-radius:50%;background:var(--green-soft);cursor:pointer}.flip-button{position:absolute;z-index:3;right:18px;bottom:18px;display:flex;gap:7px;align-items:center;min-height:42px;padding:0 15px;color:var(--paper);border:0;border-radius:999px;background:var(--green-deep);font-weight:780;cursor:pointer;box-shadow:0 10px 24px rgba(33,51,45,.2)}
.image-move-actions{position:absolute;top:10px;right:10px;display:flex;gap:6px;align-items:center}.image-move-actions .drag-hint{position:static}.return-image{min-height:29px;padding:0 9px;color:var(--green-deep);border:1px solid rgba(33,51,45,.18);border-radius:999px;background:rgba(255,253,247,.94);font-size:9px;font-weight:760;cursor:pointer}.add-answer-button{background:var(--cinnabar)}
.image-overlay{position:fixed;z-index:120;inset:0;display:grid;padding:32px;place-items:center;background:rgba(20,28,25,.82);backdrop-filter:blur(14px)}.image-overlay section{position:relative;display:grid;width:min(1400px,100%);height:min(900px,calc(100vh - 64px));padding:16px;grid-template-rows:minmax(0,1fr) auto;border-radius:18px;background:var(--paper)}.image-overlay img{width:100%;height:100%;object-fit:contain}.image-overlay p{min-width:0;margin:10px 42px 0 0;overflow:hidden;text-overflow:ellipsis;white-space:nowrap}.image-overlay button{position:absolute;z-index:1;top:14px;right:14px;display:grid;width:40px;height:40px;place-items:center;color:var(--paper);border:0;border-radius:50%;background:var(--green-deep);cursor:pointer}button:disabled{cursor:not-allowed;opacity:.42}
@keyframes card-settle{0%{transform:scale(.97);opacity:.78}55%{transform:scale(1.018)}100%{transform:scale(1);opacity:1}}
@media(max-width:760px){.draft-card{padding:14px}.draft-header{align-items:flex-start;flex-wrap:wrap}.card-subject{order:3}.card-inner{min-height:420px}.card-face{min-height:390px}.main-image-wrap,.image-loading,.face-empty{min-height:290px}.image-overlay{padding:12px}.image-overlay section{height:calc(100vh - 24px)}}
@media(prefers-reduced-motion:reduce){.draft-card,.card-inner{transition:none}.draft-card.is-selected,.draft-card.is-drop-question,.draft-card.is-drop-answer{transform:none}.draft-card.is-settled{animation:none}}
</style>
