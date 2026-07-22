<script setup lang="ts">
import { Check, ChevronDown, CircleUserRound, Pencil, Plus, ShieldAlert, Trash2, X } from '@lucide/vue'
import { computed, nextTick, onBeforeUnmount, onMounted, ref, watch } from 'vue'
import type { ProfileSummary } from '../../../shared/api/bindings'

const props = defineProps<{
  profiles: ProfileSummary[]
  activeProfileId: string
  busy: boolean
  errorMessage: string
}>()

const emit = defineEmits<{
  select: [profileId: string]
  create: [name: string]
  rename: [profileId: string, name: string]
  delete: [profileId: string, confirmationName: string]
  retry: []
}>()

const root = ref<HTMLElement>()
const trigger = ref<HTMLButtonElement>()
const nameInput = ref<HTMLInputElement>()
const open = ref(false)
const mode = ref<'list' | 'create' | 'rename' | 'delete'>('list')
const editingProfileId = ref('')
const draftName = ref('')
const localError = ref('')

const activeProfile = computed(() =>
  props.profiles.find(profile => profile.id === props.activeProfileId),
)
const editingProfile = computed(() =>
  props.profiles.find(profile => profile.id === editingProfileId.value),
)
const deletionConfirmed = computed(() =>
  mode.value === 'delete' && draftName.value === editingProfile.value?.name,
)
const currentName = computed(() => activeProfile.value?.name ?? props.profiles[0]?.name ?? '学习档案')

function toggle() {
  if (props.busy) return
  open.value = !open.value
  if (!open.value) resetForm()
}

function resetForm() {
  mode.value = 'list'
  editingProfileId.value = ''
  draftName.value = ''
  localError.value = ''
}

async function beginCreate() {
  mode.value = 'create'
  draftName.value = ''
  localError.value = ''
  await nextTick()
  nameInput.value?.focus()
}

async function beginRename(profile: ProfileSummary) {
  mode.value = 'rename'
  editingProfileId.value = profile.id
  draftName.value = profile.name
  localError.value = ''
  await nextTick()
  nameInput.value?.focus()
  nameInput.value?.select()
}

async function beginDelete(profile: ProfileSummary) {
  mode.value = 'delete'
  editingProfileId.value = profile.id
  draftName.value = ''
  localError.value = ''
  await nextTick()
  nameInput.value?.focus()
}

function submitName() {
  const name = draftName.value.trim()
  if (!name) {
    localError.value = '请输入档案名称。'
    nameInput.value?.focus()
    return
  }
  if ([...name].length > 40) {
    localError.value = '档案名称不能超过 40 个字。'
    nameInput.value?.focus()
    return
  }
  localError.value = ''
  if (mode.value === 'create') emit('create', name)
  if (mode.value === 'rename') emit('rename', editingProfileId.value, name)
}

function submitDelete() {
  if (!editingProfile.value || !deletionConfirmed.value || props.busy) return
  localError.value = ''
  emit('delete', editingProfile.value.id, draftName.value)
}

function selectProfile(profileId: string) {
  if (profileId === props.activeProfileId || props.busy) return
  emit('select', profileId)
}

function close({ restoreFocus = false } = {}) {
  open.value = false
  resetForm()
  if (restoreFocus) nextTick(() => trigger.value?.focus())
}

function handleDocumentPointer(event: PointerEvent) {
  if (open.value && !root.value?.contains(event.target as Node)) close()
}

function handleKeydown(event: KeyboardEvent) {
  if (event.key === 'Escape' && open.value) {
    event.preventDefault()
    close({ restoreFocus: true })
  }
}

watch(
  () => props.busy,
  (busy, wasBusy) => {
    if (wasBusy && !busy && !props.errorMessage) close({ restoreFocus: true })
  },
)

onMounted(() => {
  document.addEventListener('pointerdown', handleDocumentPointer)
  document.addEventListener('keydown', handleKeydown)
})
onBeforeUnmount(() => {
  document.removeEventListener('pointerdown', handleDocumentPointer)
  document.removeEventListener('keydown', handleKeydown)
})
</script>

