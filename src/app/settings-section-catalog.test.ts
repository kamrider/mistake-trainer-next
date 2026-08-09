import { describe, expect, it } from 'vitest'
import { buildSettingsSections } from './settings-section-catalog'

describe('buildSettingsSections', () => {
  it('keeps the commercial settings groups in a stable task-oriented order', () => {
    const sections = buildSettingsSections({
      overview: true,
      subjects: true,
      review: true,
    })

    expect([...new Set(sections.map(section => section.group))]).toEqual([
      '账户与同步',
      '学习体验',
      '数据与安全',
      '应用维护',
    ])
    expect(sections.map(section => section.id)).toEqual([
      'settings-sync',
      'settings-overview',
      'settings-subjects',
      'settings-review',
      'settings-ocr',
      'settings-storage',
      'settings-backup',
      'settings-migration',
      'settings-updates',
      'settings-diagnostics',
    ])
  })

  it('only includes sections backed by loaded optional settings data', () => {
    const sections = buildSettingsSections({
      overview: false,
      subjects: false,
      review: false,
    })

    expect(sections.map(section => section.id)).not.toContain('settings-overview')
    expect(sections.map(section => section.id)).not.toContain('settings-subjects')
    expect(sections.map(section => section.id)).not.toContain('settings-review')
    expect(sections.map(section => section.id)).toContain('settings-sync')
    expect(sections.map(section => section.id)).toContain('settings-diagnostics')
  })

  it('never emits duplicate section ids', () => {
    const sections = buildSettingsSections({
      overview: true,
      subjects: true,
      review: true,
    })
    const ids = sections.map(section => section.id)

    expect(new Set(ids).size).toBe(ids.length)
  })
})
