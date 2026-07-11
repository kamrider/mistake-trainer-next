use std::{error::Error, fs, path::Path};

use specta_typescript::Typescript;
use tauri_specta::Builder;

use crate::commands::system::specta_commands;

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
    Builder::<R>::new().commands(specta_commands::<R>())
}
