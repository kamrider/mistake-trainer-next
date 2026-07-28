<script setup lang="ts">
import { CheckCircle2, Layers3, LockKeyhole, ScanText } from '@lucide/vue'
</script>

<template>
  <section
    class="intelligence-panel"
    role="region"
    aria-labelledby="intelligence-title"
  >
    <header>
      <span class="panel-icon"><Layers3 :size="21" /></span>
      <div>
        <p>LOCAL · CLEAR BOUNDARIES</p>
        <h2 id="intelligence-title">
          智能功能模式
        </h2>
        <span>当前切图与后续自动识题是两条独立能力，不再让模型档位和现有功能混在一起。</span>
      </div>
    </header>

    <div class="mode-grid">
      <article aria-label="智能切图（已开放）">
        <div class="mode-heading">
          <span class="mode-icon active"><CheckCircle2 :size="19" /></span>
          <div>
            <p>模式一</p>
            <h3>智能切图</h3>
          </div>
          <strong class="mode-badge active">已开放</strong>
        </div>
        <p class="mode-summary">
          用本地传统视觉查找分栏和留白，把一页多题拆成多张素材图片。
        </p>
        <ul>
          <li>不读取文字，不使用 OCR</li>
          <li>不下载 small / medium 模型</li>
          <li>切图确认后只进入素材牌库</li>
          <li>原图、现有题卡和加密存储保持不变</li>
        </ul>
      </article>

      <article
        class="future-mode"
        aria-label="全自动识题（未开放）"
      >
        <div class="mode-heading">
          <span class="mode-icon"><ScanText :size="19" /></span>
          <div>
            <p>模式二</p>
            <h3>全自动识题</h3>
          </div>
          <strong class="mode-badge">未开放</strong>
        </div>
        <p class="mode-summary">
          后续识别科目、题干与答案，完成题答匹配、自动组卡和一键导出。
        </p>
        <ul>
          <li>届时才会评估 OCR 与内容理解模型</li>
          <li>模型档位会结合本机性能重新设计</li>
          <li>当前版本不会运行或发起下载</li>
          <li>未经过真实试卷验收前不会自动入库</li>
        </ul>
      </article>
    </div>

    <p class="privacy-note">
      <LockKeyhole :size="16" />
      两种模式都以本机隐私和人工可撤销为前提；当前只有智能切图可以执行。
    </p>
  </section>
</template>

<style scoped>
.intelligence-panel {
  margin-bottom: 16px;
  padding: 24px 26px;
  border: 1px solid rgba(33, 51, 45, .22);
  border-radius: 17px;
  background: linear-gradient(145deg, rgba(255, 253, 247, .96), rgba(220, 228, 220, .36));
  box-shadow: 0 16px 48px rgba(34, 48, 43, .05);
}

header,
.mode-heading,
.privacy-note {
  display: flex;
  align-items: flex-start;
}

header {
  gap: 13px;
  margin-bottom: 18px;
}

.panel-icon,
.mode-icon {
  display: grid;
  flex: 0 0 auto;
  place-items: center;
}

.panel-icon {
  width: 42px;
  height: 42px;
  color: var(--green-deep);
  border-radius: 12px;
  background: var(--green-soft);
}

header p,
.mode-heading p {
  margin: 0 0 5px;
  color: var(--cinnabar);
  font-size: 10px;
  font-weight: 820;
  letter-spacing: .13em;
}

header h2 {
  margin: 0;
  color: var(--green-deep);
  font-family: var(--font-serif);
  font-size: 22px;
}

header span:last-child {
  display: block;
  margin-top: 6px;
  color: var(--ink-muted);
  font-size: 12px;
  line-height: 1.6;
}

.mode-grid {
  display: grid;
  grid-template-columns: repeat(2, minmax(0, 1fr));
  gap: 12px;
}

article {
  padding: 17px;
  border: 1px solid rgba(69, 99, 84, .32);
  border-radius: 14px;
  background: rgba(255, 253, 247, .78);
  box-shadow: inset 3px 0 0 #557263;
}

article.future-mode {
  border-style: dashed;
  border-color: rgba(33, 51, 45, .18);
  background: rgba(244, 241, 232, .58);
  box-shadow: none;
}

.mode-heading {
  gap: 10px;
  align-items: center;
}

.mode-icon {
  width: 34px;
  height: 34px;
  color: var(--ink-muted);
  border-radius: 10px;
  background: rgba(33, 51, 45, .07);
}

.mode-icon.active {
  color: var(--green-deep);
  background: var(--green-soft);
}

.mode-heading div {
  min-width: 0;
}

.mode-heading p {
  margin-bottom: 1px;
  color: var(--ink-muted);
}

.mode-heading h3 {
  margin: 0;
  color: var(--green-deep);
  font-size: 15px;
}

.mode-badge {
  margin-left: auto;
  padding: 5px 8px;
  color: var(--ink-muted);
  border: 1px solid rgba(33, 51, 45, .16);
  border-radius: 999px;
  font-size: 9px;
  white-space: nowrap;
}

.mode-badge.active {
  color: #fff;
  border-color: #557263;
  background: #557263;
}

.mode-summary {
  margin: 13px 0 9px;
  color: var(--ink);
  font-size: 12px;
  line-height: 1.6;
}

ul {
  display: grid;
  gap: 5px;
  margin: 0;
  padding-left: 18px;
  color: var(--ink-muted);
  font-size: 11px;
  line-height: 1.5;
}

.privacy-note {
  gap: 8px;
  align-items: center;
  margin: 14px 0 0;
  padding: 11px 13px;
  color: var(--ink-muted);
  border-radius: 11px;
  background: rgba(33, 51, 45, .055);
  font-size: 11px;
}

@media (max-width: 760px) {
  .intelligence-panel {
    padding: 20px 18px;
  }

  .mode-grid {
    grid-template-columns: 1fr;
  }
}
</style>
