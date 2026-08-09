import { existsSync, readFileSync } from 'node:fs'
import { resolve } from 'node:path'
import { describe, expect, it } from 'vitest'

const facadePath = resolve('src-tauri/src/modules/capture_lan.rs')
const registryPath = resolve(
  'src-tauri/src/modules/capture_lan_session_registry.rs',
)
const apiTestsPath = resolve(
  'src-tauri/src/modules/capture_lan_api_tests.rs',
)
const readSource = (path: string) => readFileSync(path, 'utf8')

describe('capture LAN session registry boundary', () => {
  it('moves active-session ownership behind one private registry', () => {
    const facade = readSource(facadePath)

    expect(facade).toMatch(
      /#\[path = "capture_lan_session_registry\.rs"\]\r?\nmod session_registry;/,
    )
    expect(facade).toContain(
      'use session_registry::{CaptureLanSessionRegistry, WeakCaptureLanSessionRegistry};',
    )
    expect(facade).toContain('sessions: CaptureLanSessionRegistry,')
    expect(facade).not.toContain('struct ActiveSession')
    expect(facade).not.toContain('Arc<Mutex<Option<ActiveSession>>>')
    expect(facade).not.toContain('Weak<Mutex<Option<ActiveSession>>>')
    expect(facade).not.toContain('fn remove_active_session(')

    expect(existsSync(registryPath)).toBe(true)
    if (!existsSync(registryPath)) return

    const registry = readSource(registryPath)
    const production = registry.split('#[cfg(test)]')[0]
    expect(production).toContain('struct ActiveSession')
    expect(production).toContain('pub(super) struct CaptureLanSessionRegistry')
    expect(production).toContain('active: Arc<Mutex<Option<ActiveSession>>>')
    expect(production).toContain(
      'pub(super) struct WeakCaptureLanSessionRegistry',
    )
  })

  it('centralizes atomic expiration, projection, and matching-ID cleanup', () => {
    const facade = readSource(facadePath)
    if (!existsSync(registryPath)) return
    const registry = readSource(registryPath)

    for (const token of [
      'pub(super) fn ensure_startable(',
      'pub(super) fn install(',
      'pub(super) fn status(',
      'pub(super) fn stop(',
      'pub(super) fn downgrade(',
      'pub(super) fn remove_if_session(',
      'pub(super) fn shutdown_if_last_owner(',
      'active.session_id == session_id',
      'active.state.is_expired(now_utc_ms)',
      'CaptureLanError::AlreadyActive',
      'CaptureLanError::Unavailable',
      'stale_server_cleanup_does_not_remove_the_replacement_session',
      'explicit_stop_signals_the_active_session',
    ]) {
      expect(registry).toContain(token)
    }

    for (const token of [
      'self.sessions.ensure_startable(now_utc_ms)?',
      'self.sessions.install(',
      'self.sessions.status(now_utc_ms)',
      'self.sessions.stop()',
      'self.sessions.downgrade()',
      'self.sessions.remove_if_session(&session_id)',
      'self.sessions.shutdown_if_last_owner()',
      'weak_sessions.remove_if_session(&session_id)',
    ]) {
      expect(facade).toContain(token)
    }
    expect(facade).not.toContain('let _ = self.stop();')
  })

  it('keeps network, security, database, and async server work in the facade', () => {
    const facade = readSource(facadePath)
    if (!existsSync(registryPath)) return
    const registry = readSource(registryPath)
    const production = registry.split('#[cfg(test)]')[0]

    for (const forbidden of [
      'async fn',
      '.await',
      'TcpListener',
      'QrCode',
      'getrandom',
      'Sha256',
      'run_server',
      'get_capture_batch_detail',
      'query_row',
      'remove_dir_all',
    ]) {
      expect(production).not.toContain(forbidden)
    }
    for (const token of [
      'TcpListener::bind(',
      'tokio::runtime::Builder::new_multi_thread()',
      'getrandom::fill(&mut raw_token)',
      'Sha256::digest(token.as_bytes())',
      'QrCode::new(mobile_url.as_bytes())',
      'async fn run_server(',
      'tokio::fs::remove_dir_all(session_temp_root(&state)).await',
      'fn collecting_batch_next_sequence(',
    ]) {
      expect(facade).toContain(token)
    }

    const apiTests = readSource(apiTestsPath)
    expect(apiTests).toContain('WeakCaptureLanSessionRegistry::default()')
    expect(apiTests).not.toContain('Weak::<Mutex<Option<ActiveSession>>>::new()')
  })
})
