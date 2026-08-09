<script setup lang="ts">
const steps = [
  {
    number: '01',
    title: '选择题目',
    detail: '从到期队列、最近训练或全部活动题中确认本次材料。',
  },
  {
    number: '02',
    title: '保存快照',
    detail: '保存题目顺序与排版，可同步、可恢复，也可重复使用。',
  },
  {
    number: '03',
    title: '生成文件',
    detail: '需要时再选择本机位置，生成 DOCX 或整理后的原图文件夹。',
  },
]
</script>

<template>
  <aside
    class="export-workflow-guide"
    aria-labelledby="export-workflow-title"
  >
    <div>
      <p>三步完成</p>
      <h3 id="export-workflow-title">
        先保存方案，再生成文件
      </h3>
    </div>
    <ol>
      <li
        v-for="step in steps"
        :key="step.number"
      >
        <span aria-hidden="true">{{ step.number }}</span>
        <div>
          <strong>{{ step.title }}</strong>
          <small>{{ step.detail }}</small>
        </div>
      </li>
    </ol>
  </aside>
</template>

<style scoped>
.export-workflow-guide {
  display: grid;
  grid-template-columns: minmax(180px,.55fr) minmax(0,1.45fr);
  gap: 18px;
  align-items: center;
  margin-top: 20px;
  padding: 16px 18px;
  border: 1px solid rgba(33,51,45,.14);
  border-radius: 5px 15px 15px;
  background: linear-gradient(135deg,rgba(220,228,220,.48),rgba(255,253,247,.72));
}
.export-workflow-guide p { margin: 0 0 5px; color: var(--cinnabar); font-size: 12px; font-weight: 800; letter-spacing: .12em; }
.export-workflow-guide h3 { margin: 0; color: var(--green-deep); font-family: var(--font-serif); font-size: 18px; }
.export-workflow-guide ol { display: grid; grid-template-columns: repeat(3,minmax(0,1fr)); gap: 8px; margin: 0; padding: 0; list-style: none; }
.export-workflow-guide li { display: grid; grid-template-columns: 34px minmax(0,1fr); gap: 9px; align-items: center; min-height: 62px; padding: 9px; border-radius: 10px; background: rgba(255,253,247,.65); }
.export-workflow-guide li > span { display: grid; width: 32px; height: 32px; place-items: center; color: var(--paper); border-radius: 10px 3px 10px 10px; background: var(--green-deep); font-family: var(--font-serif); font-size: 12px; }
.export-workflow-guide strong,.export-workflow-guide small { display: block; }
.export-workflow-guide strong { color: var(--green-deep); font-size: 13px; }
.export-workflow-guide small { margin-top: 3px; color: var(--ink-muted); font-size: 12px; line-height: 1.45; }
@media(max-width:900px) {
  .export-workflow-guide { grid-template-columns: 1fr; }
}
@media(max-width:700px) {
  .export-workflow-guide ol { grid-template-columns: 1fr; }
}
</style>
