use serde::{Deserialize, Serialize};
use specta::Type;
use thiserror::Error;

#[cfg(windows)]
#[path = "capture_firewall_windows.rs"]
mod windows_impl;

pub const CAPTURE_FIREWALL_RULE_NAME: &str = "Mistake Trainer Next - Mobile Capture";
pub const LEGACY_CAPTURE_FIREWALL_RULE_NAME: &str =
    "Mistake Trainer Next - Mobile Capture (Private)";
pub const CAPTURE_FIREWALL_HELPER_ARGUMENT: &str = "--configure-capture-firewall";

fn remote_scope_is_exact_local_subnet(value: &str) -> bool {
    value.trim().eq_ignore_ascii_case("LocalSubnet")
}

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
    let needs_firewall_repair =
        supported && !matches!(firewall_rule, CaptureLanFirewallRuleState::Ready);

    CaptureLanPreflight {
        supported,
        active_profiles: active_profiles.to_vec(),
        firewall_rule,
        can_start: supported && !needs_firewall_repair,
        needs_network_change: false,
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

pub fn firewall_helper_requested<I, S>(arguments: I) -> bool
where
    I: IntoIterator<Item = S>,
    S: AsRef<std::ffi::OsStr>,
{
    let mut arguments = arguments.into_iter();
    let _program = arguments.next();
    matches!(arguments.next(), Some(argument) if argument.as_ref() == CAPTURE_FIREWALL_HELPER_ARGUMENT)
        && arguments.next().is_none()
}

pub fn run_capture_firewall_helper_if_requested() -> Option<i32> {
    if !firewall_helper_requested(std::env::args_os()) {
        return None;
    }
    Some(match install_capture_firewall_rule() {
        Ok(()) => 0,
        Err(error) => {
            eprintln!("capture firewall helper failed: {error}");
            1
        }
    })
}

#[cfg(windows)]
pub fn repair_capture_firewall() -> Result<CaptureLanPreflight, CaptureFirewallError> {
    windows_impl::launch_elevated_repair()?;
    capture_lan_preflight()
}

#[cfg(not(windows))]
pub fn repair_capture_firewall() -> Result<CaptureLanPreflight, CaptureFirewallError> {
    Err(CaptureFirewallError::Unsupported)
}

#[cfg(windows)]
fn install_capture_firewall_rule() -> Result<(), CaptureFirewallError> {
    windows_impl::install_rule()
}

#[cfg(not(windows))]
fn install_capture_firewall_rule() -> Result<(), CaptureFirewallError> {
    Err(CaptureFirewallError::Unsupported)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn public_profile_allows_qr_after_the_persistent_rule_is_ready() {
        let value = evaluate_preflight(
            true,
            &[CaptureLanProfile::Public],
            CaptureLanFirewallRuleState::Ready,
        );
        assert!(value.can_start);
        assert!(!value.needs_network_change);
        assert!(!value.needs_firewall_repair);
    }

    #[test]
    fn domain_profile_allows_qr_after_the_persistent_rule_is_ready() {
        let value = evaluate_preflight(
            true,
            &[CaptureLanProfile::Domain],
            CaptureLanFirewallRuleState::Ready,
        );
        assert!(value.can_start);
        assert!(!value.needs_network_change);
    }

    #[test]
    fn mixed_profiles_allow_qr_after_the_persistent_rule_is_ready() {
        let value = evaluate_preflight(
            true,
            &[CaptureLanProfile::Private, CaptureLanProfile::Public],
            CaptureLanFirewallRuleState::Ready,
        );
        assert!(value.can_start);
        assert!(!value.needs_network_change);
    }

    #[test]
    fn remote_scope_rejects_any_broader_address_list() {
        assert!(remote_scope_is_exact_local_subnet(" LocalSubnet "));
        assert!(!remote_scope_is_exact_local_subnet(
            "LocalSubnet,203.0.113.7"
        ));
        assert!(!remote_scope_is_exact_local_subnet("*"));
        assert!(!remote_scope_is_exact_local_subnet("LocalSubnet,"));
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
        let value = evaluate_preflight(false, &[], CaptureLanFirewallRuleState::Unavailable);
        assert!(!value.supported);
        assert!(!value.can_start);
    }

    #[test]
    fn elevated_helper_requires_the_exact_single_argument() {
        assert!(firewall_helper_requested([
            "mistake-trainer-next.exe",
            CAPTURE_FIREWALL_HELPER_ARGUMENT,
        ]));
        assert!(!firewall_helper_requested(["mistake-trainer-next.exe"]));
        assert!(!firewall_helper_requested([
            "mistake-trainer-next.exe",
            CAPTURE_FIREWALL_HELPER_ARGUMENT,
            "unexpected",
        ]));
        assert!(!firewall_helper_requested([
            "mistake-trainer-next.exe",
            "--configure-anything-else",
        ]));
    }
}
