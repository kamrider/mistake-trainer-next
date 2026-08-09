<script setup lang="ts">
import { BookOpen, Plus, Trash2, Volume2 } from '@lucide/vue'
import type { SubjectPreferences } from '../../shared/api/bindings'

const props = defineProps<{
  preferences: SubjectPreferences
  builtinSubjects: readonly string[]
  customSubject: string
  saving: boolean
  message: string
}>()

const emit = defineEmits<{
  toggleSubject: [subject: string, enabled: boolean]
  updateCustomSubject: [value: string]
  addCustomSubject: []
  removeCustomSubject: [subject: string]
  updateCaptureSound: [enabled: boolean]
  save: []
}>()

function isSubjectEnabled(subject: string) {
  return props.preferences.enabledSubjects.includes(subject)
}

function isLastEnabledSubject(subject: string) {
  return props.preferences.enabledSubjects.length === 1 && isSubjectEnabled(subject)
}

function updateSubject(subject: string, event: Event) {
  emit('toggleSubject', subject, (event.target as HTMLInputElement).checked)
}

function updateCustomSubject(event: Event) {
  emit('updateCustomSubject', (event.target as HTMLInputElement).value)
}

function updateCaptureSound(event: Event) {
  emit('updateCaptureSound', (event.target as HTMLInputElement).checked)
}
</script>

<template>
  <section
    id="settings-subjects"
    class="subject-panel"
    aria-labelledby="subject-settings-title"
  >
    <header>
      <div>
        <p>采集偏好 · 当前学习档案</p>
        <h2 id="subject-settings-title">
          常用科目
        </h2>
        <span>勾选会出现在采集台顶部；一键即可把整批题统一设置为同一科目。</span>
      </div>
      <BookOpen :size="24" />
    </header>

    <div class="builtin-subjects">
      <label
        v-for="subject in builtinSubjects"
        :key="subject"
        :class="{ selected: isSubjectEnabled(subject) }"
      >
        <input
          type="checkbox"
          :value="subject"
          :checked="isSubjectEnabled(subject)"
          :disabled="isLastEnabledSubject(subject)"
          @change="updateSubject(subject, $event)"
        >
        <span>{{ subject }}</span>
      </label>
    </div>
    <p
      id="subject-invariant-note"
      class="subject-invariant-note"
    >
      至少保留一个常用科目；最后一个已选科目会保持启用。
    </p>

    <div class="custom-subjects">
      <span
        v-for="subject in preferences.customSubjects"
        :key="subject"
      >{{ subject }}<button
        type="button"
        :aria-label="`删除自定义科目 ${subject}`"
        @click="emit('removeCustomSubject', subject)"
      ><Trash2 :size="13" /></button></span>
    </div>

    <div class="subject-controls">
      <form @submit.prevent="emit('addCustomSubject')">
        <input
          :value="customSubject"
          aria-label="自定义科目名称"
          :aria-describedby="message ? 'subject-message' : undefined"
          maxlength="40"
          placeholder="例如：编程、竞赛数学"
          @input="updateCustomSubject"
        >
        <button
          type="submit"
          aria-label="添加自定义科目"
          :disabled="!customSubject.trim()"
        >
          <Plus :size="15" />添加
        </button>
      </form>

      <label class="sound-toggle">
        <input
          type="checkbox"
          :checked="preferences.captureSoundEnabled"
          @change="updateCaptureSound"
        >
        <Volume2 :size="16" /><span><strong>拖放成功音效</strong><small>仅在题卡融合成功后播放短提示音</small></span>
      </label>

      <button
        type="button"
        class="save-subjects"
        :disabled="saving"
        @click="emit('save')"
      >
        {{ saving ? '保存中…' : '保存科目配置' }}
      </button>
    </div>

    <p
      v-if="message"
      id="subject-message"
      class="subject-message"
      role="status"
    >
      {{ message }}
    </p>
  </section>
</template>

<style scoped>
.subject-panel {
  margin-top: 16px;
  padding: 26px;
  border: 1px solid var(--line);
  border-radius: 17px;
  background: rgba(255, 253, 247, .78);
  box-shadow: 0 16px 48px rgba(34, 48, 43, .05);
}

.subject-panel > header {
  display: flex;
  align-items: center;
  justify-content: space-between;
  gap: 20px;
  margin-bottom: 18px;
}

.subject-panel > header p {
  margin: 0 0 8px;
  color: var(--cinnabar);
  font-size: 12px;
  font-weight: 800;
  letter-spacing: .14em;
}

