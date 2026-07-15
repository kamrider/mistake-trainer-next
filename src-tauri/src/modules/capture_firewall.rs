use serde::{Deserialize, Serialize};
use specta::Type;
use thiserror::Error;

pub const CAPTURE_FIREWALL_RULE_NAME: &str = "Mistake Trainer Next - Mobile Capture (Private)";
pub const CAPTURE_FIREWALL_HELPER_ARGUMENT: &str = "--configure-capture-firewall";

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
pub fn open_network_settings() -> Result<bool, CaptureFirewallError> {
    windows_impl::open_network_settings()
}

#[cfg(not(windows))]
pub fn open_network_settings() -> Result<bool, CaptureFirewallError> {
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

#[cfg(windows)]
mod windows_impl {
    use std::{
        ffi::OsStr,
        os::windows::ffi::OsStrExt,
        path::Path,
        time::Duration,
    };

    use windows::{
        Win32::{
            Foundation::{
                CloseHandle, ERROR_CANCELLED, RPC_E_CHANGED_MODE, VARIANT_FALSE, VARIANT_TRUE,
                WAIT_OBJECT_0,
            },
            NetworkManagement::WindowsFirewall::{
                INetFwPolicy2, INetFwRule, NET_FW_ACTION_ALLOW, NET_FW_IP_PROTOCOL_TCP,
                NET_FW_PROFILE2_DOMAIN, NET_FW_PROFILE2_PRIVATE, NET_FW_PROFILE2_PUBLIC,
                NET_FW_RULE_DIR_IN, NetFwPolicy2, NetFwRule,
            },
            System::Com::{
                CLSCTX_INPROC_SERVER, COINIT_APARTMENTTHREADED, CoCreateInstance, CoInitializeEx,
                CoUninitialize,
            },
            System::Threading::{GetExitCodeProcess, WaitForSingleObject},
            UI::Shell::{SEE_MASK_NOCLOSEPROCESS, SHELLEXECUTEINFOW, ShellExecuteExW},
        },
        core::{BSTR, HRESULT, PCWSTR, w},
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

    pub(super) fn install_rule() -> Result<(), CaptureFirewallError> {
        let _com = ComGuard::initialize()
            .map_err(|error| CaptureFirewallError::Repair(error.to_string()))?;
        let policy: INetFwPolicy2 = unsafe {
            CoCreateInstance(&NetFwPolicy2, None, CLSCTX_INPROC_SERVER)
                .map_err(|error| CaptureFirewallError::Repair(error.to_string()))?
        };
        let rules = unsafe {
            policy
                .Rules()
                .map_err(|error| CaptureFirewallError::Repair(error.to_string()))?
        };
        let name = BSTR::from(CAPTURE_FIREWALL_RULE_NAME);
        let _ = unsafe { rules.Remove(&name) };
        let rule: INetFwRule = unsafe {
            CoCreateInstance(&NetFwRule, None, CLSCTX_INPROC_SERVER)
                .map_err(|error| CaptureFirewallError::Repair(error.to_string()))?
        };
        let current_exe = std::env::current_exe()
            .map_err(|error| CaptureFirewallError::Repair(error.to_string()))?;
        let application = BSTR::from(current_exe.to_string_lossy().as_ref());
        let description = BSTR::from(
            "Allow Mistake Trainer phone capture only from this computer's private local network.",
        );
        let local_subnet = BSTR::from("LocalSubnet");
        unsafe {
            rule.SetName(&name)
                .and_then(|_| rule.SetDescription(&description))
                .and_then(|_| rule.SetApplicationName(&application))
                .and_then(|_| rule.SetProtocol(NET_FW_IP_PROTOCOL_TCP.0))
                .and_then(|_| rule.SetDirection(NET_FW_RULE_DIR_IN))
                .and_then(|_| rule.SetAction(NET_FW_ACTION_ALLOW))
                .and_then(|_| rule.SetProfiles(NET_FW_PROFILE2_PRIVATE.0))
                .and_then(|_| rule.SetRemoteAddresses(&local_subnet))
                .and_then(|_| rule.SetEdgeTraversal(VARIANT_FALSE))
                .and_then(|_| rule.SetEnabled(VARIANT_TRUE))
                .and_then(|_| rules.Add(&rule))
                .map_err(|error| CaptureFirewallError::Repair(error.to_string()))?;
        }
        Ok(())
    }

    pub(super) fn launch_elevated_repair() -> Result<(), CaptureFirewallError> {
        let current_exe = std::env::current_exe()
            .map_err(|error| CaptureFirewallError::Repair(error.to_string()))?;
        let executable = wide(current_exe.as_os_str());
        let mut execute_info: SHELLEXECUTEINFOW = unsafe { std::mem::zeroed() };
        execute_info.cbSize = std::mem::size_of::<SHELLEXECUTEINFOW>() as u32;
        execute_info.fMask = SEE_MASK_NOCLOSEPROCESS;
        execute_info.lpVerb = w!("runas");
        execute_info.lpFile = PCWSTR(executable.as_ptr());
        execute_info.lpParameters = w!("--configure-capture-firewall");
        execute_info.nShow = 0;
        if let Err(error) = unsafe { ShellExecuteExW(&mut execute_info) } {
            if error.code() == HRESULT::from_win32(ERROR_CANCELLED.0) {
                return Err(CaptureFirewallError::Cancelled);
            }
            return Err(CaptureFirewallError::Repair(error.to_string()));
        }
        if execute_info.hProcess.is_invalid() {
            return Err(CaptureFirewallError::Repair(
                "elevated helper returned no process handle".to_owned(),
            ));
        }
        let wait = unsafe {
            WaitForSingleObject(
                execute_info.hProcess,
                u32::try_from(Duration::from_secs(60).as_millis()).unwrap_or(u32::MAX),
            )
        };
        if wait != WAIT_OBJECT_0 {
            let _ = unsafe { CloseHandle(execute_info.hProcess) };
            return Err(CaptureFirewallError::Repair(
                "elevated helper did not finish in time".to_owned(),
            ));
        }
        let mut exit_code = 1u32;
        let exit_result = unsafe { GetExitCodeProcess(execute_info.hProcess, &mut exit_code) };
        let _ = unsafe { CloseHandle(execute_info.hProcess) };
        exit_result.map_err(|error| CaptureFirewallError::Repair(error.to_string()))?;
        if exit_code != 0 {
            return Err(CaptureFirewallError::Repair(format!(
                "elevated helper exited with code {exit_code}"
            )));
        }
        Ok(())
    }

    pub(super) fn open_network_settings() -> Result<bool, CaptureFirewallError> {
        let mut execute_info: SHELLEXECUTEINFOW = unsafe { std::mem::zeroed() };
        execute_info.cbSize = std::mem::size_of::<SHELLEXECUTEINFOW>() as u32;
        execute_info.lpVerb = w!("open");
        execute_info.lpFile = w!("ms-settings:network-status");
        execute_info.nShow = 1;
        unsafe { ShellExecuteExW(&mut execute_info) }
            .map(|_| true)
            .map_err(|error| CaptureFirewallError::Repair(error.to_string()))
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

    fn wide(value: &OsStr) -> Vec<u16> {
        value.encode_wide().chain(std::iter::once(0)).collect()
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
