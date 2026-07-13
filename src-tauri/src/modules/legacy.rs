use std::{
    collections::{BTreeMap, HashMap},
    fs,
    io::{self, Read},
    path::{Component, Path, PathBuf},
};

use serde::{Deserialize, Serialize};
use sha2::{Digest, Sha256};
use specta::Type;
use thiserror::Error;

const MAX_MEMBERS: usize = 512;
const MAX_DIRECTORY_ENTRIES: usize = 2_048;
const MAX_METADATA_BYTES: u64 = 16 * 1024 * 1024;
const MAX_RECORDS: usize = 100_000;
const MAX_ISSUES: usize = 10_000;
const MAX_ASSET_BYTES: u64 = 64 * 1024 * 1024;
const MAX_TOTAL_ASSET_BYTES: u64 = 8 * 1024 * 1024 * 1024;
const MAX_LABEL_CHARS: usize = 160;

#[derive(Debug, Error)]
pub enum LegacyScanError {
    #[error("legacy storage root is not a directory")]
    InvalidRoot,
    #[error("failed to inspect legacy storage: {0}")]
    Io(#[from] io::Error),
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Type)]
#[serde(rename_all = "camelCase")]
pub struct LegacyIssue {
    pub code: String,
    pub member: String,
    pub record_id: Option<String>,
    pub detail: String,
}

#[derive(Clone, Debug, Default, PartialEq, Eq, Serialize, Type)]
#[serde(rename_all = "camelCase")]
pub struct LegacyScanReport {
    pub members: i32,
    pub metadata_records: i32,
    pub existing_assets: i32,
    pub training_records: i32,
    pub frozen_records: i32,
    pub duplicate_assets: i32,
    pub truncated: bool,
    pub issues: Vec<LegacyIssue>,
}

#[derive(Debug, Deserialize)]
struct LegacyStore {
    #[allow(dead_code)]
    version: Option<String>,
    #[serde(default)]
    files: BTreeMap<String, LegacyFile>,
}

#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
struct LegacyFile {
    id: Option<String>,
    relative_path: Option<String>,
    hash: Option<String>,
    pair_id: Option<String>,
    #[serde(default)]
    training_records: Vec<serde_json::Value>,
    #[serde(default)]
    is_frozen: bool,
}

pub fn scan_legacy_storage(root: &Path) -> Result<LegacyScanReport, LegacyScanError> {
    if !root.is_dir() {
        return Err(LegacyScanError::InvalidRoot);
    }
    let canonical_root = root.canonicalize()?;
    if !canonical_root.is_dir() {
        return Err(LegacyScanError::InvalidRoot);
    }

    let mut report = LegacyScanReport::default();
    let sources = discover_member_sources(&canonical_root, &mut report)?;
    report.members = i32::try_from(sources.len()).unwrap_or(i32::MAX);
    let mut observed_hashes: HashMap<String, (String, String)> = HashMap::new();
    let mut scanned_asset_bytes = 0_u64;

    for source in sources {
        scan_member(
            &source,
            &canonical_root,
            &mut report,
            &mut observed_hashes,
            &mut scanned_asset_bytes,
        );
    }

    Ok(report)
}

struct MemberSource {
    name: String,
    member_root: PathBuf,
    metadata_path: PathBuf,
    files_root: PathBuf,
}

