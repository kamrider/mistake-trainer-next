#![cfg_attr(not(debug_assertions), windows_subsystem = "windows")]

fn main() {
    if let Some(exit_code) =
        mistake_trainer_next_lib::modules::capture_firewall::run_capture_firewall_helper_if_requested()
    {
        std::process::exit(exit_code);
    }

    if std::env::args().any(|argument| argument == "--export-bindings") {
        let path = std::path::PathBuf::from(env!("CARGO_MANIFEST_DIR"))
            .join("../src/shared/api/bindings.ts");
        mistake_trainer_next_lib::bindings::export_typescript_bindings(&path)
            .expect("failed to export TypeScript bindings");
        return;
    }

    mistake_trainer_next_lib::run();
}
