#![cfg_attr(not(debug_assertions), windows_subsystem = "windows")]

fn main() {
    if std::env::args().any(|argument| argument == "--export-bindings") {
        let path = std::path::PathBuf::from(env!("CARGO_MANIFEST_DIR"))
            .join("../src/shared/api/bindings.ts");
        mistake_trainer_next_lib::bindings::export_typescript_bindings(&path)
            .expect("failed to export TypeScript bindings");
        return;
    }

    mistake_trainer_next_lib::run();
}
