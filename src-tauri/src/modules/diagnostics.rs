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

const REPORT_SCHEMA_VERSION: u32 = 1;
const APPLICATION_NAME: &str = "Mistake Trainer Next";

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

#[derive(Debug, Serialize)]
#[serde(rename_all = "camelCase")]
struct DiagnosticReport {
    schema_version: u32,
    report_id: String,
    generated_at_utc_ms: i64,
    application: DiagnosticApplication,
    library: DiagnosticLibrary,
    sync: DiagnosticSync,
    warnings: Vec<DiagnosticWarning>,
}

#[derive(Debug, Serialize)]
#[serde(rename_all = "camelCase")]
struct DiagnosticApplication {
    name: &'static str,
    version: String,
    platform: &'static str,
    architecture: &'static str,
}

#[derive(Debug, Serialize)]
#[serde(rename_all = "camelCase")]
struct DiagnosticLibrary {
    storage_kind: DiagnosticStorageKind,
    schema_version: u64,
    integrity: DiagnosticIntegrity,
    profile_count: u64,
    problem_count: u64,
    asset_count: u64,
    capture_batch_count: u64,
    review_event_count: u64,
    export_snapshot_count: u64,
}

#[derive(Debug, Serialize)]
#[serde(rename_all = "snake_case")]
enum DiagnosticIntegrity {
    Ok,
    Failed,
}

#[derive(Debug, Serialize)]
#[serde(rename_all = "camelCase")]
struct DiagnosticSync {
    pending_operation_count: u64,
    failed_operation_count: u64,
    unresolved_conflict_count: u64,
}

#[derive(Debug, Serialize)]
#[serde(rename_all = "camelCase")]
struct DiagnosticWarning {
    code: &'static str,
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
        build_report(&connection, &report_id, context)?
    };
    let warning_count = u32::try_from(report.warnings.len()).unwrap_or(u32::MAX);
    let bytes = serde_json::to_vec_pretty(&report)?;
    write_report_atomically(destination, &file_label, &report_id, &bytes)?;

    Ok(DiagnosticExportReceipt {
        report_id,
        file_label,
        generated_at_utc_ms: context.now_utc_ms as f64,
        warning_count,
    })
}

fn build_report(
    connection: &Connection,
    report_id: &str,
    context: DiagnosticContext<'_>,
) -> Result<DiagnosticReport, DiagnosticError> {
    let schema_version = connection
        .pragma_query_value(None, "user_version", |row| row.get::<_, i64>(0))
        .map(non_negative)?;
    let integrity = connection
        .query_row("PRAGMA quick_check(1)", [], |row| row.get::<_, String>(0))
        .map(|value| {
            if value == "ok" {
                DiagnosticIntegrity::Ok
            } else {
                DiagnosticIntegrity::Failed
            }
        })?;
    let warnings = if matches!(integrity, DiagnosticIntegrity::Failed) {
        vec![DiagnosticWarning {
            code: "library_integrity_check_failed",
        }]
    } else {
        Vec::new()
    };

    Ok(DiagnosticReport {
        schema_version: REPORT_SCHEMA_VERSION,
        report_id: report_id.to_owned(),
        generated_at_utc_ms: context.now_utc_ms,
        application: DiagnosticApplication {
            name: APPLICATION_NAME,
            version: context.app_version.to_owned(),
            platform: std::env::consts::OS,
            architecture: std::env::consts::ARCH,
        },
        library: DiagnosticLibrary {
            storage_kind: context.storage_kind,
            schema_version,
            integrity,
            profile_count: count(connection, "learner_profiles", None)?,
            problem_count: count(connection, "problems", None)?,
            asset_count: count(connection, "assets", None)?,
            capture_batch_count: count(connection, "capture_batches", None)?,
            review_event_count: count(connection, "review_events", None)?,
            export_snapshot_count: count(connection, "export_snapshots", None)?,
        },
        sync: DiagnosticSync {
            pending_operation_count: count(
                connection,
                "sync_operations",
                Some("status = 'pending'"),
            )?,
            failed_operation_count: count(
                connection,
                "sync_operations",
                Some("status = 'failed'"),
            )?,
            unresolved_conflict_count: count(
                connection,
                "sync_conflicts",
                Some("resolved_at_utc_ms IS NULL"),
            )?,
        },
        warnings,
    })
}

fn count(
    connection: &Connection,
    trusted_table: &'static str,
    trusted_filter: Option<&'static str>,
) -> Result<u64, DiagnosticError> {
    let sql = match trusted_filter {
        Some(filter) => format!("SELECT COUNT(*) FROM {trusted_table} WHERE {filter}"),
        None => format!("SELECT COUNT(*) FROM {trusted_table}"),
    };
    connection
        .query_row(&sql, [], |row| row.get::<_, i64>(0))
        .map(non_negative)
        .map_err(Into::into)
}

fn non_negative(value: i64) -> u64 {
    u64::try_from(value).unwrap_or_default()
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
