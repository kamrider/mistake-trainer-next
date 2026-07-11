pub mod application;
pub mod bindings;
pub mod commands;
pub mod domain;
pub mod infrastructure;
pub mod modules;

#[cfg_attr(mobile, tauri::mobile_entry_point)]
pub fn run() {
    let specta = bindings::builder();

    #[cfg(debug_assertions)]
    bindings::export_typescript_bindings(
        &std::path::PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("../src/shared/api/bindings.ts"),
    )
    .expect("failed to export TypeScript bindings");

    tauri::Builder::default()
        .invoke_handler(specta.invoke_handler())
        .setup(move |app| {
            specta.mount_events(app);
            Ok(())
        })
        .run(tauri::generate_context!())
        .expect("failed to run Mistake Trainer Next");
}
