import type { AuthStatusKind } from '../shared/api/bindings'

export function formatSettingsAuthStatus(kind: AuthStatusKind): string {
  return {
    unconfigured: '未配置云端',
    signed_out: '未登录',
    verification_required: '等待邮箱验证',
    connected: '已连接',
    offline: '离线模式',
  }[kind]
}

export function formatSettingsBytes(bytes: number | null): string {
  if (bytes === null || !Number.isFinite(bytes) || bytes < 0) return '未知大小'
  if (bytes < 1024) return `${bytes} B`
  if (bytes < 1024 * 1024) return `${(bytes / 1024).toFixed(1)} KB`
  if (bytes < 1024 * 1024 * 1024) return `${(bytes / 1024 / 1024).toFixed(1)} MB`
  return `${(bytes / 1024 / 1024 / 1024).toFixed(2)} GB`
}

export function formatSettingsTime(timestamp: number | null): string {
  if (
    timestamp === null
    || !Number.isFinite(timestamp)
    || timestamp < 0
    || timestamp > 8_640_000_000_000_000
  ) return '时间未知'
  return new Intl.DateTimeFormat('zh-CN', {
    dateStyle: 'medium',
    timeStyle: 'short',
  }).format(new Date(timestamp))
}
