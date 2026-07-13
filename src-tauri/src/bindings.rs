use std::{error::Error, fs, path::Path};

use specta_typescript::Typescript;
use tauri_specta::{Builder, collect_commands};

use crate::commands;

pub fn builder() -> Builder<tauri::Wry> {
    builder_for::<tauri::Wry>()
}

pub fn export_typescript_bindings(path: &Path) -> Result<(), Box<dyn Error>> {
    builder().export(Typescript::default(), path)?;
    let generated = fs::read_to_string(path)?;
    fs::write(path, format!("{}\n", generated.trim_end()))?;
    Ok(())
}

fn builder_for<R: tauri::Runtime>() -> Builder<R> {
    Builder::<R>::new().commands(collect_commands![
        commands::system::system_status,
        commands::library::library_context,
        commands::library::problem_detail,
        commands::library::problem_change_status,
        commands::library::problem_list,
        commands::library::problem_update,
        commands::review::review_queue,
        commands::review::review_submit,
        commands::capture::capture_commit,
        commands::capture::capture_list,
        commands::capture::capture_remove,
        commands::capture::capture_select,
    ])
}
