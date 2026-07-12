<script setup lang="ts">
import { computed, ref } from 'vue'
import { Check, FileImage, ImagePlus, LockKeyhole, Trash2 } from '@lucide/vue'
import type { CaptureCommitInput, StagedAsset } from '../../../shared/api/bindings'

const props = defineProps<{
  assets: StagedAsset[]
  saving: boolean
  errorMessage?: string
}>()

const emit = defineEmits<{
  select: [role: 'question' | 'answer']
  remove: [stagedAssetId: string]
  commit: [input: CaptureCommitInput]
}>()

const subject = ref('')
const note = ref('')
const questions = computed(() => props.assets.filter(asset => asset.role === 'question'))
const answers = computed(() => props.assets.filter(asset => asset.role === 'answer'))
const ready = computed(() => questions.value.length > 0 && answers.value.length > 0)

function submit() {
  if (!ready.value || props.saving) return
  emit('commit', {
    subject: subject.value.trim(),
    note: note.value.trim(),
    stagedAssetIds: props.assets.map(asset => asset.id),
  })
}
</script>

<template>
  <main
    class="capture-workspace"
    aria-labelledby="capture-title"
  >
    <header class="capture-header">
      <div>
        <p class="eyebrow">
          采集整理 · 本机加密暂存
        </p>
        <h1 id="capture-title">
          录入错题
        </h1>
        <p class="intro">
          先放下题目，再放下答案。保存时它们才会成为题库里完整的一道题。
        </p>
      </div>
      <div class="privacy-note">
        <LockKeyhole
          :size="17"
          aria-hidden="true"
        />
        文件路径不会交给页面
      </div>
    </header>

    <p
      v-if="errorMessage"
      class="error-banner"
      role="alert"
    >
      {{ errorMessage }}
    </p>

    <form @submit.prevent="submit">
      <section
        class="asset-columns"
        aria-label="题目与答案图片"
      >
        <article class="asset-panel question-panel">
          <div class="panel-heading">
            <span class="step-mark">一</span>
            <div>
              <h2>题目</h2>
              <p>可一次选择多张，按选择顺序组成题目。</p>
            </div>
          </div>
          <ul
            v-if="questions.length"
            class="asset-list"
          >
            <li
              v-for="asset in questions"
              :key="asset.id"
            >
              <FileImage
                :size="19"
                aria-hidden="true"
              />
              <span>
                <strong>{{ asset.fileName }}</strong>
                <small>{{ asset.width }} × {{ asset.height }}</small>
              </span>
              <button
                type="button"
                :aria-label="`移除 ${asset.fileName}`"
                @click="emit('remove', asset.id)"
              >
                <Trash2
                  :size="16"
                  aria-hidden="true"
                />
              </button>
            </li>
          </ul>
          <div
            v-else
            class="panel-empty"
          >
            <ImagePlus
              :size="30"
              aria-hidden="true"
            />
            <span>还没有题图</span>
          </div>
          <button
            class="select-button"
            type="button"
            @click="emit('select', 'question')"
          >
            <ImagePlus
              :size="18"
              aria-hidden="true"
            />
            选择题图
          </button>
        </article>

        <article class="asset-panel answer-panel">
          <div class="panel-heading">
            <span class="step-mark">二</span>
            <div>
              <h2>答案</h2>
              <p>解析、订正过程和标准答案都可以放在这里。</p>
            </div>
          </div>
          <ul
            v-if="answers.length"
            class="asset-list"
          >
            <li
              v-for="asset in answers"
              :key="asset.id"
            >
              <FileImage
                :size="19"
                aria-hidden="true"
              />
              <span>
                <strong>{{ asset.fileName }}</strong>
                <small>{{ asset.width }} × {{ asset.height }}</small>
              </span>
              <button
                type="button"
                :aria-label="`移除 ${asset.fileName}`"
                @click="emit('remove', asset.id)"
              >
                <Trash2
                  :size="16"
                  aria-hidden="true"
                />
              </button>
            </li>
          </ul>
          <div
            v-else
            class="panel-empty"
          >
            <ImagePlus
              :size="30"
              aria-hidden="true"
            />
            <span>还没有答案图</span>
          </div>
          <button
            class="select-button"
            type="button"
            @click="emit('select', 'answer')"
          >
            <ImagePlus
              :size="18"
              aria-hidden="true"
            />
            选择答案图
          </button>
        </article>
      </section>

      <section class="details-card">
        <div class="details-heading">
          <span class="step-mark">三</span>
          <div>
            <h2>整理线索</h2>
            <p>留下下次复习时真正有用的一句话。</p>
          </div>
        </div>
        <div class="form-grid">
          <label>
            <span>科目</span>
            <input
              v-model="subject"
              required
              maxlength="40"
              placeholder="例如：数学"
            >
          </label>
          <label>
            <span>错因或笔记</span>
            <textarea
              v-model="note"
              maxlength="500"
              rows="3"
              placeholder="我在哪里绕远了？下次看见什么信号要停一下？"
            />
          </label>
        </div>
      </section>

      <footer class="capture-footer">
        <p v-if="!ready">
          {{ questions.length === 0 ? '还需要至少一张题图' : '还需要至少一张答案图' }}
        </p>
        <p
          v-else
          class="ready-copy"
        >
          <Check
            :size="17"
            aria-hidden="true"
          />
          题目与答案已经配齐
        </p>
        <button
          class="save-button"
          type="submit"
          :disabled="!ready || saving"
        >
          {{ saving ? '正在加密保存…' : '保存到题库' }}
        </button>
      </footer>
    </form>
  </main>
