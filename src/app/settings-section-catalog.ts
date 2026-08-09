export interface SettingsSectionLink {
  id: string
  label: string
  hint: string
  group: string
}

export interface SettingsSectionAvailability {
  overview: boolean
  subjects: boolean
  review: boolean
}

export function buildSettingsSections(
  availability: SettingsSectionAvailability,
): SettingsSectionLink[] {
  return [
    { id: 'settings-sync', label: '同步账户', hint: '本地与云端', group: '账户与同步' },
    ...(availability.overview
      ? [{ id: 'settings-overview', label: '本机概况', hint: '题库与冲突', group: '账户与同步' }]
      : []),
    ...(availability.subjects
      ? [{ id: 'settings-subjects', label: '科目配置', hint: '采集常用项', group: '学习体验' }]
      : []),
    ...(availability.review
      ? [{ id: 'settings-review', label: '训练节奏', hint: '专注插曲', group: '学习体验' }]
      : []),
    { id: 'settings-ocr', label: '智能功能', hint: '切图与识题', group: '学习体验' },
    { id: 'settings-storage', label: '存储位置', hint: '容量与迁移', group: '数据与安全' },
    { id: 'settings-backup', label: '备份恢复', hint: '完整快照', group: '数据与安全' },
    { id: 'settings-migration', label: '旧版迁移', hint: '安全导入', group: '数据与安全' },
    { id: 'settings-updates', label: '应用更新', hint: '签名版本', group: '应用维护' },
    { id: 'settings-diagnostics', label: '安全诊断', hint: '故障报告', group: '应用维护' },
  ]
}
