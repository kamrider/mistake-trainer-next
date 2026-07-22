use std::{error::Error, fs, path::Path};

use specta_typescript::Typescript;
use tauri_specta::{Builder, collect_commands};

use crate::commands;

pub fn builder() -> Builder<tauri::Wry> {
    Builder::<tauri::Wry>::new().commands(collect_commands![
        commands::backup::backup_create,
        commands::backup::backup_prepare_restore,
        commands::backup::backup_restore,
        commands::backup::backup_restore_status,
        commands::system::system_status,
        commands::profiles::profile_list,
        commands::profiles::profile_create,
        commands::profiles::profile_rename,
        commands::profiles::profile_select,
        commands::library::library_context,
        commands::library::problem_detail,
        commands::library::problem_change_status,
        commands::library::problem_list,
        commands::library::problem_update,
        commands::legacy::legacy_scan,
        commands::legacy::legacy_import,
        commands::legacy::legacy_import_list,
        commands::legacy::legacy_rollback,
        commands::review::review_queue,
        commands::review::review_current_problem,
        commands::review::review_manual_start,
        commands::review::review_exam_start,
        commands::review::review_exam_navigate,
        commands::review::review_exam_begin_grading,
        commands::review::review_submit,
        commands::review::review_focus_select,
        commands::review::review_focus_skip,
        commands::review_history::review_history_list,
        commands::review_history::review_history_detail,
        commands::insights::dashboard_overview,
        commands::insights::report_summary,
        commands::insights::settings_overview,
        commands::preferences::subject_preferences_get,
        commands::preferences::subject_preferences_save,
        commands::preferences::review_preferences_get,
        commands::preferences::review_preferences_save,
        commands::exports::export_candidates,
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
        commands::capture_inbox::capture_batch_assign_subject,
        commands::capture_inbox::capture_batch_discard,
        commands::capture_inbox::capture_import_select,
        commands::capture_inbox::capture_import_bytes,
        commands::capture_inbox::capture_item_preview,
        commands::capture_inbox::capture_item_remove,
        commands::capture_inbox::capture_layout_apply,
        commands::capture_inbox::capture_item_move,
        commands::capture_inbox::capture_item_stage_role,
        commands::capture_inbox::capture_card_merge,
        commands::capture_inbox::capture_draft_delete,
        commands::capture_inbox::capture_draft_update,
        commands::capture_inbox::capture_commit_ready,
        commands::capture_lan::capture_lan_addresses,
        commands::capture_lan::capture_lan_preflight,
        commands::capture_lan::capture_lan_firewall_repair,
        commands::capture_lan::capture_lan_start,
        commands::capture_lan::capture_lan_status,
        commands::capture_lan::capture_lan_stop,
        commands::sync::sync_backend_status,
        commands::sync::sync_backend_set,
        commands::sync::auth_status_command,
        commands::sync::auth_sign_up,
        commands::sync::auth_sign_in,
        commands::sync::auth_restore,
        commands::sync::auth_disconnect,
        commands::sync::sync_now,
    ])
}

pub fn export_typescript_bindings(path: &Path) -> Result<(), Box<dyn Error>> {
    builder().export(Typescript::default(), path)?;
    let generated = fs::read_to_string(path)?;
    fs::write(path, format!("{}\n", generated.trim_end()))?;
    Ok(())
}
