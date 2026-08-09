use std::{
    fs::{self, OpenOptions},
    io::Write,
    path::Path,
    sync::Mutex,
};

use rusqlite::Connection;
use serde::Serialize;
use specta::Type;
use thiserror::Error;
use uuid::Uuid;

use super::{
    startup_safety::StartupFailureRecord, windows_compatibility::WindowsCompatibilityStatus,
};

#[path = "diagnostic_report_builder.rs"]
mod report_builder;

#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Type)]
#[serde(rename_all = "snake_case")]
pub enum DiagnosticStorageKind {
    Default,
    Custom,
}

#[derive(Clone, Copy, Debug)]
pub struct DiagnosticContext<'a> {
    pub app_version: &'a str,
    pub storage_kind: DiagnosticStorageKind,
    pub now_utc_ms: i64,
    pub windows_compatibility: &'a WindowsCompatibilityStatus,
    pub startup_failure: Option<&'a StartupFailureRecord>,
}

#[derive(Clone, Debug, PartialEq, Serialize, Type)]
#[serde(rename_all = "camelCase")]
pub struct DiagnosticExportReceipt {
    pub report_id: String,
    pub file_label: String,
    pub generated_at_utc_ms: f64,
    pub warning_count: u32,
}

#[derive(Debug, Error)]
pub enum DiagnosticError {
    #[error("diagnostic destination is not a directory")]
    InvalidDestination,
    #[error("diagnostic database is unavailable")]
    Database(#[from] rusqlite::Error),
    #[error("diagnostic database lock is unavailable")]
    Lock,
    #[error("diagnostic report could not be serialized")]
    Serialize(#[from] serde_json::Error),
    #[error("diagnostic report could not be written")]
    Io(#[from] std::io::Error),
}

impl DiagnosticError {
    pub fn code(&self) -> &'static str {
        match self {
            Self::InvalidDestination => "invalid_destination",
            Self::Database(_) => "database",
            Self::Lock => "lock",
            Self::Serialize(_) => "serialize",
            Self::Io(_) => "io",
        }
    }
}

pub fn export_diagnostic_report(
    connection: &Mutex<Connection>,
    destination: &Path,
    context: DiagnosticContext<'_>,
) -> Result<DiagnosticExportReceipt, DiagnosticError> {
    ensure_destination_directory(destination)?;

    let report_id = Uuid::now_v7().to_string();
    let short_id = report_id.replace('-', "");
    let file_label = format!(
        "Mistake-Trainer-Diagnostics-{}-{}.json",
        context.now_utc_ms,
        &short_id[..8],
    );
    let report = {
        let connection = connection.lock().map_err(|_| DiagnosticError::Lock)?;
        report_builder::build_report(&connection, &report_id, context)?
    };
    let warning_count = report.warning_count();
    let bytes = serde_json::to_vec_pretty(&report)?;
    write_report_atomically(destination, &file_label, &report_id, &bytes)?;

    Ok(DiagnosticExportReceipt {
        report_id,
        file_label,
        generated_at_utc_ms: context.now_utc_ms as f64,
        warning_count,
    })
}

fn ensure_destination_directory(destination: &Path) -> Result<(), DiagnosticError> {
    match fs::metadata(destination) {
        Ok(metadata) if metadata.is_dir() => Ok(()),
        Ok(_) => Err(DiagnosticError::InvalidDestination),
        Err(error) if error.kind() == std::io::ErrorKind::NotFound => {
            Err(DiagnosticError::InvalidDestination)
        }
        Err(error) => Err(DiagnosticError::Io(error)),
    }
}

fn write_report_atomically(
    destination: &Path,
    file_label: &str,
    report_id: &str,
    bytes: &[u8],
) -> Result<(), DiagnosticError> {
    let final_path = destination.join(file_label);
    let temporary_path = destination.join(format!(".mistake-trainer-diagnostic-{report_id}.tmp"));
    let write_result = (|| {
        let mut output = OpenOptions::new()
            .write(true)
            .create_new(true)
            .open(&temporary_path)?;
        output.write_all(bytes)?;
        output.sync_all()?;
        drop(output);
        if final_path.exists() {
            return Err(std::io::Error::new(
                std::io::ErrorKind::AlreadyExists,
                "diagnostic report already exists",
            ));
        }
        fs::rename(&temporary_path, &final_path)
    })();
    if write_result.is_err() {
        let _ = fs::remove_file(&temporary_path);
    }
    write_result.map_err(Into::into)
}

#[cfg(test)]
mod tests {
    use super::write_report_atomically;
    use tempfile::tempdir;

    #[test]
    fn an_existing_final_report_is_never_overwritten_and_temp_is_removed() {
        let destination = tempdir().unwrap();
        let final_path = destination.path().join("diagnostic.json");
        std::fs::write(&final_path, b"existing").unwrap();

        let error = write_report_atomically(
            destination.path(),
            "diagnostic.json",
            "fixed-report-id",
            b"replacement",
        )
        .unwrap_err();

        assert_eq!(error.code(), "io");
        assert_eq!(std::fs::read(final_path).unwrap(), b"existing");
        assert!(
            !destination
                .path()
                .join(".mistake-trainer-diagnostic-fixed-report-id.tmp")
                .exists()
        );
    }
}
