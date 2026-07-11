use std::{
    collections::{BTreeMap, HashMap},
    fs,
    io::{self, Read},
    path::{Component, Path, PathBuf},
};

use serde::{Deserialize, Serialize};
use sha2::{Digest, Sha256};
use thiserror::Error;

#[derive(Debug, Error)]
pub enum LegacyScanError {
    #[error("legacy storage root is not a directory")]
    InvalidRoot,
    #[error("failed to inspect legacy storage: {0}")]
    Io(#[from] io::Error),
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct LegacyIssue {
    pub code: String,
    pub member: String,
    pub record_id: Option<String>,
    pub detail: String,
}

#[derive(Clone, Debug, Default, PartialEq, Eq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct LegacyScanReport {
    pub members: usize,
    pub metadata_records: usize,
    pub existing_assets: usize,
    pub training_records: usize,
    pub frozen_records: usize,
    pub duplicate_assets: usize,
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
    original_file_name: Option<String>,
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

    let sources = discover_member_sources(root)?;
    let mut report = LegacyScanReport {
        members: sources.len(),
        ..LegacyScanReport::default()
    };
    let mut observed_hashes: HashMap<String, (String, String)> = HashMap::new();

    for source in sources {
        scan_member(&source, &mut report, &mut observed_hashes);
    }

    Ok(report)
}

struct MemberSource {
    name: String,
    metadata_path: PathBuf,
    files_root: PathBuf,
}

fn discover_member_sources(root: &Path) -> Result<Vec<MemberSource>, io::Error> {
    let mut sources = Vec::new();
    let root_metadata = root.join(".metadata.json");
    if root_metadata.is_file() {
        sources.push(MemberSource {
            name: "default".to_owned(),
            metadata_path: root_metadata,
            files_root: root.to_path_buf(),
        });
    }

    let members_root = root.join("members");
    if members_root.is_dir() {
        let mut entries = fs::read_dir(members_root)?.collect::<Result<Vec<_>, _>>()?;
        entries.sort_by_key(fs::DirEntry::file_name);
        for entry in entries {
            if entry.file_type()?.is_dir() {
                let member_root = entry.path();
                sources.push(MemberSource {
                    name: entry.file_name().to_string_lossy().into_owned(),
                    metadata_path: member_root.join(".metadata.json"),
                    files_root: member_root.join("files"),
                });
            }
        }
    }

    Ok(sources)
}

fn scan_member(
    source: &MemberSource,
    report: &mut LegacyScanReport,
    observed_hashes: &mut HashMap<String, (String, String)>,
) {
    let metadata = match fs::read_to_string(&source.metadata_path) {
        Ok(contents) => contents,
        Err(error) => {
            report.issues.push(issue(
                "missing_metadata",
                source,
                None,
                format!("cannot read {}: {error}", source.metadata_path.display()),
            ));
            return;
        }
    };
    let store: LegacyStore = match serde_json::from_str(&metadata) {
        Ok(store) => store,
        Err(error) => {
            report.issues.push(issue(
                "invalid_metadata",
                source,
                None,
                format!("invalid JSON: {error}"),
            ));
            return;
        }
    };

    report.metadata_records += store.files.len();
    let record_ids = store
        .files
        .keys()
        .cloned()
        .collect::<std::collections::HashSet<_>>();

    for (map_id, record) in store.files {
        let record_id = record.id.as_deref().unwrap_or(&map_id).to_owned();
        report.training_records += record.training_records.len();
        report.frozen_records += usize::from(record.is_frozen);

        if let Some(pair_id) = record.pair_id.as_deref()
            && !record_ids.contains(pair_id)
        {
            report.issues.push(issue(
                "missing_pair",
                source,
                Some(record_id.clone()),
                format!("paired record {pair_id} does not exist"),
            ));
        }

        let Some(relative_path) = record.relative_path.as_deref() else {
            report.issues.push(issue(
                "missing_relative_path",
                source,
                Some(record_id),
                "record has no relativePath".to_owned(),
            ));
            continue;
        };
        let relative_path = Path::new(relative_path);
        if !is_safe_relative_path(relative_path) {
            report.issues.push(issue(
                "unsafe_relative_path",
                source,
                Some(record_id),
                format!(
                    "relativePath escapes member files directory: {}",
                    relative_path.display()
                ),
            ));
            continue;
        }

        let asset_path = source.files_root.join(relative_path);
        if !asset_path.is_file() {
            let name = record
                .original_file_name
                .as_deref()
                .unwrap_or("unknown file");
            report.issues.push(issue(
                "missing_asset",
                source,
                Some(record_id),
                format!("{name} is missing at {}", asset_path.display()),
            ));
            continue;
        }
        report.existing_assets += 1;

        match sha256_file(&asset_path) {
            Ok(actual_hash) => {
                if let Some(expected_hash) = record.hash.as_deref()
                    && !expected_hash.eq_ignore_ascii_case(&actual_hash)
                {
                    report.issues.push(issue(
                        "hash_mismatch",
                        source,
                        Some(record_id.clone()),
                        format!("expected {expected_hash}, calculated {actual_hash}"),
                    ));
                }
                if let Some((first_member, first_id)) =
                    observed_hashes.insert(actual_hash, (source.name.clone(), record_id.clone()))
                {
                    report.duplicate_assets += 1;
                    report.issues.push(issue(
                        "duplicate_asset",
                        source,
                        Some(record_id),
                        format!("same content as {first_member}/{first_id}"),
                    ));
                }
            }
            Err(error) => report.issues.push(issue(
                "unreadable_asset",
                source,
                Some(record_id),
                error.to_string(),
            )),
        }
    }
}

fn is_safe_relative_path(path: &Path) -> bool {
    !path.as_os_str().is_empty()
        && path
            .components()
            .all(|component| matches!(component, Component::Normal(_)))
}

fn sha256_file(path: &Path) -> Result<String, io::Error> {
    let mut file = fs::File::open(path)?;
    let mut digest = Sha256::new();
    let mut buffer = [0_u8; 64 * 1024];
    loop {
        let read = file.read(&mut buffer)?;
        if read == 0 {
            break;
        }
        digest.update(&buffer[..read]);
    }
    Ok(format!("{:x}", digest.finalize()))
}

fn issue(
    code: &str,
    source: &MemberSource,
    record_id: Option<String>,
    detail: String,
) -> LegacyIssue {
    LegacyIssue {
        code: code.to_owned(),
        member: source.name.clone(),
        record_id,
        detail,
    }
}