<template>
  <div
    ref="root"
    class="profile-switcher"
  >
    <button
      ref="trigger"
      class="profile-trigger"
      type="button"
      aria-controls="profile-switcher-panel"
      :aria-expanded="open"
      :aria-label="`当前学习档案：${currentName}。打开档案菜单`"
      :disabled="busy"
      @click="toggle"
    >
      <CircleUserRound
        :size="21"
        aria-hidden="true"
      />
      <span class="profile-copy"><strong>{{ currentName }}</strong><small>私人学习档案</small></span>
      <ChevronDown
        class="chevron"
        :class="{ rotated: open }"
        :size="16"
        aria-hidden="true"
      />
    </button>

    <Transition name="profile-popover">
      <section
        v-if="open"
        id="profile-switcher-panel"
        class="profile-panel"
        role="dialog"
        aria-label="切换学习档案"
      >
        <header>
          <div>
            <strong>学习档案</strong>
            <small>题库、训练与统计彼此独立</small>
          </div>
          <button
            class="icon-button"
            type="button"
            aria-label="关闭档案菜单"
            @click="close({ restoreFocus: true })"
          >
            <X :size="16" />
          </button>
        </header>

        <Transition
          name="profile-mode"
          mode="out-in"
        >
          <div
            v-if="mode === 'list'"
            key="list"
            class="profile-list"
          >
            <div
              v-for="profile in profiles"
              :key="profile.id"
              class="profile-row"
              :class="{ selected: profile.id === activeProfileId, deletable: profiles.length > 1 }"
            >
              <button
                class="select-profile"
                type="button"
                :disabled="busy"
                :aria-label="profile.id === activeProfileId ? `当前档案：${profile.name}` : `切换到档案：${profile.name}`"
                :aria-current="profile.id === activeProfileId ? 'true' : undefined"
                @click="selectProfile(profile.id)"
              >
                <span class="profile-avatar">{{ profile.name.slice(0, 1) }}</span>
                <span><strong>{{ profile.name }}</strong><small>{{ profile.id === activeProfileId ? '当前使用' : '切换到此档案' }}</small></span>
                <Check
                  v-if="profile.id === activeProfileId"
                  :size="17"
                  aria-hidden="true"
                />
              </button>
              <button
                class="rename-button"
                type="button"
                :aria-label="`重命名档案：${profile.name}`"
                :disabled="busy"
                @click="beginRename(profile)"
              >
                <Pencil :size="14" />
              </button>
              <button
                v-if="profiles.length > 1"
                class="delete-button"
                type="button"
                :aria-label="`删除档案：${profile.name}`"
                :disabled="busy"
                @click="beginDelete(profile)"
              >
                <Trash2 :size="14" />
              </button>
            </div>
            <button
              class="create-button"
              type="button"
              :disabled="busy"
              @click="beginCreate"
            >
              <Plus :size="16" />新建学习档案
            </button>
          </div>

          <form
            v-else-if="mode === 'create' || mode === 'rename'"
            :key="mode"
            class="profile-form"
            @submit.prevent="submitName"
          >
            <label for="profile-name">{{ mode === 'create' ? '新档案名称' : '重命名档案' }}</label>
            <input
              id="profile-name"
              ref="nameInput"
              v-model="draftName"
              maxlength="40"
              autocomplete="off"
              :disabled="busy"
            >
            <p
              v-if="localError || errorMessage"
              class="profile-error"
              role="alert"
            >
              {{ localError || errorMessage }}
            </p>
            <div class="form-actions">
              <button
                type="button"
                :disabled="busy"
                @click="resetForm"
              >
                取消
              </button>
              <button
                class="confirm-button"
                type="submit"
                :disabled="busy"
              >
                {{ busy ? '正在保存…' : '保存' }}
              </button>
            </div>
          </form>

          <form
            v-else
            key="delete"
            class="profile-form delete-form"
            @submit.prevent="submitDelete"
          >
            <div class="danger-heading">
              <span><ShieldAlert :size="18" /></span>
              <div>
                <strong>永久删除“{{ editingProfile?.name }}”？</strong>
                <small id="profile-delete-impact">该档案的错题、采集草稿、训练记录和导出快照都会删除，无法撤销。</small>
              </div>
            </div>
            <label for="profile-delete-confirmation">输入下方档案名确认删除</label>
            <code class="confirmation-chip">{{ editingProfile?.name }}</code>
            <input
              id="profile-delete-confirmation"
              ref="nameInput"
              v-model="draftName"
              autocomplete="off"
              spellcheck="false"
              :aria-label="`输入“${editingProfile?.name ?? ''}”确认删除`"
              aria-describedby="profile-delete-impact"
              :disabled="busy"
            >
            <p
              v-if="errorMessage"
              class="profile-error"
              role="alert"
            >
              {{ errorMessage }}
            </p>
            <div class="form-actions">
              <button
                type="button"
                :disabled="busy"
                @click="resetForm"
              >
                保留档案
              </button>
              <button
                class="danger-button"
                type="submit"
                :disabled="busy || !deletionConfirmed"
              >
                {{ busy ? '正在删除…' : '永久删除档案' }}
              </button>
            </div>
          </form>
        </Transition>

        <p
          v-if="mode === 'list' && errorMessage"
          class="profile-error panel-error"
          role="alert"
        >
          {{ errorMessage }}
        </p>
        <button
          v-if="mode === 'list' && errorMessage"
          class="retry-button"
          type="button"
          :disabled="busy"
          @click="$emit('retry')"
        >
          重新读取档案
        </button>
      </section>
    </Transition>
  </div>
