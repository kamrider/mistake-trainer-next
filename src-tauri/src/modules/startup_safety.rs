use std::{
    fs::{self, OpenOptions},
    io::Write as _,
    path::{Path, PathBuf},
};

use serde::Serialize;

use super::windows_compatibility::{
    WindowsCompatibilityStatus, WindowsSupportLevel, current_windows_compatibility,
};

pub const STARTUP_FAILURE_FILE_NAME: &str = "startup-failure.json";
pub const SELF_CHECK_SCHEMA_VERSION: u32 = 1;

#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct StartupFailureRecord {
    pub schema_version: u32,
    pub application_version: String,
    pub occurred_at_utc_ms: i64,
    pub reason_code: &'static str,
}

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct WindowsSelfCheckReport {
    pub schema_version: u32,
    pub application_version: String,
    pub checked_at_utc_ms: i64,
    pub windows: WindowsCompatibilityStatus,
}

pub fn write_startup_failure_record(
    application_data_root: &Path,
    application_version: &str,
    occurred_at_utc_ms: i64,
) -> std::io::Result<PathBuf> {
    fs::create_dir_all(application_data_root)?;
    let final_path = application_data_root.join(STARTUP_FAILURE_FILE_NAME);
    let temporary_path = application_data_root.join(format!(
        ".startup-failure-{}-{}.tmp",
        std::process::id(),
        occurred_at_utc_ms
    ));
    let record = StartupFailureRecord {
        schema_version: 1,
        application_version: application_version.to_owned(),
        occurred_at_utc_ms,
        reason_code: "tauri_startup_failed",
    };
    let bytes = serde_json::to_vec_pretty(&record).map_err(std::io::Error::other)?;

    let result = (|| {
        write_new_synced(&temporary_path, &bytes)?;
        replace_file_atomically(&temporary_path, &final_path)
    })();
    if result.is_err() {
        let _ = fs::remove_file(&temporary_path);
    }
    result.map(|()| final_path)
}

pub fn write_windows_self_check(
    output_path: &Path,
    application_version: &str,
    checked_at_utc_ms: i64,
) -> std::io::Result<WindowsSupportLevel> {
    let report = WindowsSelfCheckReport {
        schema_version: SELF_CHECK_SCHEMA_VERSION,
        application_version: application_version.to_owned(),
        checked_at_utc_ms,
        windows: current_windows_compatibility(),
    };
    let support_level = report.windows.support_level;
    let bytes = serde_json::to_vec_pretty(&report).map_err(std::io::Error::other)?;
    write_new_synced(output_path, &bytes)?;
    Ok(support_level)
}

pub fn default_application_data_root() -> Option<PathBuf> {
    std::env::var_os("APPDATA")
        .map(PathBuf::from)
        .map(|root| root.join("com.mistaketrainer.next"))
}

fn write_new_synced(path: &Path, bytes: &[u8]) -> std::io::Result<()> {
    let mut output = OpenOptions::new().write(true).create_new(true).open(path)?;
    output.write_all(bytes)?;
    output.sync_all()
}

#[cfg(windows)]
fn replace_file_atomically(source: &Path, target: &Path) -> std::io::Result<()> {
    use std::os::windows::ffi::OsStrExt as _;

    use windows::{
        Win32::Storage::FileSystem::{
            MOVEFILE_REPLACE_EXISTING, MOVEFILE_WRITE_THROUGH, MoveFileExW,
        },
        core::PCWSTR,
    };

    let source = source
        .as_os_str()
        .encode_wide()
        .chain(std::iter::once(0))
        .collect::<Vec<_>>();
    let target = target
        .as_os_str()
        .encode_wide()
        .chain(std::iter::once(0))
        .collect::<Vec<_>>();
    unsafe {
        MoveFileExW(
            PCWSTR::from_raw(source.as_ptr()),
            PCWSTR::from_raw(target.as_ptr()),
            MOVEFILE_REPLACE_EXISTING | MOVEFILE_WRITE_THROUGH,
        )
        .map_err(|error| std::io::Error::other(error.to_string()))
    }
}

#[cfg(not(windows))]
fn replace_file_atomically(source: &Path, target: &Path) -> std::io::Result<()> {
    if target.exists() {
        fs::remove_file(target)?;
    }
    fs::rename(source, target)
}
