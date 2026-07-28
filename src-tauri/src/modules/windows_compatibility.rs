use serde::Serialize;
use specta::Type;

pub const MINIMUM_WINDOWS_BUILD: u32 = 17_763;
const WINDOWS_11_FIRST_BUILD: u32 = 22_000;

#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Type)]
#[serde(rename_all = "snake_case")]
pub enum WindowsSupportLevel {
    Supported,
    Extended,
    Unsupported,
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct WindowsCompatibilityFacts {
    pub os_name: String,
    pub display_version: String,
    pub build_number: u32,
    pub update_build_revision: u32,
    pub process_architecture: String,
    pub native_architecture: String,
    pub webview2_version: Option<String>,
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Type)]
#[serde(rename_all = "camelCase")]
pub struct WindowsCompatibilityStatus {
    pub support_level: WindowsSupportLevel,
    pub supported: bool,
    pub os_name: String,
    pub display_version: String,
    pub build_number: u32,
    pub update_build_revision: u32,
    pub process_architecture: String,
    pub native_architecture: String,
    pub webview2_version: Option<String>,
    pub minimum_windows_build: u32,
    pub summary: String,
}

pub fn assess_windows_compatibility(
    facts: WindowsCompatibilityFacts,
) -> WindowsCompatibilityStatus {
    let x64_process = facts.process_architecture == "x86_64";
    let arm64_process = facts.process_architecture == "arm64";
    let native_x64 = facts.native_architecture == "x86_64";
    let native_arm64 = facts.native_architecture == "arm64";
    let support_level = if x64_process && native_x64 && facts.build_number >= WINDOWS_11_FIRST_BUILD
    {
        WindowsSupportLevel::Supported
    } else if (arm64_process && native_arm64 && facts.build_number >= WINDOWS_11_FIRST_BUILD)
        || (x64_process && facts.build_number >= MINIMUM_WINDOWS_BUILD)
    {
        WindowsSupportLevel::Extended
    } else {
        WindowsSupportLevel::Unsupported
    };
    let summary = match support_level {
        WindowsSupportLevel::Supported => {
            "Windows 11 x64 正式支持；当前系统满足付费版运行基线。".to_owned()
        }
        WindowsSupportLevel::Extended if facts.native_architecture == "arm64" => {
            if facts.process_architecture == "arm64" {
                "Windows 11 ARM64 原生版本处于延伸兼容；基础功能已原生验证，本地识别性能仍需单独验收。"
                    .to_owned()
            } else {
                "当前通过 Windows ARM64 的 x64 仿真运行，属于延伸兼容；建议改装 ARM64 原生版本。"
                    .to_owned()
            }
        }
        WindowsSupportLevel::Extended => {
            "Windows 10 x64 延伸兼容；仅建议在仍有 Microsoft 安全更新的 ESU 或 LTSC 环境使用。"
                .to_owned()
        }
        WindowsSupportLevel::Unsupported => format!(
            "当前 Windows 或处理器架构不受支持；需要 Windows 内部版本不低于 {MINIMUM_WINDOWS_BUILD} 的 x64 环境。"
        ),
    };

    let os_name = normalize_os_name(&facts.os_name, facts.build_number);
    WindowsCompatibilityStatus {
        supported: support_level != WindowsSupportLevel::Unsupported,
        support_level,
        os_name,
        display_version: facts.display_version,
        build_number: facts.build_number,
        update_build_revision: facts.update_build_revision,
        process_architecture: facts.process_architecture,
        native_architecture: facts.native_architecture,
        webview2_version: facts.webview2_version,
        minimum_windows_build: MINIMUM_WINDOWS_BUILD,
        summary,
    }
}

fn normalize_os_name(registry_name: &str, build_number: u32) -> String {
    if build_number >= WINDOWS_11_FIRST_BUILD && registry_name.contains("Windows 10") {
        return registry_name.replacen("Windows 10", "Windows 11", 1);
    }
    registry_name.to_owned()
}