</template>

<style scoped>
.profile-switcher { position: relative; }
.profile-trigger { display: flex; gap: 10px; align-items: center; width: 100%; padding: 12px; color: var(--ink); border: 1px solid var(--line); border-radius: 13px; background: rgba(255,253,247,.72); cursor: pointer; text-align: left; transition: border-color var(--motion-feedback) var(--ease-standard), background var(--motion-feedback) var(--ease-standard), transform var(--motion-feedback) var(--ease-standard); }
.profile-trigger:hover { border-color: rgba(33,51,45,.32); background: var(--paper-light); }
.profile-trigger:active { transform: scale(.985); }
.profile-trigger:disabled { cursor: wait; opacity: .7; }
.profile-copy { min-width: 0; flex: 1; }
.profile-copy strong, .profile-copy small { display: block; overflow: hidden; text-overflow: ellipsis; white-space: nowrap; }
.profile-copy strong { font-size: 13px; }
.profile-copy small { margin-top: 3px; color: var(--ink-muted); font-size: 10px; }
.chevron { flex: 0 0 auto; color: var(--ink-muted); transition: transform var(--motion-standard) var(--ease-standard); }
.chevron.rotated { transform: rotate(180deg); }
.profile-panel { position: absolute; z-index: 40; bottom: calc(100% + 10px); left: 0; width: 300px; overflow: hidden; border: 1px solid rgba(65,73,65,.18); border-radius: 16px; background: rgba(255,253,247,.98); box-shadow: 0 18px 55px rgba(41,45,38,.18); transform-origin: left bottom; backdrop-filter: blur(20px); }
.profile-panel header { display: flex; align-items: flex-start; justify-content: space-between; padding: 16px 16px 12px; border-bottom: 1px solid var(--line); }
.profile-panel header strong, .profile-panel header small { display: block; }
.profile-panel header strong { font-family: Georgia, 'Songti SC', serif; font-size: 16px; }
.profile-panel header small { margin-top: 4px; color: var(--ink-muted); font-size: 11px; }
.icon-button, .rename-button, .delete-button { display: grid; place-items: center; color: var(--ink-muted); border: 0; border-radius: 8px; background: transparent; cursor: pointer; }
.icon-button { width: 28px; height: 28px; }
.icon-button:hover, .rename-button:hover { color: var(--ink); background: var(--sand-soft); }
.delete-button:hover { color: var(--cinnabar); background: rgba(185,88,63,.1); }
.profile-list { display: grid; gap: 6px; max-height: 310px; padding: 10px; overflow-y: auto; }
.profile-row { display: grid; grid-template-columns: minmax(0,1fr) 32px; align-items: center; border: 1px solid transparent; border-radius: 12px; transition: border-color var(--motion-feedback) var(--ease-standard), background var(--motion-feedback) var(--ease-standard); }
.profile-row.deletable { grid-template-columns: minmax(0,1fr) 32px 32px; }
.profile-row:hover { background: rgba(232,221,199,.38); }
.profile-row.selected { border-color: rgba(33,51,45,.16); background: var(--green-soft); }
.select-profile { display: grid; grid-template-columns: 34px minmax(0,1fr) 18px; gap: 9px; align-items: center; min-width: 0; padding: 9px; color: var(--ink); border: 0; background: transparent; cursor: pointer; text-align: left; }
.select-profile:disabled, .rename-button:disabled, .delete-button:disabled, .create-button:disabled { cursor: wait; opacity: .65; }
.profile-avatar { display: grid; width: 34px; height: 34px; place-items: center; color: var(--green-deep); border-radius: 10px; background: rgba(255,253,247,.78); font-family: Georgia, 'Songti SC', serif; font-weight: 700; }
.select-profile strong, .select-profile small { display: block; overflow: hidden; text-overflow: ellipsis; white-space: nowrap; }
.select-profile strong { font-size: 13px; }
.select-profile small { margin-top: 3px; color: var(--ink-muted); font-size: 10px; }
.rename-button, .delete-button { width: 30px; height: 30px; }
.create-button { display: flex; gap: 8px; align-items: center; justify-content: center; min-height: 40px; margin-top: 3px; color: var(--green-deep); border: 1px dashed rgba(33,51,45,.28); border-radius: 11px; background: transparent; cursor: pointer; font-weight: 700; }
.create-button:hover { background: var(--green-soft); }
.profile-form { display: grid; gap: 9px; padding: 16px; }
.profile-form label { font-size: 12px; font-weight: 700; }
.profile-form input { width: 100%; min-height: 41px; padding: 0 11px; color: var(--ink); border: 1px solid var(--line-strong); border-radius: 10px; outline: none; background: var(--paper-light); }
.profile-form input:focus { border-color: var(--green-deep); box-shadow: 0 0 0 3px rgba(33,51,45,.1); }
.delete-form input:focus { border-color: var(--cinnabar); box-shadow: 0 0 0 3px rgba(185,88,63,.1); }
.danger-heading { display: grid; grid-template-columns: 34px minmax(0,1fr); gap: 10px; align-items: start; padding: 11px; border: 1px solid rgba(185,88,63,.2); border-radius: 11px; background: rgba(185,88,63,.07); }
.danger-heading > span { display: grid; width: 34px; height: 34px; place-items: center; color: var(--cinnabar); border-radius: 9px; background: rgba(255,253,247,.8); }
.danger-heading strong, .danger-heading small { display: block; }
.danger-heading strong { font-size: 13px; }
.danger-heading small { margin-top: 4px; color: var(--ink-muted); font-size: 10px; line-height: 1.55; }
.confirmation-chip { justify-self: start; max-width: 100%; overflow: hidden; padding: 5px 8px; color: var(--cinnabar); border-radius: 7px; background: rgba(185,88,63,.08); font-family: inherit; font-size: 12px; font-weight: 700; text-overflow: ellipsis; white-space: nowrap; }
.profile-error { margin: 0; color: var(--cinnabar); font-size: 11px; line-height: 1.55; }
.panel-error { padding: 0 16px 14px; }
.retry-button { min-height: 34px; margin: 0 16px 14px; padding: 0 12px; color: var(--green-deep); border: 1px solid rgba(33,51,45,.24); border-radius: 9px; background: var(--green-soft); cursor: pointer; font-weight: 700; }
.form-actions { display: flex; gap: 8px; justify-content: flex-end; margin-top: 3px; }
.form-actions button { min-height: 34px; padding: 0 13px; color: var(--ink); border: 1px solid var(--line); border-radius: 9px; background: transparent; cursor: pointer; }
.form-actions .confirm-button { color: white; border-color: var(--green-deep); background: var(--green-deep); }
.form-actions .danger-button { color: white; border-color: var(--cinnabar); background: var(--cinnabar); }
.form-actions .danger-button:disabled { cursor: not-allowed; opacity: .42; }
.profile-popover-enter-active, .profile-popover-leave-active { transition: opacity var(--motion-standard) var(--ease-standard), transform var(--motion-standard) var(--ease-standard); }
.profile-popover-enter-from, .profile-popover-leave-to { opacity: 0; transform: translateY(7px) scale(.97); }
.profile-mode-enter-active, .profile-mode-leave-active { transition: opacity var(--motion-standard) var(--ease-standard), transform var(--motion-standard) var(--ease-standard); }
.profile-mode-enter-from { opacity: 0; transform: translateX(8px); }
.profile-mode-leave-to { opacity: 0; transform: translateX(-8px); }
@media (prefers-reduced-motion: reduce) { .profile-popover-enter-active, .profile-popover-leave-active, .profile-mode-enter-active, .profile-mode-leave-active, .chevron { transition-duration: 1ms; } }
</style>