</template>

<style scoped>
.capture-workspace { max-width: 1120px; margin: 0 auto; padding: 54px 50px 76px; }
.capture-header { display: flex; justify-content: space-between; gap: 30px; align-items: flex-end; }
.eyebrow { margin: 0 0 8px; color: var(--cinnabar); font-size: 12px; font-weight: 760; letter-spacing: .13em; }
h1 { margin: 0; font-size: clamp(42px,5vw,64px); letter-spacing: -.055em; line-height: 1; }
.intro { margin: 15px 0 0; color: var(--ink-muted); font-size: 16px; }
.privacy-note { display: inline-flex; gap: 8px; align-items: center; padding: 10px 13px; color: #50665d; border: 1px solid rgba(33,51,45,.13); border-radius: 999px; background: rgba(255,253,247,.55); font-size: 12px; }
.error-banner { margin: 24px 0 0; padding: 12px 15px; color: #7f3829; border: 1px solid rgba(185,88,63,.28); border-radius: 10px; background: rgba(185,88,63,.08); }
.asset-columns { display: grid; grid-template-columns: repeat(2,minmax(0,1fr)); gap: 18px; margin-top: 40px; }
.asset-panel, .details-card { border: 1px solid var(--line); background: rgba(255,253,247,.72); box-shadow: var(--shadow-soft); }
.asset-panel { display: flex; min-height: 390px; padding: 26px; border-radius: 5px 22px 22px; flex-direction: column; }
.answer-panel { background: linear-gradient(145deg,rgba(255,253,247,.72),rgba(232,221,199,.28)); }
.panel-heading, .details-heading { display: flex; gap: 13px; align-items: flex-start; }
.step-mark { display: grid; width: 31px; height: 31px; flex: 0 0 auto; place-items: center; color: var(--paper); border-radius: 50%; background: var(--green-deep); font-family: serif; font-weight: 700; }
h2 { margin: 0; font-size: 22px; letter-spacing: -.025em; }
.panel-heading p, .details-heading p { margin: 5px 0 0; color: var(--ink-muted); font-size: 12px; line-height: 1.5; }
.panel-empty { display: grid; flex: 1; min-height: 155px; place-content: center; justify-items: center; gap: 12px; color: #73847d; }
.asset-list { display: grid; gap: 9px; margin: 25px 0; padding: 0; list-style: none; }
.asset-list li { display: grid; grid-template-columns: auto minmax(0,1fr) auto; gap: 11px; align-items: center; padding: 13px; border: 1px solid rgba(33,51,45,.1); border-radius: 12px; background: rgba(246,241,231,.72); }
.asset-list span { display: grid; min-width: 0; }
.asset-list strong { overflow: hidden; font-size: 13px; text-overflow: ellipsis; white-space: nowrap; }
.asset-list small { margin-top: 3px; color: var(--ink-muted); }
.asset-list button { display: grid; width: 32px; height: 32px; place-items: center; color: var(--ink-muted); border: 0; border-radius: 50%; background: transparent; cursor: pointer; }
.asset-list button:hover { color: var(--cinnabar); background: rgba(185,88,63,.09); }
.select-button { display: inline-flex; gap: 8px; align-items: center; justify-content: center; min-height: 43px; margin-top: auto; color: var(--green-deep); border: 1px solid rgba(33,51,45,.2); border-radius: 999px; background: transparent; font-weight: 720; cursor: pointer; transition: transform var(--motion-feedback) var(--ease-standard), background var(--motion-standard) var(--ease-standard); }
.select-button:hover { background: var(--green-soft); }
.select-button:active { transform: scale(.985); }
.details-card { margin-top: 18px; padding: 27px; border-radius: 5px 20px 20px; }
.form-grid { display: grid; grid-template-columns: minmax(180px,.6fr) minmax(300px,1.4fr); gap: 18px; margin-top: 23px; }
label { display: grid; gap: 8px; color: var(--ink-muted); font-size: 12px; font-weight: 700; }
input, textarea { width: 100%; box-sizing: border-box; padding: 12px 14px; color: var(--ink); border: 1px solid var(--line); border-radius: 11px; outline: none; background: rgba(246,241,231,.62); font: inherit; font-size: 14px; resize: vertical; transition: border-color var(--motion-feedback), box-shadow var(--motion-feedback); }
input:focus, textarea:focus { border-color: rgba(33,51,45,.55); box-shadow: 0 0 0 3px rgba(33,51,45,.08); }
.capture-footer { display: flex; justify-content: flex-end; gap: 22px; align-items: center; margin-top: 24px; }
.capture-footer p { margin: 0; color: var(--cinnabar); font-size: 12px; }
.capture-footer .ready-copy { display: inline-flex; gap: 6px; align-items: center; color: #506e61; }
.save-button { min-width: 166px; min-height: 46px; padding: 0 20px; color: var(--paper); border: 0; border-radius: 999px; background: var(--green-deep); font-weight: 760; cursor: pointer; transition: transform var(--motion-feedback) var(--ease-standard), opacity var(--motion-standard); }
.save-button:active:not(:disabled) { transform: scale(.985); }
.save-button:disabled { cursor: not-allowed; opacity: .38; }
@media (max-width: 820px) { .capture-workspace { padding: 34px 22px 96px; } .capture-header { align-items: flex-start; flex-direction: column; } .asset-columns, .form-grid { grid-template-columns: 1fr; } .capture-footer { align-items: stretch; flex-direction: column; } }
@media (prefers-reduced-motion: reduce) { .select-button, .save-button { transition: none; } }
</style>