fn discover_member_sources(
    root: &Path,
    report: &mut LegacyScanReport,
) -> Result<Vec<MemberSource>, io::Error> {
    let mut sources = Vec::new();
    let root_metadata = root.join(".metadata.json");
    if root_metadata.is_file() {
        match root_metadata.canonicalize() {
            Ok(metadata_path) if metadata_path.starts_with(root) => sources.push(MemberSource {
                name: "default".to_owned(),
                member_root: root.to_path_buf(),
                metadata_path,
                files_root: root.to_path_buf(),
            }),
            _ => push_issue(
                report,
                LegacyIssue {
                    code: "unsafe_metadata_path".to_owned(),
                    member: "default".to_owned(),
                    record_id: None,
                    detail: "metadata resolves outside the selected directory".to_owned(),
                },
            ),
        }
    }

    let members_root = root.join("members");
    if !members_root.is_dir() {
        return Ok(sources);
    }
    let canonical_members_root = members_root.canonicalize()?;
    if !canonical_members_root.starts_with(root) {
        push_issue(
            report,
            LegacyIssue {
                code: "unsafe_member_path".to_owned(),
                member: "members".to_owned(),
                record_id: None,
                detail: "members directory resolves outside the selected directory".to_owned(),
            },
        );
        return Ok(sources);
    }

    let mut entries = fs::read_dir(&canonical_members_root)?
        .take(MAX_DIRECTORY_ENTRIES.saturating_add(1))
        .collect::<Result<Vec<_>, _>>()?;
    if entries.len() > MAX_DIRECTORY_ENTRIES {
        entries.truncate(MAX_DIRECTORY_ENTRIES);
        mark_truncated(report, "目录条目数量超过安全扫描上限，报告已截断");
    }
    entries.sort_by_key(fs::DirEntry::file_name);
    for entry in entries {
        if !entry.path().is_dir() {
            continue;
        }
        if sources.len() >= MAX_MEMBERS {
            mark_truncated(report, "学习档案数量超过安全扫描上限，报告已截断");
            break;
        }
        let name = safe_label(&entry.file_name().to_string_lossy());
        let member_root = match entry.path().canonicalize() {
            Ok(path) if path.starts_with(root) && path.starts_with(&canonical_members_root) => path,
            _ => {
                push_issue(
                    report,
                    LegacyIssue {
                        code: "unsafe_member_path".to_owned(),
                        member: name,
                        record_id: None,
                        detail: "learning profile resolves outside the selected directory"
                            .to_owned(),
                    },
                );
                continue;
            }
        };
        sources.push(MemberSource {
            name,
            metadata_path: member_root.join(".metadata.json"),
            files_root: member_root.join("files"),
            member_root,
        });
    }

    Ok(sources)
}