.subject-panel h2 {
  margin: 0;
  color: var(--green-deep);
  font-family: Georgia, 'Microsoft YaHei', serif;
  font-size: 21px;
}

.subject-panel > header span {
  display: block;
  margin-top: 8px;
  color: var(--ink-muted);
  font-size: 12px;
  line-height: 1.7;
}

.subject-panel > header > svg {
  flex: 0 0 auto;
  color: var(--green-deep);
}

button {
  display: inline-flex;
  min-height: 44px;
  gap: 7px;
  align-items: center;
  padding: 10px 14px;
  border: 1px solid var(--line);
  border-radius: 10px;
  background: var(--paper-raised);
  cursor: pointer;
}

button:disabled {
  opacity: .5;
}

.builtin-subjects {
  display: flex;
  gap: 9px;
  flex-wrap: wrap;
}

.builtin-subjects label {
  position: relative;
  cursor: pointer;
}

.builtin-subjects input {
  position: absolute;
  width: 1px;
  height: 1px;
  opacity: 0;
}

.builtin-subjects span {
  display: grid;
  min-width: 58px;
  min-height: 44px;
  padding: 0 13px;
  place-items: center;
  color: var(--ink-muted);
  border: 1px solid var(--line);
  border-radius: 999px;
  background: var(--paper);
  transition:
    transform var(--motion-feedback),
    color var(--motion-feedback),
    background var(--motion-feedback);
}

.builtin-subjects label.selected span {
  color: var(--paper);
  border-color: var(--green-deep);
  background: var(--green-deep);
}

.builtin-subjects label:hover span {
  transform: translateY(-1px);
}

.builtin-subjects input:focus-visible + span {
  outline: 3px solid rgba(185, 88, 63, .24);
  outline-offset: 2px;
}

.builtin-subjects input:disabled + span {
  cursor: not-allowed;
  opacity: .72;
}

.custom-subjects {
  display: flex;
  gap: 8px;
  flex-wrap: wrap;
  min-height: 8px;
  margin-top: 12px;
}

.subject-invariant-note {
  margin: 9px 0 0;
  color: var(--ink-muted);
  font-size: 12px;
  line-height: 1.6;
}

.custom-subjects > span {
  display: inline-flex;
  gap: 5px;
  align-items: center;
  padding: 6px 7px 6px 11px;
  color: #7e412f;
  border-radius: 999px;
  background: rgba(185, 88, 63, .1);
  font-size: 12px;
}

.custom-subjects button {
  display: grid;
  min-width: 44px;
  min-height: 44px;
  padding: 0;
  place-items: center;
  border: 0;
  border-radius: 50%;
  background: transparent;
}

.subject-controls {
  display: grid;
  grid-template-columns: minmax(280px, 1fr) minmax(250px, 1fr) auto;
  gap: 12px;
  align-items: center;
  margin-top: 17px;
}

.subject-controls form {
  display: flex;
  gap: 8px;
}

.subject-controls form input {
  flex: 1;
  min-width: 0;
  min-height: 44px;
  padding: 10px 12px;
  border: 1px solid var(--line);
  border-radius: 10px;
  background: var(--paper);
}

.sound-toggle {
  display: flex;
  min-height: 44px;
  gap: 9px;
  align-items: center;
  padding: 8px 11px;
  border: 1px solid var(--line);
  border-radius: 11px;
  cursor: pointer;
}

.sound-toggle > span {
  display: grid;
}

.sound-toggle small {
  color: var(--ink-muted);
  font-size: 12px;
}

.save-subjects {
  color: var(--paper);
  border-color: var(--green-deep);
  background: var(--green-deep);
}

.subject-message {
  margin: 12px 0 0;
  color: #557263;
  font-size: 12px;
}

@media (max-width: 980px) {
  .subject-controls {
    grid-template-columns: 1fr;
  }
}

@media (max-width: 760px) {
  .subject-panel {
    padding: 22px;
  }

  .subject-panel > header {
    align-items: flex-start;
  }

  .subject-panel button,
  .subject-panel input {
    min-height: 44px;
  }

  .builtin-subjects span {
    min-height: 44px;
  }

  .custom-subjects {
    gap: 10px;
  }

  .custom-subjects > span {
    font-size: 12px;
  }

  .custom-subjects button {
    min-width: 44px;
    min-height: 44px;
  }

  .sound-toggle small {
    font-size: 12px;
  }
}

@media (prefers-reduced-motion: reduce) {
  .builtin-subjects span {
    transition: none;
  }

  .builtin-subjects label:hover span {
    transform: none;
  }
}
</style>
