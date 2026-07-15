use serde::{Deserialize, Serialize};
use specta::Type;
use thiserror::Error;

pub const CAPTURE_FIREWALL_RULE_NAME: &str = "Mistake Trainer Next - Mobile Capture (Private)";

#[derive(Clone, Copy, Debug, Deserialize, Serialize, Type, PartialEq, Eq)]
#[serde(rename_all = "snake_case")]
pub enum CaptureLanProfile {
    Domain,
    Private,
    Public,
}

#[derive(Clone, Copy, Debug, Deserialize, Serialize, Type, PartialEq, Eq)]
#[serde(rename_all = "snake_case")]
pub enum CaptureLanFirewallRuleState {
    Ready,
    Missing,
    Invalid,
    Unavailable,
}

#[derive(Clone, Debug, Deserialize, Serialize, Type, PartialEq, Eq)]
#[serde(rename_all = "camelCase")]
pub struct CaptureLanPreflight {
    pub supported: bool,
    pub active_profiles: Vec<CaptureLanProfile>,
    pub firewall_rule: CaptureLanFirewallRuleState,
    pub can_start: bool,
    pub needs_network_change: bool,
    pub needs_firewall_repair: bool,
}

#[derive(Debug, Error)]
pub enum CaptureFirewallError {
    #[error("Windows firewall inspection failed: {0}")]
    Inspection(String),
    #[error("Windows firewall repair is unavailable on this platform")]
    Unsupported,
    #[error("Windows firewall repair was cancelled")]
    Cancelled,
    #[error("Windows firewall repair failed: {0}")]
    Repair(String),
}

pub fn evaluate_preflight(
    supported: bool,
    active_profiles: &[CaptureLanProfile],
    firewall_rule: CaptureLanFirewallRuleState,
) -> CaptureLanPreflight {
    let has_trusted_profile = active_profiles
        .iter()
        .any(|profile| matches!(profile, CaptureLanProfile::Domain | CaptureLanProfile::Private));
    let needs_network_change = supported && !has_trusted_profile;
    let needs_firewall_repair = supported
        && !matches!(firewall_rule, CaptureLanFirewallRuleState::Ready);

    CaptureLanPreflight {
        supported,
        active_profiles: active_profiles.to_vec(),
        firewall_rule,
        can_start: supported && has_trusted_profile && !needs_firewall_repair,
        needs_network_change,
        needs_firewall_repair,
    }
}

#[cfg(windows)]
pub fn capture_lan_preflight() -> Result<CaptureLanPreflight, CaptureFirewallError> {
    windows_impl::inspect()
}

#[cfg(not(windows))]
pub fn capture_lan_preflight() -> Result<CaptureLanPreflight, CaptureFirewallError> {
    Ok(evaluate_preflight(
        false,
        &[],
        CaptureLanFirewallRuleState::Unavailable,
    ))
}

#[cfg(windows)]
mod windows_impl {
    use std::path::Path;

    use windows::{
        Win32::{
            Foundation::{RPC_E_CHANGED_MODE, VARIANT_TRUE},
            NetworkManagement::WindowsFirewall::{
                INetFwPolicy2, NET_FW_ACTION_ALLOW, NET_FW_IP_PROTOCOL_TCP,
                NET_FW_PROFILE2_DOMAIN, NET_FW_PROFILE2_PRIVATE, NET_FW_PROFILE2_PUBLIC,
                NET_FW_RULE_DIR_IN, NetFwPolicy2,
            },
            System::Com::{
                CLSCTX_INPROC_SERVER, COINIT_APARTMENTTHREADED, CoCreateInstance, CoInitializeEx,
                CoUninitialize,
            },
        },
        core::BSTR,
    };

    use super::{
        CAPTURE_FIREWALL_RULE_NAME, CaptureFirewallError, CaptureLanFirewallRuleState,
        CaptureLanPreflight, CaptureLanProfile, evaluate_preflight,
    };

    struct ComGuard(bool);

    impl ComGuard {
        fn initialize() -> Result<Self, CaptureFirewallError> {
            let result = unsafe { CoInitializeEx(None, COINIT_APARTMENTTHREADED) };
            if result.is_ok() {
                Ok(Self(true))
            } else if result == RPC_E_CHANGED_MODE {
                Ok(Self(false))
            } else {
                Err(CaptureFirewallError::Inspection(
                    windows::core::Error::from_hresult(result).to_string(),
                ))
            }
        }
    }

    impl Drop for ComGuard {
        fn drop(&mut self) {
            if self.0 {
                unsafe { CoUninitialize() };
            }
        }
    }

