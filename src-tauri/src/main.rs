#![cfg_attr(not(debug_assertions), windows_subsystem = "windows")]

fn main() {
    if let Some(exit_code) = run_windows_self_check_if_requested() {
        std::process::exit(exit_code);
    }

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

    if mistake_trainer_next_lib::run().is_err() {
        if let Some(root) =
            mistake_trainer_next_lib::modules::startup_safety::default_application_data_root()
        {
            let _ = mistake_trainer_next_lib::modules::startup_safety::write_startup_failure_record(
                &root,
                env!("CARGO_PKG_VERSION"),
                current_utc_millis(),
            );
        }
        let _ = rfd::MessageDialog::new()
            .set_title("错题训练器未能启动")
            .set_description(
                "应用启动失败，资料库没有被删除或覆盖。请重新启动电脑后再试；如果问题仍然存在，请在文件资源管理器地址栏输入 %APPDATA%\\com.mistaketrainer.next，找到 startup-failure.json 后联系支持。",
            )
            .set_level(rfd::MessageLevel::Error)
            .set_buttons(rfd::MessageButtons::Ok)
            .show();
        std::process::exit(1);
    }
}

fn run_windows_self_check_if_requested() -> Option<i32> {
    let mut arguments = std::env::args_os().skip(1);
    while let Some(argument) = arguments.next() {
        if argument != "--windows-self-check" {
            continue;
        }
        let Some(output) = arguments.next().map(std::path::PathBuf::from) else {
            return Some(11);
        };
        if !output.is_absolute() {
            return Some(11);
        }
        return Some(
            match mistake_trainer_next_lib::modules::startup_safety::write_windows_self_check(
                &output,
                env!("CARGO_PKG_VERSION"),
                current_utc_millis(),
            ) {
                Ok(
                    mistake_trainer_next_lib::modules::windows_compatibility::WindowsSupportLevel::Supported
                    | mistake_trainer_next_lib::modules::windows_compatibility::WindowsSupportLevel::Extended,
                ) => 0,
                Ok(
                    mistake_trainer_next_lib::modules::windows_compatibility::WindowsSupportLevel::Unsupported,
                ) => 10,
                Err(_) => 11,
            },
        );
    }
    None
}

fn current_utc_millis() -> i64 {
    std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .map(|duration| i64::try_from(duration.as_millis()).unwrap_or(i64::MAX))
        .unwrap_or_default()
}
