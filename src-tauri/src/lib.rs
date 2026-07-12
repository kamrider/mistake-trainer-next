pub mod application;
pub mod bindings;
pub mod commands;
pub mod domain;
pub mod infrastructure;
pub mod modules;

#[cfg_attr(mobile, tauri::mobile_entry_point)]
pub fn run() {
    use tauri::Manager as _;

    let specta = bindings::builder();

    #[cfg(debug_assertions)]
    bindings::export_typescript_bindings(
        &std::path::PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("../src/shared/api/bindings.ts"),
    )
    .expect("failed to export TypeScript bindings");

    tauri::Builder::default()
        .invoke_handler(specta.invoke_handler())
        .setup(move |app| {
            let data_root = app.path().app_data_dir()?.join("library");
            let secrets = infrastructure::runtime::KeyringSecretStore::new(
                "com.mistaketrainer.next.local-library",
            );
            let runtime = infrastructure::runtime::initialize_local_library(
                &data_root,
                &secrets,
                current_utc_millis(),
            )?;
            app.manage(runtime);
            app.manage(modules::capture::CaptureStage::default());
            specta.mount_events(app);
            Ok(())
        })
        .run(tauri::generate_context!())
        .expect("failed to run Mistake Trainer Next");
}

fn current_utc_millis() -> i64 {
    std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .map(|duration| i64::try_from(duration.as_millis()).unwrap_or(i64::MAX))
        .unwrap_or_default()
}