    pub(super) fn inspect() -> Result<CaptureLanPreflight, CaptureFirewallError> {
        let _com = ComGuard::initialize()?;
        let policy: INetFwPolicy2 = unsafe {
            CoCreateInstance(&NetFwPolicy2, None, CLSCTX_INPROC_SERVER)
                .map_err(|error| CaptureFirewallError::Inspection(error.to_string()))?
        };
        let profile_bits = unsafe {
            policy
                .CurrentProfileTypes()
                .map_err(|error| CaptureFirewallError::Inspection(error.to_string()))?
        };
        let mut profiles = Vec::new();
        if profile_bits & NET_FW_PROFILE2_DOMAIN.0 != 0 {
            profiles.push(CaptureLanProfile::Domain);
        }
        if profile_bits & NET_FW_PROFILE2_PRIVATE.0 != 0 {
            profiles.push(CaptureLanProfile::Private);
        }
        if profile_bits & NET_FW_PROFILE2_PUBLIC.0 != 0 {
            profiles.push(CaptureLanProfile::Public);
        }

        let current_exe = std::env::current_exe()
            .map_err(|error| CaptureFirewallError::Inspection(error.to_string()))?;
        let firewall_rule = inspect_named_rule(&policy, &current_exe)?;
        Ok(evaluate_preflight(true, &profiles, firewall_rule))
    }

    fn inspect_named_rule(
        policy: &INetFwPolicy2,
        current_exe: &Path,
    ) -> Result<CaptureLanFirewallRuleState, CaptureFirewallError> {
        let rules = unsafe {
            policy
                .Rules()
                .map_err(|error| CaptureFirewallError::Inspection(error.to_string()))?
        };
        let rule = match unsafe { rules.Item(&BSTR::from(CAPTURE_FIREWALL_RULE_NAME)) } {
            Ok(rule) => rule,
            Err(_) => return Ok(CaptureLanFirewallRuleState::Missing),
        };
        let expected_path = normalize_path(current_exe);
        let application = unsafe { rule.ApplicationName() }
            .map(|value| normalize_text_path(&value.to_string()))
            .unwrap_or_default();
        let remote_addresses = unsafe { rule.RemoteAddresses() }
            .map(|value| value.to_string())
            .unwrap_or_default();
        let is_ready = unsafe {
            rule.Enabled().map(|value| value == VARIANT_TRUE).unwrap_or(false)
                && rule.Action().map(|value| value == NET_FW_ACTION_ALLOW).unwrap_or(false)
                && rule.Direction().map(|value| value == NET_FW_RULE_DIR_IN).unwrap_or(false)
                && rule.Protocol().map(|value| value == NET_FW_IP_PROTOCOL_TCP.0).unwrap_or(false)
                && rule.Profiles().map(|value| value == NET_FW_PROFILE2_PRIVATE.0).unwrap_or(false)
        } && application == expected_path
            && remote_addresses
                .split(',')
                .any(|address| address.trim().eq_ignore_ascii_case("LocalSubnet"));

        Ok(if is_ready {
            CaptureLanFirewallRuleState::Ready
        } else {
            CaptureLanFirewallRuleState::Invalid
        })
    }

    fn normalize_path(path: &Path) -> String {
        normalize_text_path(&path.to_string_lossy())
    }

    fn normalize_text_path(path: &str) -> String {
        path.trim_start_matches(r"\\?\")
            .replace('/', r"\")
            .to_lowercase()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn public_only_profile_blocks_qr_even_with_a_rule() {
        let value = evaluate_preflight(
            true,
            &[CaptureLanProfile::Public],
            CaptureLanFirewallRuleState::Ready,
        );
        assert!(!value.can_start);
        assert!(value.needs_network_change);
        assert!(!value.needs_firewall_repair);
    }

    #[test]
    fn private_profile_and_exact_rule_allow_qr() {
        let value = evaluate_preflight(
            true,
            &[CaptureLanProfile::Private],
            CaptureLanFirewallRuleState::Ready,
        );
        assert!(value.can_start);
        assert!(!value.needs_network_change);
        assert!(!value.needs_firewall_repair);
    }

    #[test]
    fn missing_rule_requires_repair_without_enabling_public_access() {
        let value = evaluate_preflight(
            true,
            &[CaptureLanProfile::Private],
            CaptureLanFirewallRuleState::Missing,
        );
        assert!(!value.can_start);
        assert!(value.needs_firewall_repair);
    }

    #[test]
    fn non_windows_status_never_allows_qr() {
        let value = evaluate_preflight(
            false,
            &[],
            CaptureLanFirewallRuleState::Unavailable,
        );
        assert!(!value.supported);
        assert!(!value.can_start);
    }
}
