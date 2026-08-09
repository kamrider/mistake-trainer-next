import { existsSync, readFileSync } from 'node:fs'
import { resolve } from 'node:path'
import { describe, expect, it } from 'vitest'

const facadePath = resolve('src-tauri/src/modules/capture_firewall.rs')
const adapterPath = resolve(
  'src-tauri/src/modules/capture_firewall_windows.rs',
)

const readSource = (path: string) => readFileSync(path, 'utf8')

describe('capture firewall Windows adapter boundary', () => {
  it('routes Windows operations through one private cfg-gated adapter', () => {
    const facade = readSource(facadePath)

    expect(facade).toMatch(
      /#\[cfg\(windows\)\]\r?\n#\[path = "capture_firewall_windows\.rs"\]\r?\nmod windows_impl;/,
    )
    expect(existsSync(adapterPath)).toBe(true)
    if (!existsSync(adapterPath)) return

    const adapter = readSource(adapterPath)
    for (const operation of ['inspect', 'install_rule', 'launch_elevated_repair']) {
      expect(adapter).toMatch(new RegExp(`pub\\(super\\) fn ${operation}\\(`))
      expect(facade).toContain(`windows_impl::${operation}(`)
    }
    expect(adapter.match(/pub\(super\) fn /g)).toHaveLength(3)
  })

  it('isolates native APIs while preserving public policy ownership', () => {
    const facade = readSource(facadePath)
    if (!existsSync(adapterPath)) return
    const adapter = readSource(adapterPath)

    for (const token of [
      'pub enum CaptureLanProfile',
      'pub enum CaptureLanFirewallRuleState',
      'pub struct CaptureLanPreflight',
      'pub enum CaptureFirewallError',
      'pub fn evaluate_preflight(',
      'pub fn capture_lan_preflight(',
      'pub fn repair_capture_firewall(',
      'pub fn firewall_helper_requested',
      'pub fn run_capture_firewall_helper_if_requested(',
      'fn remote_scope_is_exact_local_subnet(',
    ]) {
      expect(facade).toContain(token)
      expect(adapter).not.toContain(token)
    }

    for (const token of [
      'use windows::',
      'struct ComGuard',
      'CoCreateInstance',
      'ShellExecuteExW',
      'WaitForSingleObject',
      'GetExitCodeProcess',
      'fn inspect_named_rule(',
      'fn normalize_text_path(',
      'fn wide(',
    ]) {
      expect(adapter).toContain(token)
      expect(facade).not.toContain(token)
    }
    expect(facade).not.toContain('unsafe {')
  })

  it('locks the installed rule to the executable and local subnet', () => {
    if (!existsSync(adapterPath)) return
    const adapter = readSource(adapterPath)

    for (const token of [
      'LEGACY_CAPTURE_FIREWALL_RULE_NAME',
      'CAPTURE_FIREWALL_RULE_NAME',
      'SetApplicationName(&application)',
      'SetProtocol(NET_FW_IP_PROTOCOL_TCP.0)',
      'SetDirection(NET_FW_RULE_DIR_IN)',
      'SetAction(NET_FW_ACTION_ALLOW)',
      'SetProfiles(all_profiles)',
      'SetRemoteAddresses(&local_subnet)',
      'SetEdgeTraversal(VARIANT_FALSE)',
      'SetEnabled(VARIANT_TRUE)',
      'remote_scope_is_exact_local_subnet(&remote_addresses)',
      'Duration::from_secs(60)',
      'ERROR_CANCELLED',
      'CloseHandle(execute_info.hProcess)',
    ]) {
      expect(adapter).toContain(token)
    }
    expect(adapter).toContain('BSTR::from("LocalSubnet")')
    expect(adapter).toContain('w!("--configure-capture-firewall")')
  })
})
