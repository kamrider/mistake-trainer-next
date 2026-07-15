use std::{error::Error, fs, path::Path};

use specta_typescript::Typescript;
use tauri_specta::{Builder, collect_commands};

use crate::commands;

pub fn builder() -> Builder<tauri::Wry> {
    Builder::<tauri::Wry>::new().commands(collect_commands![
        commands::backup::backup_create,
        commands::backup::backup_validate,
        commands::system::system_status,
        commands::library::library_context,
        commands::library::problem_detail,
        commands::library::problem_change_status,
        commands::library::problem_list,
        commands::library::problem_update,
        commands::legacy::legacy_scan,
        commands::review::review_queue,
        commands::review::review_submit,
        commands::insights::report_summary,
        commands::insights::settings_overview,
        commands::exports::export_list,
        commands::exports::export_trash_list,
        commands::exports::export_create,
        commands::exports::export_generate,
        commands::exports::export_delete,
        commands::exports::export_restore,
        commands::capture_inbox::capture_batch_create,
        commands::capture_inbox::capture_batch_list,
        commands::capture_inbox::capture_batch_detail,
        commands::capture_inbox::capture_batch_update,
        commands::capture_inbox::capture_batch_discard,
        commands::capture_inbox::capture_import_select,
        commands::capture_inbox::capture_import_bytes,
        commands::capture_inbox::capture_item_preview,
        commands::capture_inbox::capture_item_remove,
        commands::capture_inbox::capture_layout_apply,
        commands::capture_inbox::capture_draft_create,
        commands::capture_inbox::capture_item_move,
        commands::capture_inbox::capture_draft_update,
        commands::capture_inbox::capture_commit_ready,
    ])
}

pub fn export_typescript_bindings(path: &Path) -> Result<(), Box<dyn Error>> {
    builder().export(Typescript::default(), path)?;
    let generated = fs::read_to_string(path)?;
    fs::write(path, format!("{}\n", generated.trim_end()))?;
    Ok(())
}