fn scan_member(
    source: &MemberSource,
    selected_root: &Path,
    report: &mut LegacyScanReport,
    observed_hashes: &mut HashMap<String, (String, String)>,
    scanned_asset_bytes: &mut u64,
) {
    if usize::try_from(report.metadata_records).unwrap_or(MAX_RECORDS) >= MAX_RECORDS {
        mark_truncated(report, "元数据记录数量超过安全扫描上限，报告已截断");
        return;
    }

    let metadata_path = match source.metadata_path.canonicalize() {
        Ok(path) if path.starts_with(selected_root) && path.starts_with(&source.member_root) => {
            path
        }
        Ok(_) => {
            push_source_issue(
                report,
                "unsafe_metadata_path",
                source,
                None,
                "metadata resolves outside the selected directory",
            );
            return;
        }
        Err(_) => {
            push_source_issue(
                report,
                "missing_metadata",
                source,
                None,
                "member metadata cannot be read",
            );
            return;
        }
    };
    let metadata = match read_bounded(&metadata_path, MAX_METADATA_BYTES) {
        Ok(contents) => contents,
        Err(BoundedReadError::TooLarge) => {
            push_source_issue(
                report,
                "metadata_too_large",
                source,
                None,
                "metadata exceeds the safe scan size",
            );
            return;
        }
        Err(BoundedReadError::Io) => {
            push_source_issue(
                report,
                "missing_metadata",
                source,
                None,
                "member metadata cannot be read",
            );
            return;
        }
    };
    let store: LegacyStore = match serde_json::from_slice(&metadata) {
        Ok(store) => store,
        Err(error) => {
            push_source_issue(
                report,
                "invalid_metadata",
                source,
                None,
                &format!(
                    "invalid JSON near line {} column {}",
                    error.line(),
                    error.column()
                ),
            );
            return;
        }
    };

    let processed = usize::try_from(report.metadata_records).unwrap_or(MAX_RECORDS);
    let remaining_records = MAX_RECORDS.saturating_sub(processed);
    let records_to_scan = store.files.len().min(remaining_records);
    report.metadata_records = report
        .metadata_records
        .saturating_add(i32::try_from(records_to_scan).unwrap_or(i32::MAX));
    if store.files.len() > remaining_records {
        mark_truncated(report, "元数据记录数量超过安全扫描上限，报告已截断");
    }
    let record_ids = store
        .files
        .keys()
        .take(records_to_scan)
        .cloned()
        .collect::<std::collections::HashSet<_>>();

    let canonical_files_root = if source.files_root.exists() {
        match source.files_root.canonicalize() {
            Ok(path)
                if path.starts_with(selected_root) && path.starts_with(&source.member_root) =>
            {
                Some(path)
            }
            _ => {
                push_source_issue(
                    report,
                    "unsafe_files_root",
                    source,
                    None,
                    "image directory resolves outside the selected directory",
                );
                None
            }
        }
    } else {
        None
    };
    let unsafe_files_root = source.files_root.exists() && canonical_files_root.is_none();

    for (map_id, record) in store.files.into_iter().take(records_to_scan) {
        let record_id = safe_label(record.id.as_deref().unwrap_or(&map_id));
        report.training_records = report
            .training_records
            .saturating_add(i32::try_from(record.training_records.len()).unwrap_or(i32::MAX));
        report.frozen_records = report
            .frozen_records
            .saturating_add(i32::from(record.is_frozen));

        if let Some(pair_id) = record.pair_id.as_deref()
            && !record_ids.contains(pair_id)
        {
            push_source_issue(
                report,
                "missing_pair",
                source,
                Some(record_id.clone()),
                "paired record does not exist",
            );
        }

        let Some(relative_path) = record.relative_path.as_deref() else {
            push_source_issue(
                report,
                "missing_relative_path",
                source,
                Some(record_id),
                "record has no relativePath",
            );
            continue;
        };
        let relative_path = Path::new(relative_path);
        if !is_safe_relative_path(relative_path) {
            push_source_issue(
                report,
                "unsafe_relative_path",
                source,
                Some(record_id),
                "relativePath escapes the member image directory",
            );
            continue;
        }
        if unsafe_files_root {
            continue;
        }

        let Some(files_root) = canonical_files_root.as_ref() else {
            push_source_issue(
                report,
                "missing_asset",
                source,
                Some(record_id),
                "referenced image is missing",
            );
            continue;
        };
        let asset_path = files_root.join(relative_path);
        if !asset_path.is_file() {
            push_source_issue(
                report,
                "missing_asset",
                source,
                Some(record_id),
                "referenced image is missing",
            );
            continue;
        }
        let canonical_asset_path = match asset_path.canonicalize() {
            Ok(path) => path,
            Err(_) => {
                push_source_issue(
                    report,
                    "unreadable_asset",
                    source,
                    Some(record_id),
                    "referenced image cannot be read",
                );
                continue;
            }
        };
        if !canonical_asset_path.starts_with(selected_root)
            || !canonical_asset_path.starts_with(files_root)
        {
            push_source_issue(
                report,
                "unsafe_asset_path",
                source,
                Some(record_id),
                "asset resolves outside the selected directory",
            );
            continue;
        }

        let remaining_total = MAX_TOTAL_ASSET_BYTES.saturating_sub(*scanned_asset_bytes);
        if remaining_total == 0 {
            mark_truncated(report, "图片累计大小超过安全扫描上限，报告已截断");
            continue;
        }
        let read_limit = MAX_ASSET_BYTES.min(remaining_total);
        match sha256_file(&canonical_asset_path, read_limit) {
            Ok((actual_hash, byte_length)) => {
                *scanned_asset_bytes = scanned_asset_bytes.saturating_add(byte_length);
                report.existing_assets = report.existing_assets.saturating_add(1);
                if let Some(expected_hash) = record.hash.as_deref()
                    && !expected_hash.eq_ignore_ascii_case(&actual_hash)
                {
                    push_source_issue(
                        report,
                        "hash_mismatch",
                        source,
                        Some(record_id.clone()),
                        "stored hash does not match calculated content hash",
                    );
                }
                if observed_hashes
                    .insert(actual_hash, (source.name.clone(), record_id.clone()))
                    .is_some()
                {
                    report.duplicate_assets = report.duplicate_assets.saturating_add(1);
                    push_source_issue(
                        report,
                        "duplicate_asset",
                        source,
                        Some(record_id),
                        "same content as an earlier record",
                    );
                }
            }
            Err(BoundedReadError::TooLarge) if remaining_total < MAX_ASSET_BYTES => {
                mark_truncated(report, "图片累计大小超过安全扫描上限，报告已截断");
            }
            Err(BoundedReadError::TooLarge) => push_source_issue(
                report,
                "asset_too_large",
                source,
                Some(record_id),
                "referenced image exceeds the safe scan size",
            ),
            Err(BoundedReadError::Io) => push_source_issue(
                report,
                "unreadable_asset",
                source,
                Some(record_id),
                "referenced image cannot be read",
            ),
        }
    }
}

