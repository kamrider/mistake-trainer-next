import { describe, expect, it } from 'vitest'
import { formatSettingsAuthStatus, formatSettingsBytes, formatSettingsTime } from './settings-formatters'

describe('settings formatters', () => {
  it('formats bounded byte counts without inventing invalid sizes', () => {
    expect(formatSettingsBytes(0)).toBe('0 B')
    expect(formatSettingsBytes(1023)).toBe('1023 B')
    expect(formatSettingsBytes(1024)).toBe('1.0 KB')
    expect(formatSettingsBytes(2_097_152)).toBe('2.0 MB')
    expect(formatSettingsBytes(2_147_483_648)).toBe('2.00 GB')
    expect(formatSettingsBytes(null)).toBe('未知大小')
    expect(formatSettingsBytes(-1)).toBe('未知大小')
    expect(formatSettingsBytes(Number.POSITIVE_INFINITY)).toBe('未知大小')
  })

  it('formats valid timestamps and rejects unsafe date ranges', () => {
    const formatted = formatSettingsTime(Date.UTC(2026, 6, 29, 12))

    expect(formatted).toContain('2026')
    expect(formatted).toContain('7')
    expect(formatted).toContain('29')
    expect(formatSettingsTime(null)).toBe('时间未知')
    expect(formatSettingsTime(-1)).toBe('时间未知')
    expect(formatSettingsTime(Number.POSITIVE_INFINITY)).toBe('时间未知')
    expect(formatSettingsTime(8_640_000_000_000_001)).toBe('时间未知')
  })

  it('maps every generated cloud authentication status to one stable label', () => {
    expect(formatSettingsAuthStatus('unconfigured')).toBe('未配置云端')
    expect(formatSettingsAuthStatus('signed_out')).toBe('未登录')
    expect(formatSettingsAuthStatus('verification_required')).toBe('等待邮箱验证')
    expect(formatSettingsAuthStatus('connected')).toBe('已连接')
    expect(formatSettingsAuthStatus('offline')).toBe('离线模式')
  })
})
