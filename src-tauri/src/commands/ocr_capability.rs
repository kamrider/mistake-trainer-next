use tauri::State;
use uuid::Uuid;

use crate::{
    application::result::AppResult,
    commands::storage::ApplicationControlRoot,
    modules::ocr_capability::{
        OcrCapabilityError, OcrCapabilityManager, OcrCapabilityStatus, OcrComponentId,
        OcrComponentStatus, capability_status, download_client, install_component,
        remove_component,
    },
};

#[tauri::command]
#[specta::specta]
pub async fn ocr_capability_status(
    control_root: State<'_, ApplicationControlRoot>,
    manager: State<'_, OcrCapabilityManager>,
) -> Result<AppResult<OcrCapabilityStatus>, ()> {
    let _guard = manager.lock_mutation().await;
    let root = control_root.0.clone();
    Ok(
        match tauri::async_runtime::spawn_blocking(move || capability_status(&root)).await {
            Ok(Ok(status)) => AppResult::success(status),
            Ok(Err(error)) => capability_failure(&error),
            Err(_) => capability_failure(&OcrCapabilityError::Io(std::io::Error::other(
                "capability worker stopped",
            ))),
        },
    )
}

#[tauri::command]
#[specta::specta]
pub async fn ocr_component_install(
    component_id: OcrComponentId,
    control_root: State<'_, ApplicationControlRoot>,
    manager: State<'_, OcrCapabilityManager>,
) -> Result<AppResult<OcrComponentStatus>, ()> {
    let _guard = manager.lock_mutation().await;
    let client = match download_client() {
        Ok(client) => client,
        Err(error) => return Ok(capability_failure(&error)),
    };
    Ok(
        match install_component(&control_root.0, component_id, &client).await {
            Ok(status) => AppResult::success(status),
            Err(error) => capability_failure(&error),
        },
    )
}

#[tauri::command]
#[specta::specta]
pub async fn ocr_component_remove(
    component_id: OcrComponentId,
    control_root: State<'_, ApplicationControlRoot>,
    manager: State<'_, OcrCapabilityManager>,
) -> Result<AppResult<OcrComponentStatus>, ()> {
    let _guard = manager.lock_mutation().await;
    let root = control_root.0.clone();
    Ok(
        match tauri::async_runtime::spawn_blocking(move || remove_component(&root, component_id))
            .await
        {
            Ok(Ok(status)) => AppResult::success(status),
            Ok(Err(error)) => capability_failure(&error),
            Err(_) => capability_failure(&OcrCapabilityError::Io(std::io::Error::other(
                "component removal worker stopped",
            ))),
        },
    )
}

fn capability_failure<T>(error: &OcrCapabilityError) -> AppResult<T> {
    let (code, user_message, retryable) = match error {
        OcrCapabilityError::Unavailable => (
            "ocr_component_unavailable",
            "这个组件还没有可用于当前软件的安全运行时，未进行下载。",
            false,
        ),
        OcrCapabilityError::UnsupportedHardware => (
            "ocr_hardware_not_suitable",
            "本机没有通过该模型档位的流畅性预检，未进行下载；现有功能保持不变。",
            false,
        ),
        OcrCapabilityError::Request(_) | OcrCapabilityError::HttpStatus => (
            "ocr_component_download_failed",
            "组件没有下载完成，请检查网络后重试；未完成文件已清理。",
            true,
        ),
        OcrCapabilityError::InvalidLength
        | OcrCapabilityError::Integrity
        | OcrCapabilityError::InvalidManifest => (
            "ocr_component_integrity_failed",
            "下载内容没有通过完整性校验，已拒绝安装；请稍后重试。",
            true,
        ),
        OcrCapabilityError::Io(_) | OcrCapabilityError::Serialize(_) => (
            "ocr_component_storage_failed",
            "组件没有安装完成，请检查本机可用空间后重试；现有功能保持不变。",
            true,
        ),
    };
    let diagnostic_id = Uuid::now_v7().to_string();
    eprintln!(
        "optional OCR component operation failed [{diagnostic_id}] {}",
        error.code()
    );
    AppResult::failure(code, user_message, retryable, diagnostic_id)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn integrity_failure_does_not_expose_a_url_or_path() {
        let result: AppResult<OcrComponentStatus> =
            capability_failure(&OcrCapabilityError::Integrity);
        let serialized = serde_json::to_string(&result).unwrap();

        assert!(serialized.contains("ocr_component_integrity_failed"));
        assert!(!serialized.contains("modelscope"));
        assert!(!serialized.contains("http"));
        assert!(!serialized.contains(r"C:\\"));
    }

    #[test]
    fn network_failure_is_retryable_without_claiming_installation() {
        let result: AppResult<OcrComponentStatus> =
            capability_failure(&OcrCapabilityError::HttpStatus);
        let AppResult::Failure { error, .. } = result else {
            panic!("network failure must not have a success shape")
        };

        assert_eq!(error.code, "ocr_component_download_failed");
        assert!(error.retryable);
    }

    #[test]
    fn unsupported_hardware_is_a_stable_non_retryable_decision() {
        let result: AppResult<OcrComponentStatus> =
            capability_failure(&OcrCapabilityError::UnsupportedHardware);
        let AppResult::Failure { error, .. } = result else {
            panic!("hardware rejection must not have a success shape")
        };

        assert_eq!(error.code, "ocr_hardware_not_suitable");
        assert!(!error.retryable);
    }
}
