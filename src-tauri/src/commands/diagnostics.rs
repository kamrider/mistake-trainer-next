use std::{path::PathBuf, sync::Arc};

use tauri::State;
use uuid::Uuid;

use crate::{
    application::result::AppResult,
    commands::storage::ApplicationControlRoot,
    infrastructure::{
        runtime::LibraryRuntime,
        storage_location::{StorageLocationError, resolve_storage},
    },
    modules::diagnostics::{
        DiagnosticContext, DiagnosticError, DiagnosticExportReceipt, DiagnosticStorageKind,
        export_diagnostic_report,
    },
    modules::windows_compatibility::current_windows_compatibility,
};

#[tauri::command]
#[specta::specta]
pub async fn diagnostics_export(
    runtime: State<'_, LibraryRuntime>,
    control_root: State<'_, ApplicationControlRoot>,
) -> Result<AppResult<Option<DiagnosticExportReceipt>>, ()> {
    let connection = Arc::clone(&runtime.connection);
    let control_root = control_root.0.clone();
    drop(runtime);

    let selected = tauri::async_runtime::spawn_blocking(|| {
        rfd::FileDialog::new()
            .set_title("选择安全诊断报告保存位置")
            .pick_folder()
    })
    .await;
    let selected = match selected {
        Ok(selected) => match require_selected_destination(selected) {
            Ok(selected) => selected,
            Err(cancelled) => return Ok(cancelled),
        },
        Err(_) => {
            return Ok(AppResult::failure(
                "diagnostics_dialog_failed",
                "文件夹选择窗口没有正常打开，请稍后重试。",
                true,
                Uuid::now_v7().to_string(),
            ));
        }
    };

    let worker = tauri::async_runtime::spawn_blocking(move || {
        let storage = resolve_storage(&control_root).map_err(DiagnosticCommandError::Storage)?;
        let storage_kind = if storage.is_custom() {
            DiagnosticStorageKind::Custom
        } else {
            DiagnosticStorageKind::Default
        };
        let windows_compatibility = current_windows_compatibility();
        export_diagnostic_report(
            &connection,
            &selected,
            DiagnosticContext {
                app_version: env!("CARGO_PKG_VERSION"),
                storage_kind,
                now_utc_ms: current_utc_millis(),
                windows_compatibility: &windows_compatibility,
            },
        )
        .map(Some)
        .map_err(DiagnosticCommandError::Report)
    })
    .await;

    Ok(match worker {
        Ok(Ok(receipt)) => AppResult::success(receipt),
        Ok(Err(error)) => diagnostics_failure(&error),
        Err(_) => diagnostics_failure(&DiagnosticCommandError::Report(DiagnosticError::Lock)),
    })
}

fn require_selected_destination(
    selected: Option<PathBuf>,
) -> Result<PathBuf, AppResult<Option<DiagnosticExportReceipt>>> {
    selected.ok_or_else(|| AppResult::success(None))
}

enum DiagnosticCommandError {
    Storage(StorageLocationError),
    Report(DiagnosticError),
}

fn diagnostics_failure<T>(error: &DiagnosticCommandError) -> AppResult<T> {
    let (code, user_message, retryable) = match error {
        DiagnosticCommandError::Storage(_) => (
            "diagnostics_storage_status_failed",
            "无法确认当前资料库状态，诊断报告没有生成；请先检查存储位置后重试。",
            true,
        ),
        DiagnosticCommandError::Report(DiagnosticError::InvalidDestination) => (
            "diagnostics_destination_invalid",
            "这个位置不能保存诊断报告，请选择一个可写文件夹。",
            true,
        ),
        DiagnosticCommandError::Report(DiagnosticError::Database(_) | DiagnosticError::Lock) => (
            "diagnostics_library_busy",
            "资料库正在忙碌，诊断报告没有生成，请稍后重试。",
            true,
        ),
        DiagnosticCommandError::Report(DiagnosticError::Serialize(_) | DiagnosticError::Io(_)) => (
            "diagnostics_export_failed",
            "诊断报告没有生成，现有资料不会受到影响；请检查磁盘空间和保存位置后重试。",
            true,
        ),
    };
    let internal_code = match error {
        DiagnosticCommandError::Storage(error) => error.code(),
        DiagnosticCommandError::Report(error) => error.code(),
    };
    let diagnostic_id = Uuid::now_v7().to_string();
    eprintln!("diagnostic export failed [{diagnostic_id}] {internal_code}");
    AppResult::failure(code, user_message, retryable, diagnostic_id)
}

fn current_utc_millis() -> i64 {
    std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .map(|duration| i64::try_from(duration.as_millis()).unwrap_or(i64::MAX))
        .unwrap_or_default()
}

#[cfg(test)]
mod tests {
    use super::{DiagnosticCommandError, diagnostics_failure, require_selected_destination};
    use crate::{
        application::result::AppResult,
        modules::diagnostics::{DiagnosticError, DiagnosticExportReceipt},
    };

    #[test]
    fn cancelling_the_native_folder_dialog_is_a_successful_noop() {
        let result = require_selected_destination(None).expect_err("selection should be absent");
        assert!(matches!(result, AppResult::Success { data: None, .. }));
    }

    #[test]
    fn public_failures_do_not_interpolate_internal_io_details() {
        let error = DiagnosticCommandError::Report(DiagnosticError::Io(std::io::Error::new(
            std::io::ErrorKind::PermissionDenied,
            r"C:\Users\Private\secret-path",
        )));
        let result: AppResult<Option<DiagnosticExportReceipt>> = diagnostics_failure(&error);
        let serialized = serde_json::to_string(&result).unwrap();

        assert!(!serialized.contains("secret-path"));
        assert!(!serialized.contains(r"C:\Users"));
        assert!(serialized.contains("diagnostics_export_failed"));
    }
}
