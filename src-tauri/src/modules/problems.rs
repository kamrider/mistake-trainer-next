use std::{
    collections::HashMap,
    path::{Path, PathBuf},
};

use rusqlite::{Connection, OptionalExtension, params};
use serde::Serialize;
use thiserror::Error;
use uuid::Uuid;

use crate::infrastructure::assets::{AssetCryptoError, encrypt_asset, plaintext_sha256};

#[derive(Clone, Copy, Debug, Serialize)]
#[serde(rename_all = "snake_case")]
pub enum AssetRole {
    Question,
    Answer,
}

impl AssetRole {
    const fn as_str(self) -> &'static str {
        match self {
            Self::Question => "question",
            Self::Answer => "answer",
        }
    }
}

#[derive(Clone, Debug)]
pub struct CaptureAsset {
    pub role: AssetRole,
    pub media_type: String,
    pub bytes: Vec<u8>,
}

#[derive(Clone, Debug)]
pub struct CreateProblem {
    pub account_id: String,
    pub profile_id: String,
    pub subject: String,
    pub note: String,
    pub assets: Vec<CaptureAsset>,
    pub now_utc_ms: i64,
}

#[derive(Clone, Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct Problem {
    pub id: String,
    pub account_id: String,
    pub profile_id: String,
    pub subject: String,
    pub note: String,
    pub asset_ids: Vec<String>,
    pub created_at_utc_ms: i64,
    pub updated_at_utc_ms: i64,
    pub revision: i64,
}