pub fn current_windows_compatibility() -> WindowsCompatibilityStatus {
    assess_windows_compatibility(current_windows_facts())
}

#[cfg(windows)]
fn current_windows_facts() -> WindowsCompatibilityFacts {
    let build_number = registry_string(
        windows::Win32::System::Registry::HKEY_LOCAL_MACHINE,
        r"SOFTWARE\Microsoft\Windows NT\CurrentVersion",
        "CurrentBuildNumber",
    )
    .and_then(|value| value.parse().ok())
    .unwrap_or_default();
    let display_version = registry_string(
        windows::Win32::System::Registry::HKEY_LOCAL_MACHINE,
        r"SOFTWARE\Microsoft\Windows NT\CurrentVersion",
        "DisplayVersion",
    )
    .or_else(|| {
        registry_string(
            windows::Win32::System::Registry::HKEY_LOCAL_MACHINE,
            r"SOFTWARE\Microsoft\Windows NT\CurrentVersion",
            "ReleaseId",
        )
    })
    .unwrap_or_else(|| "unknown".to_owned());
    let os_name = registry_string(
        windows::Win32::System::Registry::HKEY_LOCAL_MACHINE,
        r"SOFTWARE\Microsoft\Windows NT\CurrentVersion",
        "ProductName",
    )
    .unwrap_or_else(|| "Windows".to_owned());

    WindowsCompatibilityFacts {
        os_name,
        display_version,
        build_number,
        update_build_revision: registry_dword(
            windows::Win32::System::Registry::HKEY_LOCAL_MACHINE,
            r"SOFTWARE\Microsoft\Windows NT\CurrentVersion",
            "UBR",
        )
        .unwrap_or_default(),
        process_architecture: std::env::consts::ARCH.to_owned(),
        native_architecture: native_architecture(),
        webview2_version: webview2_version(),
    }
}

#[cfg(not(windows))]
fn current_windows_facts() -> WindowsCompatibilityFacts {
    WindowsCompatibilityFacts {
        os_name: std::env::consts::OS.to_owned(),
        display_version: "unknown".to_owned(),
        build_number: 0,
        update_build_revision: 0,
        process_architecture: std::env::consts::ARCH.to_owned(),
        native_architecture: std::env::consts::ARCH.to_owned(),
        webview2_version: None,
    }
}

#[cfg(windows)]
fn native_architecture() -> String {
    use windows::Win32::System::{
        SystemInformation::{
            IMAGE_FILE_MACHINE, IMAGE_FILE_MACHINE_AMD64, IMAGE_FILE_MACHINE_ARM64,
            IMAGE_FILE_MACHINE_I386, IMAGE_FILE_MACHINE_UNKNOWN,
        },
        Threading::{GetCurrentProcess, IsWow64Process2},
    };

    let mut process_machine = IMAGE_FILE_MACHINE_UNKNOWN;
    let mut native_machine = IMAGE_FILE_MACHINE_UNKNOWN;
    let detected = unsafe {
        IsWow64Process2(
            GetCurrentProcess(),
            &mut process_machine,
            Some(&mut native_machine),
        )
    };
    if detected.is_err() {
        return std::env::consts::ARCH.to_owned();
    }
    match native_machine {
        IMAGE_FILE_MACHINE_AMD64 => "x86_64",
        IMAGE_FILE_MACHINE_ARM64 => "arm64",
        IMAGE_FILE_MACHINE_I386 => "x86",
        IMAGE_FILE_MACHINE(_) => "unknown",
    }
    .to_owned()
}

