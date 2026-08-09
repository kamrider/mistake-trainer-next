<script setup lang="ts">
import { ArrowRight, Check, RotateCcw } from '@lucide/vue'
import type { LegacyImportReceipt, LegacyRollbackReceipt } from '../../../shared/api/bindings'

defineProps<{ receipt?: LegacyImportReceipt, rollback?: LegacyRollbackReceipt }>()
defineEmits<{ restart: [] }>()
</script>

<template>
  <article
    v-if="receipt"
    class="legacy-result"
    aria-live="polite"
  >
    <div class="result-seal">
      <Check :size="28" />
    </div>
    <div class="result-copy">
      <p>迁移完成</p>
      <h3>{{ receipt.problemCount }} 道旧题已经进入新题库</h3>
      <span>已创建 {{ receipt.memberCount }} 个学习档案，处理 {{ receipt.assetCount }} 张图片与 {{ receipt.reviewCount }} 条训练记录。</span>
      <small v-if="receipt.frozenProblemCount">另有 {{ receipt.frozenProblemCount }} 道旧版冻结题，已整理为可重生成的导出批次。</small>
      <small>新档案已加入左下角的学习档案菜单；切换档案后即可逐份验收题目。</small>
    </div>
    <div class="result-actions">
      <a href="#/library">前往题库验收 <ArrowRight :size="16" /></a>
      <button
        type="button"
        @click="$emit('restart')"
      >
        <RotateCcw :size="15" />迁移另一份资料
      </button>
    </div>
  </article>
  <article
    v-else-if="rollback"
    class="legacy-result rollback-result"
    aria-live="polite"
  >
    <div class="result-seal">
      <Check :size="28" />
    </div>
    <div class="result-copy">
      <p>撤销完成</p>
      <h3>已移除 {{ rollback.removedProblemCount }} 道仍归属于本次导入的题</h3>
      <span>同时移除 {{ rollback.removedProfileCount }} 个档案与 {{ rollback.removedAssetCount }} 个未被复用的图片资源。</span>
      <small v-if="rollback.preservedEntityCount">为保护后续编辑，保留了 {{ rollback.preservedEntityCount }} 项已复用或已变化的数据。</small>
    </div>
  </article>
</template>

<style scoped>
.legacy-result{display:grid;grid-template-columns:auto 1fr auto;gap:16px;align-items:center;padding:19px;border:1px solid rgba(33,51,45,.2);border-radius:8px 18px 18px;background:linear-gradient(120deg,rgba(33,51,45,.08),rgba(255,253,247,.75));animation:result-in var(--motion-page) var(--ease-standard) both}.result-seal{display:grid;width:54px;height:54px;place-items:center;color:#fffdf7;border-radius:18px 6px 18px 18px;background:var(--green-deep);box-shadow:0 12px 25px rgba(33,51,45,.18)}.result-copy{display:grid;gap:4px}.result-copy p{margin:0;color:#557263;font-size:12px;font-weight:850;letter-spacing:.14em}.result-copy h3{margin:0;color:var(--green-deep);font-family:Georgia,'Microsoft YaHei',serif;font-size:19px}.result-copy span,.result-copy small{color:var(--ink-muted);font-size:12px;line-height:1.6}.result-actions{display:grid;gap:8px}.result-actions a,.result-actions button{display:inline-flex;min-height:44px;gap:6px;align-items:center;justify-content:center;padding:0 14px;border:1px solid var(--green-deep);border-radius:10px;text-decoration:none}.result-actions a{color:#fffdf7;background:var(--green-deep)}.result-actions button{color:var(--green-deep);background:transparent;cursor:pointer}.rollback-result{grid-template-columns:auto 1fr}.rollback-result .result-seal{background:#557263}@keyframes result-in{from{opacity:0;transform:translateY(10px)}}@media(max-width:720px){.legacy-result{grid-template-columns:auto 1fr}.result-actions{grid-column:1/-1;grid-template-columns:1fr 1fr}}@media(max-width:480px){.legacy-result{grid-template-columns:1fr}.result-actions{grid-template-columns:1fr}}@media(prefers-reduced-motion:reduce){.legacy-result{animation:none}}
</style>