#[derive(Debug, Error)]
pub enum ProblemUseCaseError {
    #[error("learner profile was not found for this account")]
    ProfileNotFound,
    #[error("a problem requires at least one non-empty asset")]
    MissingAsset,
    #[error("problem persistence failed")]
    Database(#[from] rusqlite::Error),
    #[error("asset encryption failed")]
    Crypto(#[from] AssetCryptoError),
    #[error("asset file operation failed")]
    File(#[from] std::io::Error),
    #[error("problem outbox serialization failed")]
    Serialization(#[from] serde_json::Error),
}

#[derive(Serialize)]
#[serde(rename_all = "camelCase")]
struct AssetMetadata {
    id: String,
    account_id: String,
    plaintext_sha256: String,
    encrypted_path: String,
    byte_length: i64,
    media_type: String,
    created_at_utc_ms: i64,
}

struct NewAsset {
    metadata: AssetMetadata,
    staged_path: PathBuf,
    final_path: PathBuf,
    moved_to_final: bool,
}

struct AssetLink {
    asset_id: String,
    role: AssetRole,
    position: i64,
}

pub fn create_problem(
    connection: &mut Connection,
    blob_root: &Path,
    key: &[u8; 32],
    input: CreateProblem,
) -> Result<Problem, ProblemUseCaseError> {
    let profile_exists: bool = connection.query_row(
        "SELECT EXISTS(SELECT 1 FROM learner_profiles WHERE id = ?1 AND account_id = ?2)",
        params![input.profile_id, input.account_id],
        |row| row.get(0),
    )?;
    if !profile_exists {
        return Err(ProblemUseCaseError::ProfileNotFound);
    }
    if input.assets.is_empty() || input.assets.iter().any(|asset| asset.bytes.is_empty()) {
        return Err(ProblemUseCaseError::MissingAsset);
    }

    let mut known_assets = HashMap::<String, String>::new();
    let mut new_assets = Vec::<NewAsset>::new();
    let mut links = Vec::<AssetLink>::new();
    let mut question_position = 0_i64;
    let mut answer_position = 0_i64;
    let staging_root = blob_root.join(".staging");

    for capture in input.assets {
        let hash = plaintext_sha256(&capture.bytes);
        let asset_id = if let Some(id) = known_assets.get(&hash) {
            id.clone()
        } else if let Some(id) = connection
            .query_row(
                "SELECT id FROM assets WHERE account_id = ?1 AND plaintext_sha256 = ?2",
                params![input.account_id, hash],
                |row| row.get::<_, String>(0),
            )
            .optional()?
        {
            known_assets.insert(hash.clone(), id.clone());
            id
        } else {
            let id = Uuid::now_v7().to_string();
            let shard = &id[..2];
            let relative = PathBuf::from("blobs").join(shard).join(format!("{id}.mtb"));
            let staged_path = staging_root.join(format!("{id}.tmp"));
            let final_path = blob_root.join(&relative);
            std::fs::create_dir_all(&staging_root)?;
            std::fs::write(&staged_path, encrypt_asset(&capture.bytes, key)?)?;
            new_assets.push(NewAsset {
                metadata: AssetMetadata {
                    id: id.clone(),
                    account_id: input.account_id.clone(),
                    plaintext_sha256: hash.clone(),
                    encrypted_path: relative.to_string_lossy().replace('\\', "/"),
                    byte_length: i64::try_from(capture.bytes.len()).unwrap_or(i64::MAX),
                    media_type: capture.media_type,
                    created_at_utc_ms: input.now_utc_ms,
                },
                staged_path,
                final_path,
                moved_to_final: false,
            });
            known_assets.insert(hash, id.clone());
            id
        };

        let position = match capture.role {
            AssetRole::Question => {
                let current = question_position;
                question_position += 1;
                current
            }
            AssetRole::Answer => {
                let current = answer_position;
                answer_position += 1;
                current
            }
        };
        links.push(AssetLink {
            asset_id,
            role: capture.role,
            position,
        });
    }

    let problem = Problem {
        id: Uuid::now_v7().to_string(),
        account_id: input.account_id,
        profile_id: input.profile_id,
        subject: input.subject.trim().to_owned(),
        note: input.note.trim().to_owned(),
        asset_ids: links.iter().map(|link| link.asset_id.clone()).collect(),
        created_at_utc_ms: input.now_utc_ms,
        updated_at_utc_ms: input.now_utc_ms,
        revision: 1,
    };

    let result = persist_problem(
        connection,
        &problem,
        &links,
        &mut new_assets,
        input.now_utc_ms,
    );
    if result.is_err() {
        cleanup_new_assets(&new_assets);
    }
    let _ = std::fs::remove_dir(&staging_root);
    result.map(|_| problem)
}

fn persist_problem(
    connection: &mut Connection,
    problem: &Problem,
    links: &[AssetLink],
    new_assets: &mut [NewAsset],
    now_utc_ms: i64,
) -> Result<(), ProblemUseCaseError> {
    let problem_payload = serde_json::to_string(problem)?;
    let transaction = connection.transaction()?;
    transaction.execute(
        "INSERT INTO problems(id, account_id, profile_id, subject, note, created_at_utc_ms, updated_at_utc_ms, revision) VALUES(?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8)",
        params![problem.id, problem.account_id, problem.profile_id, problem.subject, problem.note, problem.created_at_utc_ms, problem.updated_at_utc_ms, problem.revision],
    )?;

    for asset in new_assets.iter() {
        let metadata = &asset.metadata;
        transaction.execute(
            "INSERT INTO assets(id, account_id, plaintext_sha256, encrypted_path, byte_length, media_type, created_at_utc_ms) VALUES(?1, ?2, ?3, ?4, ?5, ?6, ?7)",
            params![metadata.id, metadata.account_id, metadata.plaintext_sha256, metadata.encrypted_path, metadata.byte_length, metadata.media_type, metadata.created_at_utc_ms],
        )?;
        transaction.execute(
            "INSERT INTO sync_operations(id, account_id, profile_id, entity_type, entity_id, operation, payload_json, status, attempt_count, created_at_utc_ms, next_attempt_at_utc_ms) VALUES(?1, ?2, ?3, 'asset', ?4, 'upsert', ?5, 'pending', 0, ?6, ?6)",
            params![Uuid::now_v7().to_string(), problem.account_id, problem.profile_id, metadata.id, serde_json::to_string(metadata)?, now_utc_ms],
        )?;
    }

    for link in links {
        transaction.execute(
            "INSERT INTO problem_assets(problem_id, asset_id, role, position) VALUES(?1, ?2, ?3, ?4)",
            params![problem.id, link.asset_id, link.role.as_str(), link.position],
        )?;
    }
    transaction.execute(
        "INSERT INTO sync_operations(id, account_id, profile_id, entity_type, entity_id, operation, payload_json, status, attempt_count, created_at_utc_ms, next_attempt_at_utc_ms) VALUES(?1, ?2, ?3, 'problem', ?4, 'upsert', ?5, 'pending', 0, ?6, ?6)",
        params![Uuid::now_v7().to_string(), problem.account_id, problem.profile_id, problem.id, problem_payload, now_utc_ms],
    )?;

    for asset in new_assets.iter_mut() {
        if let Some(parent) = asset.final_path.parent() {
            std::fs::create_dir_all(parent)?;
        }
        std::fs::rename(&asset.staged_path, &asset.final_path)?;
        asset.moved_to_final = true;
    }

    transaction.commit()?;
    Ok(())
}

fn cleanup_new_assets(assets: &[NewAsset]) {
    for asset in assets {
        let path = if asset.moved_to_final {
            &asset.final_path
        } else {
            &asset.staged_path
        };
        let _ = std::fs::remove_file(path);
    }
}
