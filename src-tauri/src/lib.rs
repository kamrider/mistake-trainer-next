#![allow(
    clippy::drop_non_drop,
    clippy::enum_variant_names,
    clippy::manual_async_fn,
    clippy::result_large_err,
    clippy::too_many_arguments
)]

pub mod application;
pub mod bindings;
pub mod commands;
pub mod domain;
pub mod infrastructure;
pub mod modules;

#[cfg_attr(mobile, tauri::mobile_entry_point)]
pub fn run() -> tauri::Result<()> {
    use tauri::Manager as _;

    let specta = bindings::builder();
    let context = tauri::generate_context!();

    #[cfg(debug_assertions)]
    bindings::export_typescript_bindings(
        &std::path::PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("../src/shared/api/bindings.ts"),
    )
    .expect("failed to export TypeScript bindings");

    let builder = tauri::Builder::default();
    #[cfg(windows)]
    let builder = builder.plugin(tauri_plugin_single_instance::init(|app, _args, _cwd| {
        if let Some(window) = app.get_webview_window("main") {
            let _ = window.unminimize();
            let _ = window.show();
            let _ = window.set_focus();
        }
    }));
    #[cfg(windows)]
    let builder =
        if commands::updates::updater_config_is_ready(context.config().plugins.0.get("updater")) {
            builder.plugin(tauri_plugin_updater::Builder::new().build())
        } else {
            builder
        };

    builder
        .invoke_handler(specta.invoke_handler())
        .setup(move |app| {
            let control_root = app.path().app_data_dir()?;
            let private_recognition_temp = control_root.join("recognition-private-temp");
            if private_recognition_temp.exists()
                && std::fs::remove_dir_all(&private_recognition_temp).is_err()
            {
                eprintln!("capture recognition private temporary cleanup failed");
            }
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
                        {
                            let reset_result = runtime.connection.lock().map_err(|_| {
                                std::io::Error::other("library lock poisoned during startup")
                            })?;
                            let mut connection = reset_result;
                            if modules::capture_recognition::reset_abandoned_recognition_work(
                                &mut connection,
                                current_utc_millis(),
                            )
                            .is_err()
                            {
                                eprintln!("capture recognition recovery failed closed");
                            }
                        }
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
            let recognition_manager =
                infrastructure::capture_recognition_worker::CaptureRecognitionManager::for_product(
                    &control_root,
                    &private_recognition_temp,
                );
            app.manage(commands::storage::ApplicationControlRoot(control_root));
            app.manage(access_gate);
            app.manage(modules::auth_sync::AuthSyncManager::default());
            app.manage(modules::auth_sync::CloudAuthRuntime::from_build_environment());
            app.manage(modules::sync_coordinator::SyncCoordinator::default());
            app.manage(modules::capture_lan::CaptureLanManager::default());
            app.manage(modules::legacy::LegacyImportManager::default());
            app.manage(modules::ocr_capability::OcrCapabilityManager::default());
            app.manage(recognition_manager);
            specta.mount_events(app);
            Ok(())
        })
        .on_window_event(|window, event| {
            if matches!(event, tauri::WindowEvent::Destroyed) {
                let manager = window
                    .state::<infrastructure::capture_recognition_worker::CaptureRecognitionManager>(
                    )
                    .inner()
                    .clone();
                tauri::async_runtime::block_on(async {
                    manager.shutdown().await;
                    let _ = manager
                        .wait_for_idle(std::time::Duration::from_secs(5))
                        .await;
                });
                let private_temp_root = window
                    .state::<commands::storage::ApplicationControlRoot>()
                    .0
                    .join("recognition-private-temp");
                let _ = std::fs::remove_dir_all(private_temp_root);
            }
        })
        .run(context)
}

fn current_utc_millis() -> i64 {
    std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .map(|duration| i64::try_from(duration.as_millis()).unwrap_or(i64::MAX))
        .unwrap_or_default()
}
