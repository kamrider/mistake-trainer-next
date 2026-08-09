import { existsSync, readFileSync } from 'node:fs'
import { resolve } from 'node:path'
import { describe, expect, it } from 'vitest'

const facadePath = resolve('src-tauri/src/modules/auth_sync.rs')
const statePath = resolve('src-tauri/src/modules/auth_session_state.rs')
const readSource = (path: string) => readFileSync(path, 'utf8')

describe('auth session state boundary', () => {
  it('delegates volatile session state to one private atomic snapshot', () => {
    const facade = readSource(facadePath)

    expect(facade).toMatch(
      /#\[path = "auth_session_state\.rs"\]\r?\nmod session_state;/,
    )
    expect(facade).toContain('use session_state::CloudSessionState;')
    expect(facade).toContain('state: CloudSessionState,')
    expect(facade).not.toContain('RwLock<Option<ActiveCloudSession>>')
    expect(facade).not.toContain('RwLock<Option<String>>')
    expect(facade).not.toContain('RwLock<bool>')

    expect(existsSync(statePath)).toBe(true)
    if (!existsSync(statePath)) return

    const state = readSource(statePath)
    expect(state).toContain('struct CloudSessionStateSnapshot')
    expect(state).toContain('inner: RwLock<CloudSessionStateSnapshot>')
    expect(state.match(/RwLock</g)).toHaveLength(1)
    expect(state).toContain('pub(super) struct CloudSessionState')
  })

  it('owns redaction, status priority, and every volatile transition', () => {
    const facade = readSource(facadePath)
    if (!existsSync(statePath)) return
    const state = readSource(statePath)

    for (const token of [
      'struct ActiveCloudSession',
      'impl fmt::Debug for ActiveCloudSession',
      'pub(super) fn fmt_manager(',
      'pub(super) fn status(',
      'pub(super) fn connect(',
      'pub(super) fn mark_verification_required(',
      'pub(super) fn mark_offline(',
      'pub(super) fn reject_authentication(',
      'pub(super) fn access_token(',
      'pub(super) fn disconnect(',
      'pub(super) fn session_snapshot(',
      'AuthStatusKind::Connected',
      'AuthStatusKind::Offline',
      'AuthStatusKind::VerificationRequired',
      'AuthStatusKind::SignedOut',
      'redact_email',
      '"<redacted>"',
    ]) {
      expect(state).toContain(token)
    }

    expect(facade).toMatch(/self\s*\.\s*state\s*\.\s*connect\(/)
    for (const token of [
      'self.state.fmt_manager(formatter)',
      'self.state.status()',
      'self.state.mark_verification_required(email)',
      'self.state.mark_offline()',
      'self.state.reject_authentication()',
      'self.state.access_token()',
      'self.state.disconnect()',
      'self.state.session_snapshot()',
    ]) {
      expect(facade).toContain(token)
    }
  })

  it('keeps awaits and durable credential transactions in the facade', () => {
    const facade = readSource(facadePath)
    if (!existsSync(statePath)) return
    const state = readSource(statePath)

    for (const forbidden of [
      'async fn',
      '.await',
      'AuthTransport',
      'SecretStore',
      'CLOUD_REFRESH_TOKEN',
      'CLOUD_USER_ID',
      'tokio::',
    ]) {
      expect(state).not.toContain(forbidden)
    }
    for (const token of [
      'pub async fn sign_up<T: AuthTransport>',
      'pub async fn sign_in<T: AuthTransport>',
      'pub async fn restore<T: AuthTransport>',
      'pub async fn disconnect<T: AuthTransport>',
      'const CLOUD_REFRESH_TOKEN',
      'const CLOUD_USER_ID',
      'let previous_refresh = nonempty_secret(',
      'let restored = previous_refresh.as_deref().unwrap_or("")',
      'tokio::time::timeout(Duration::from_secs(2)',
      'fn nonempty_secret(',
    ]) {
      expect(facade).toContain(token)
    }
    expect(facade).not.toMatch(/\.read\(\)|\.write\(\)/)
  })
})
