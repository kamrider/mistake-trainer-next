use std::collections::BTreeSet;

use rusqlite::{Connection, OptionalExtension, params};
use serde::Serialize;
use specta::Type;
use uuid::Uuid;

use super::problems::ProblemUseCaseError;

#[derive(Clone, Debug)]
pub struct ProblemBulkMetadata {
    pub account_id: String,
    pub profile_id: String,
    pub problem_ids: Vec<String>,
    pub subject: Option<String>,
    pub add_tags: Vec<String>,
    pub remove_tags: Vec<String>,
    pub now_utc_ms: i64,
}

#[derive(Clone, Debug, Serialize, Type)]
#[serde(rename_all = "camelCase")]
pub struct ProblemBulkMetadataReport {
    pub updated_count: i32,
}

struct ProblemMetadataRow {
    id: String,
    subject: String,
    note: String,
    tags: Vec<String>,
    time_limit_seconds: Option<i32>,
    revision: i64,
}

pub fn update_problem_bulk_metadata(
    connection: &mut Connection,
    input: ProblemBulkMetadata,
) -> Result<ProblemBulkMetadataReport, ProblemUseCaseError> {
    let problem_ids = validate_problem_ids(input.problem_ids)?;
    let subject = input
        .subject
        .map(|subject| subject.trim().to_owned())
        .map(|subject| {
            if subject.chars().count() > 40 {
                Err(ProblemUseCaseError::InvalidText)
            } else {
                Ok(subject)
            }
        })
        .transpose()?;
    let add_tags = validate_tags(input.add_tags)?;
    let remove_tags = validate_tags(input.remove_tags)?;
    if subject.is_none() && add_tags.is_empty() && remove_tags.is_empty() {
        return Err(ProblemUseCaseError::EmptyChange);
    }

    let transaction = connection.transaction()?;
    let mut rows = Vec::with_capacity(problem_ids.len());
    for problem_id in &problem_ids {
        if crate::modules::sync_conflicts::has_open_conflict(
            &transaction,
            &input.account_id,
            "problem",
            problem_id,
        )? {
            return Err(ProblemUseCaseError::ConflictPending);
        }
        let stored = transaction
            .query_row(
                "SELECT id, subject, note, tags_json, time_limit_seconds, revision
                 FROM problems
                 WHERE id = ?1 AND account_id = ?2 AND profile_id = ?3 AND status = 'active'",
                params![problem_id, input.account_id, input.profile_id],
                |row| {
                    Ok((
                        row.get::<_, String>(0)?,
                        row.get::<_, String>(1)?,
                        row.get::<_, String>(2)?,
                        row.get::<_, String>(3)?,
                        row.get::<_, Option<i32>>(4)?,
                        row.get::<_, i64>(5)?,
                    ))
                },
            )
            .optional()?
            .ok_or(ProblemUseCaseError::ProblemNotFound)?;
        rows.push(ProblemMetadataRow {
            id: stored.0,
            subject: stored.1,
            note: stored.2,
            tags: serde_json::from_str(&stored.3)?,
            time_limit_seconds: stored.4,
            revision: stored.5,
        });
    }

    let remove_tags = remove_tags.into_iter().collect::<BTreeSet<_>>();
    let mut updated_count = 0_i32;
    for row in rows {
        let next_subject = subject.as_deref().unwrap_or(&row.subject).to_owned();
        let mut seen = BTreeSet::new();
        let mut next_tags = row
            .tags
            .iter()
            .map(|tag| tag.trim().to_owned())
            .filter(|tag| !tag.is_empty() && !remove_tags.contains(tag))
            .filter(|tag| seen.insert(tag.clone()))
            .collect::<Vec<_>>();
        for tag in &add_tags {
            if seen.insert(tag.clone()) {
                next_tags.push(tag.clone());
            }
        }
        if next_tags.len() > 20 {
            return Err(ProblemUseCaseError::InvalidTags);
        }
        if next_subject == row.subject && next_tags == row.tags {
            continue;
        }

        let next_revision = row.revision + 1;
        let tags_json = serde_json::to_string(&next_tags)?;
        let changed = transaction.execute(
            "UPDATE problems
             SET subject = ?1, tags_json = ?2, updated_at_utc_ms = ?3, revision = ?4
             WHERE id = ?5 AND account_id = ?6 AND profile_id = ?7 AND status = 'active' AND revision = ?8",
            params![
                next_subject,
                tags_json,
                input.now_utc_ms,
                next_revision,
                row.id,
                input.account_id,
                input.profile_id,
                row.revision,
            ],
        )?;
        if changed != 1 {
            return Err(ProblemUseCaseError::ProblemNotFound);
        }
        let payload = serde_json::to_string(&serde_json::json!({
            "id": row.id,
            "subject": next_subject,
            "note": row.note,
            "tags": next_tags,
            "timeLimitSeconds": row.time_limit_seconds,
            "baseRevision": row.revision,
            "revision": next_revision,
            "updatedAtUtcMs": input.now_utc_ms,
        }))?;
        transaction.execute(
            "INSERT INTO sync_operations(id, account_id, profile_id, entity_type, entity_id, operation, payload_json, status, attempt_count, created_at_utc_ms, next_attempt_at_utc_ms)
             VALUES(?1, ?2, ?3, 'problem', ?4, 'upsert', ?5, 'pending', 0, ?6, ?6)",
            params![
                Uuid::now_v7().to_string(),
                input.account_id,
                input.profile_id,
                row.id,
                payload,
                input.now_utc_ms,
            ],
        )?;
        updated_count += 1;
    }
    transaction.commit()?;
    Ok(ProblemBulkMetadataReport { updated_count })
}

fn validate_problem_ids(problem_ids: Vec<String>) -> Result<Vec<String>, ProblemUseCaseError> {
    if problem_ids.is_empty() || problem_ids.len() > 100 {
        return Err(ProblemUseCaseError::InvalidSelection);
    }
    let mut seen = BTreeSet::new();
    if problem_ids.iter().any(|id| id.trim().is_empty())
        || problem_ids.iter().any(|id| !seen.insert(id.clone()))
    {
        return Err(ProblemUseCaseError::InvalidSelection);
    }
    Ok(problem_ids)
}

fn validate_tags(tags: Vec<String>) -> Result<Vec<String>, ProblemUseCaseError> {
    if tags.len() > 20 {
        return Err(ProblemUseCaseError::InvalidTags);
    }
    let mut seen = BTreeSet::new();
    tags.into_iter()
        .map(|tag| tag.trim().to_owned())
        .map(|tag| {
            if tag.is_empty() || tag.chars().count() > 30 {
                Err(ProblemUseCaseError::InvalidTags)
            } else {
                Ok(tag)
            }
        })
        .collect::<Result<Vec<_>, _>>()
        .map(|tags| {
            tags.into_iter()
                .filter(|tag| seen.insert(tag.clone()))
                .collect()
        })
}
