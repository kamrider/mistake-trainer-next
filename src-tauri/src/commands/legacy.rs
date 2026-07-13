use std::path::PathBuf;

use uuid::Uuid;

use crate::{
    application::result::AppResult,
    modules::legacy::{LegacyScanReport, scan_legacy_storage},
};

#[tauri::command]
#[specta::specta]
pub async fn legacy_scan() -> AppResult<Option<LegacyScanReport>> {
    let worker = tauri::async_runtime::spawn_blocking(|| {
        let root = rfd::FileDialog::new()
            .set_title("选择旧版错题软件数据目录")
            .pick_folder();
        scan_selected_root(root)
    });
    match worker.await {
        Ok(Ok(report)) => AppResult::success(report),
        Ok(Err(_)) | Err(_) => AppResult::failure(
            "legacy_scan_failed",
            "旧版目录没有扫描成功；原目录未被修改，请检查选择后重试。",
            true,
            Uuid::now_v7().to_string(),
        ),
    }
}

fn scan_selected_root(
    root: Option<PathBuf>,
) -> Result<Option<LegacyScanReport>, crate::modules::legacy::LegacyScanError> {
    root.map(|path| scan_legacy_storage(&path)).transpose()
}

#[cfg(test)]
mod tests {
    use super::scan_selected_root;

    #[test]
    fn cancelling_the_folder_picker_returns_no_report() {
        assert_eq!(scan_selected_root(None).expect("cancel succeeds"), None);
    }
}