fn is_safe_relative_path(path: &Path) -> bool {
    !path.as_os_str().is_empty()
        && path
            .components()
            .all(|component| matches!(component, Component::Normal(_)))
}

#[derive(Debug)]
enum BoundedReadError {
    Io,
    TooLarge,
}

fn read_bounded(path: &Path, max_bytes: u64) -> Result<Vec<u8>, BoundedReadError> {
    let file = fs::File::open(path).map_err(|_| BoundedReadError::Io)?;
    if file.metadata().map_err(|_| BoundedReadError::Io)?.len() > max_bytes {
        return Err(BoundedReadError::TooLarge);
    }
    let mut contents = Vec::new();
    file.take(max_bytes.saturating_add(1))
        .read_to_end(&mut contents)
        .map_err(|_| BoundedReadError::Io)?;
    if u64::try_from(contents.len()).unwrap_or(u64::MAX) > max_bytes {
        return Err(BoundedReadError::TooLarge);
    }
    Ok(contents)
}

fn sha256_file(path: &Path, max_bytes: u64) -> Result<(String, u64), BoundedReadError> {
    let mut file = fs::File::open(path).map_err(|_| BoundedReadError::Io)?;
    if file.metadata().map_err(|_| BoundedReadError::Io)?.len() > max_bytes {
        return Err(BoundedReadError::TooLarge);
    }
    let mut digest = Sha256::new();
    let mut buffer = [0_u8; 64 * 1024];
    let mut total = 0_u64;
    loop {
        let read = file.read(&mut buffer).map_err(|_| BoundedReadError::Io)?;
        if read == 0 {
            break;
        }
        total = total
            .checked_add(u64::try_from(read).unwrap_or(u64::MAX))
            .ok_or(BoundedReadError::TooLarge)?;
        if total > max_bytes {
            return Err(BoundedReadError::TooLarge);
        }
        digest.update(&buffer[..read]);
    }
    Ok((format!("{:x}", digest.finalize()), total))
}

fn safe_label(value: &str) -> String {
    if value.contains(['/', '\\', ':']) {
        return "redacted".to_owned();
    }
    let mut output = value
        .chars()
        .filter(|character| !character.is_control())
        .take(MAX_LABEL_CHARS)
        .collect::<String>();
    if output.trim().is_empty() {
        output = "unknown".to_owned();
    }
    output
}

fn push_source_issue(
    report: &mut LegacyScanReport,
    code: &str,
    source: &MemberSource,
    record_id: Option<String>,
    detail: &str,
) {
    push_issue(
        report,
        LegacyIssue {
            code: code.to_owned(),
            member: source.name.clone(),
            record_id,
            detail: detail.to_owned(),
        },
    );
}

fn push_issue(report: &mut LegacyScanReport, issue: LegacyIssue) {
    if report.issues.len() < MAX_ISSUES {
        report.issues.push(issue);
    } else {
        report.truncated = true;
    }
}

fn mark_truncated(report: &mut LegacyScanReport, detail: &str) {
    if !report.truncated {
        report.truncated = true;
        push_issue(
            report,
            LegacyIssue {
                code: "scan_limit_exceeded".to_owned(),
                member: "system".to_owned(),
                record_id: None,
                detail: detail.to_owned(),
            },
        );
    }
}
