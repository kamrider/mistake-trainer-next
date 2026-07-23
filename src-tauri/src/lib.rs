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
            let control_root = app.path().app_data_dir()?;
            let secrets = infrastructure::runtime::KeyringSecretStore::new(
                "com.mistaketrainer.next.local-library",
            );
            let access_gate =
                match application::startup::initialize_configured_application_library_if_accessible(
                    &control_root,
                    &secrets,
                    current_utc_millis(),
                )? {
                    application::startup::LibraryStartup::Ready(runtime) => {
                        app.manage(runtime);
                        commands::access::LibraryAccessGate::unlocked()
                    }
                    application::startup::LibraryStartup::Locked => {
                        commands::access::LibraryAccessGate::locked()
                    }
                    application::startup::LibraryStartup::AccessUnavailable(error) => {
                        eprintln!("library access gate failed closed [{}]", error.code());
                        match error {
                            application::startup::StartupAccessUnavailable::Credentials(_) => {
                                commands::access::LibraryAccessGate::unavailable()
                            }
                            application::startup::StartupAccessUnavailable::Storage(_) => {
                                commands::access::LibraryAccessGate::storage_unavailable()
                            }
                            application::startup::StartupAccessUnavailable::StorageMigration(_) => {
                                commands::access::LibraryAccessGate::storage_unavailable()
                            }
                        }
                    }
                };
            app.manage(commands::storage::ApplicationControlRoot(control_root));
            app.manage(access_gate);
            app.manage(modules::auth_sync::AuthSyncManager::default());
            app.manage(modules::auth_sync::CloudAuthRuntime::from_build_environment());
            app.manage(modules::capture_lan::CaptureLanManager::default());
            app.manage(modules::legacy::LegacyImportManager::default());
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
