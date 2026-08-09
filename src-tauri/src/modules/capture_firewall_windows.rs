use std::{ffi::OsStr, os::windows::ffi::OsStrExt, path::Path, time::Duration};

use windows::{
    Win32::{
        Foundation::{
            CloseHandle, ERROR_CANCELLED, ERROR_FILE_NOT_FOUND, RPC_E_CHANGED_MODE, VARIANT_FALSE,
            VARIANT_TRUE, WAIT_OBJECT_0,
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
    CaptureLanPreflight, CaptureLanProfile, LEGACY_CAPTURE_FIREWALL_RULE_NAME, evaluate_preflight,
    remote_scope_is_exact_local_subnet,
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
    let _com =
        ComGuard::initialize().map_err(|error| CaptureFirewallError::Repair(error.to_string()))?;
    let policy: INetFwPolicy2 = unsafe {
        CoCreateInstance(&NetFwPolicy2, None, CLSCTX_INPROC_SERVER)
            .map_err(|error| CaptureFirewallError::Repair(error.to_string()))?
    };
    let rules = unsafe {
        policy
            .Rules()
            .map_err(|error| CaptureFirewallError::Repair(error.to_string()))?
    };
    let legacy_name = BSTR::from(LEGACY_CAPTURE_FIREWALL_RULE_NAME);
    match unsafe { rules.Remove(&legacy_name) } {
        Ok(()) => {}
        Err(error) if error.code() == HRESULT::from_win32(ERROR_FILE_NOT_FOUND.0) => {}
        Err(error) => return Err(CaptureFirewallError::Repair(error.to_string())),
    }
    let name = BSTR::from(CAPTURE_FIREWALL_RULE_NAME);
    let rule: INetFwRule = unsafe {
        CoCreateInstance(&NetFwRule, None, CLSCTX_INPROC_SERVER)
            .map_err(|error| CaptureFirewallError::Repair(error.to_string()))?
    };
    let current_exe =
        std::env::current_exe().map_err(|error| CaptureFirewallError::Repair(error.to_string()))?;
    let application = BSTR::from(current_exe.to_string_lossy().as_ref());
    let description =
        BSTR::from("Allow Mistake Trainer phone capture from this computer's local subnet only.");
    let local_subnet = BSTR::from("LocalSubnet");
    let all_profiles =
        NET_FW_PROFILE2_DOMAIN.0 | NET_FW_PROFILE2_PRIVATE.0 | NET_FW_PROFILE2_PUBLIC.0;
    unsafe {
        rule.SetName(&name)
            .and_then(|_| rule.SetDescription(&description))
            .and_then(|_| rule.SetApplicationName(&application))
            .and_then(|_| rule.SetProtocol(NET_FW_IP_PROTOCOL_TCP.0))
            .and_then(|_| rule.SetDirection(NET_FW_RULE_DIR_IN))
            .and_then(|_| rule.SetAction(NET_FW_ACTION_ALLOW))
            .and_then(|_| rule.SetProfiles(all_profiles))
            .and_then(|_| rule.SetRemoteAddresses(&local_subnet))
            .and_then(|_| rule.SetEdgeTraversal(VARIANT_FALSE))
            .and_then(|_| rule.SetEnabled(VARIANT_TRUE))
            .and_then(|_| rules.Add(&rule))
            .map_err(|error| CaptureFirewallError::Repair(error.to_string()))?;
    }
    Ok(())
}

pub(super) fn launch_elevated_repair() -> Result<(), CaptureFirewallError> {
    let current_exe =
        std::env::current_exe().map_err(|error| CaptureFirewallError::Repair(error.to_string()))?;
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
        Err(error) if error.code() == HRESULT::from_win32(ERROR_FILE_NOT_FOUND.0) => {
            return Ok(CaptureLanFirewallRuleState::Missing);
        }
        Err(error) => return Err(CaptureFirewallError::Inspection(error.to_string())),
    };
    let expected_path = normalize_path(current_exe);
    let application = unsafe { rule.ApplicationName() }
        .map(|value| normalize_text_path(&value.to_string()))
        .map_err(|error| CaptureFirewallError::Inspection(error.to_string()))?;
    let remote_addresses = unsafe { rule.RemoteAddresses() }
        .map(|value| value.to_string())
        .map_err(|error| CaptureFirewallError::Inspection(error.to_string()))?;
    let enabled = unsafe { rule.Enabled() }
        .map_err(|error| CaptureFirewallError::Inspection(error.to_string()))?;
    let action = unsafe { rule.Action() }
        .map_err(|error| CaptureFirewallError::Inspection(error.to_string()))?;
    let direction = unsafe { rule.Direction() }
        .map_err(|error| CaptureFirewallError::Inspection(error.to_string()))?;
    let protocol = unsafe { rule.Protocol() }
        .map_err(|error| CaptureFirewallError::Inspection(error.to_string()))?;
    let profiles = unsafe { rule.Profiles() }
        .map_err(|error| CaptureFirewallError::Inspection(error.to_string()))?;
    let edge_traversal = unsafe { rule.EdgeTraversal() }
        .map_err(|error| CaptureFirewallError::Inspection(error.to_string()))?;
    let all_profiles =
        NET_FW_PROFILE2_DOMAIN.0 | NET_FW_PROFILE2_PRIVATE.0 | NET_FW_PROFILE2_PUBLIC.0;
    let is_ready = enabled == VARIANT_TRUE
        && action == NET_FW_ACTION_ALLOW
        && direction == NET_FW_RULE_DIR_IN
        && protocol == NET_FW_IP_PROTOCOL_TCP.0
        && profiles == all_profiles
        && edge_traversal == VARIANT_FALSE
        && application == expected_path
        && remote_scope_is_exact_local_subnet(&remote_addresses);

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