#[cfg(windows)]
fn webview2_version() -> Option<String> {
    use windows::Win32::System::Registry::{HKEY_CURRENT_USER, HKEY_LOCAL_MACHINE};

    const USER_CLIENT_KEY: &str =
        r"SOFTWARE\Microsoft\EdgeUpdate\Clients\{F3017226-FE2A-4295-8BDF-00C3A9A7E4C5}";
    const MACHINE_CLIENT_KEY: &str =
        r"SOFTWARE\WOW6432Node\Microsoft\EdgeUpdate\Clients\{F3017226-FE2A-4295-8BDF-00C3A9A7E4C5}";

    [
        (HKEY_CURRENT_USER, USER_CLIENT_KEY),
        (HKEY_LOCAL_MACHINE, MACHINE_CLIENT_KEY),
    ]
    .into_iter()
    .find_map(|(root, key)| registry_string(root, key, "pv"))
    .filter(|value| !value.is_empty() && value != "0.0.0.0")
}

#[cfg(windows)]
fn registry_string(
    root: windows::Win32::System::Registry::HKEY,
    subkey: &str,
    value_name: &str,
) -> Option<String> {
    use std::{ffi::c_void, os::windows::ffi::OsStrExt as _};
    use windows::{
        Win32::{
            Foundation::ERROR_SUCCESS,
            System::Registry::{RRF_RT_REG_SZ, RegGetValueW},
        },
        core::PCWSTR,
    };

    let subkey = std::ffi::OsStr::new(subkey)
        .encode_wide()
        .chain(std::iter::once(0))
        .collect::<Vec<_>>();
    let value_name = std::ffi::OsStr::new(value_name)
        .encode_wide()
        .chain(std::iter::once(0))
        .collect::<Vec<_>>();
    let mut byte_length = 0_u32;
    let status = unsafe {
        RegGetValueW(
            root,
            PCWSTR(subkey.as_ptr()),
            PCWSTR(value_name.as_ptr()),
            RRF_RT_REG_SZ,
            None,
            None,
            Some(&mut byte_length),
        )
    };
    if status != ERROR_SUCCESS || byte_length < 2 {
        return None;
    }

    let unit_count = usize::try_from(byte_length / 2).ok()?;
    let mut buffer = vec![0_u16; unit_count];
    let status = unsafe {
        RegGetValueW(
            root,
            PCWSTR(subkey.as_ptr()),
            PCWSTR(value_name.as_ptr()),
            RRF_RT_REG_SZ,
            None,
            Some(buffer.as_mut_ptr().cast::<c_void>()),
            Some(&mut byte_length),
        )
    };
    if status != ERROR_SUCCESS {
        return None;
    }
    let content_length = buffer
        .iter()
        .position(|unit| *unit == 0)
        .unwrap_or(buffer.len());
    Some(String::from_utf16_lossy(&buffer[..content_length]))
}

#[cfg(windows)]
fn registry_dword(
    root: windows::Win32::System::Registry::HKEY,
    subkey: &str,
    value_name: &str,
) -> Option<u32> {
    use std::{ffi::c_void, os::windows::ffi::OsStrExt as _};
    use windows::{
        Win32::{
            Foundation::ERROR_SUCCESS,
            System::Registry::{RRF_RT_REG_DWORD, RegGetValueW},
        },
        core::PCWSTR,
    };

    let subkey = std::ffi::OsStr::new(subkey)
        .encode_wide()
        .chain(std::iter::once(0))
        .collect::<Vec<_>>();
    let value_name = std::ffi::OsStr::new(value_name)
        .encode_wide()
        .chain(std::iter::once(0))
        .collect::<Vec<_>>();
    let mut value = 0_u32;
    let mut byte_length = u32::try_from(std::mem::size_of::<u32>()).ok()?;
    let status = unsafe {
        RegGetValueW(
            root,
            PCWSTR(subkey.as_ptr()),
            PCWSTR(value_name.as_ptr()),
            RRF_RT_REG_DWORD,
            None,
            Some((&mut value as *mut u32).cast::<c_void>()),
            Some(&mut byte_length),
        )
    };
    (status == ERROR_SUCCESS && byte_length == 4).then_some(value)
}
