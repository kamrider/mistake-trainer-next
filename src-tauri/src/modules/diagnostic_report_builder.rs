use rusqlite::Connection;
use serde::Serialize;

use super::{DiagnosticContext, DiagnosticError, DiagnosticStorageKind};
use crate::modules::{
    startup_safety::StartupFailureRecord,
    windows_compatibility::{WindowsCompatibilityStatus, WindowsSupportLevel},
};

const REPORT_SCHEMA_VERSION: u32 = 3;
const APPLICATION_NAME: &str = "Mistake Trainer Next";

#[derive(Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub(super) struct DiagnosticReport {
    schema_version: u32,
    report_id: String,
    generated_at_utc_ms: i64,
    application: DiagnosticApplication,
    library: DiagnosticLibrary,
    sync: DiagnosticSync,
    warnings: Vec<DiagnosticWarning>,
}

impl DiagnosticReport {
    pub(super) fn warning_count(&self) -> u32 {
        u32::try_from(self.warnings.len()).unwrap_or(u32::MAX)
    }
}

#[derive(Debug, Serialize)]
#[serde(rename_all = "camelCase")]
struct DiagnosticApplication {
    name: &'static str,
    version: String,
    platform: &'static str,
    architecture: &'static str,
    windows: WindowsCompatibilityStatus,
    #[serde(skip_serializing_if = "Option::is_none")]
    last_startup_failure: Option<StartupFailureRecord>,
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

pub(super) fn build_report(
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
    let mut warnings = if matches!(integrity, DiagnosticIntegrity::Failed) {
        vec![DiagnosticWarning {
            code: "library_integrity_check_failed",
        }]
    } else {
        Vec::new()
    };
    match context.windows_compatibility.support_level {
        WindowsSupportLevel::Unsupported => warnings.push(DiagnosticWarning {
            code: "windows_release_unsupported",
        }),
        WindowsSupportLevel::Extended => warnings.push(DiagnosticWarning {
            code: "windows_extended_support_only",
        }),
        WindowsSupportLevel::Supported => {}
    }
    if context.windows_compatibility.webview2_version.is_none() {
        warnings.push(DiagnosticWarning {
            code: "webview2_runtime_not_detected",
        });
    }
    if context.startup_failure.is_some() {
        warnings.push(DiagnosticWarning {
            code: "previous_startup_failure_detected",
        });
    }

    Ok(DiagnosticReport {
        schema_version: REPORT_SCHEMA_VERSION,
        report_id: report_id.to_owned(),
        generated_at_utc_ms: context.now_utc_ms,
        application: DiagnosticApplication {
            name: APPLICATION_NAME,
            version: context.app_version.to_owned(),
            platform: std::env::consts::OS,
            architecture: std::env::consts::ARCH,
            windows: context.windows_compatibility.clone(),
            last_startup_failure: context.startup_failure.cloned(),
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
