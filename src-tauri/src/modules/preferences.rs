use std::collections::HashSet;

use rusqlite::{Connection, OptionalExtension as _, params};
use serde::{Deserialize, Serialize};
use specta::Type;
use thiserror::Error;

pub const DEFAULT_SUBJECTS: [&str; 9] = [
    "语文", "数学", "英语", "政治", "历史", "地理", "物理", "化学", "生物",
];
const MAX_CUSTOM_SUBJECTS: usize = 20;
const MAX_SUBJECT_CHARS: usize = 40;

#[derive(Clone, Debug, Deserialize, Eq, PartialEq, Serialize, Type)]
#[serde(rename_all = "camelCase")]
pub struct SubjectPreferences {
    pub enabled_subjects: Vec<String>,
    pub custom_subjects: Vec<String>,
    pub capture_sound_enabled: bool,
}

#[derive(Clone, Debug)]
pub struct SaveSubjectPreferences {
    pub enabled_subjects: Vec<String>,
    pub custom_subjects: Vec<String>,
    pub capture_sound_enabled: bool,
}

#[derive(Clone, Copy, Debug, Deserialize, Eq, PartialEq, Serialize, Type)]
#[serde(rename_all = "snake_case")]
pub enum ReviewFocusPolicy {
    Off,
    SessionStart,
    #[serde(rename = "every_10")]
    EveryTen,
}

impl ReviewFocusPolicy {
    pub const fn as_str(self) -> &'static str {
        match self {
            Self::Off => "off",
            Self::SessionStart => "session_start",
            Self::EveryTen => "every_10",
        }
    }

    fn parse(value: &str) -> Result<Self, PreferencesError> {
        match value {
            "off" => Ok(Self::Off),
            "session_start" => Ok(Self::SessionStart),
            "every_10" => Ok(Self::EveryTen),
            _ => Err(PreferencesError::InvalidInput),
        }
    }
}

#[derive(Clone, Debug, Eq, PartialEq, Serialize, Type)]
#[serde(rename_all = "camelCase")]
pub struct ReviewPreferences {
    pub focus_policy: ReviewFocusPolicy,
}

#[derive(Clone, Debug)]
pub struct SaveReviewPreferences {
    pub focus_policy: ReviewFocusPolicy,
}

#[derive(Debug, Error)]
pub enum PreferencesError {
    #[error("preference input is invalid")]
    InvalidInput,
    #[error("profile was not found")]
    ProfileNotFound,
    #[error("preference database operation failed")]
    Database(#[from] rusqlite::Error),
    #[error("preference serialization failed")]
    Serialization(#[from] serde_json::Error),
}

pub fn load_subject_preferences(
    connection: &Connection,
    account_id: &str,
    profile_id: &str,
) -> Result<SubjectPreferences, PreferencesError> {
    ensure_profile(connection, account_id, profile_id)?;
    let stored = connection
        .query_row(
            "SELECT enabled_subjects_json, custom_subjects_json, capture_sound_enabled
             FROM profile_preferences WHERE account_id = ?1 AND profile_id = ?2",
            params![account_id, profile_id],
            |row| {
                Ok((
                    row.get::<_, String>(0)?,
                    row.get::<_, String>(1)?,
                    row.get::<_, i64>(2)? != 0,
                ))
            },
        )
        .optional()?;
    let Some((enabled_json, custom_json, capture_sound_enabled)) = stored else {
        return Ok(default_subject_preferences());
    };
    let enabled_subjects = serde_json::from_str(&enabled_json)?;
    let custom_subjects = serde_json::from_str(&custom_json)?;
    normalize_preferences(SaveSubjectPreferences {
        enabled_subjects,
        custom_subjects,
        capture_sound_enabled,
    })
}

pub fn save_subject_preferences(
    connection: &Connection,
    account_id: &str,
    profile_id: &str,
    input: SaveSubjectPreferences,
    now_utc_ms: i64,
) -> Result<SubjectPreferences, PreferencesError> {
    ensure_profile(connection, account_id, profile_id)?;
    let normalized = normalize_preferences(input)?;
    connection.execute(
        "INSERT INTO profile_preferences(
             account_id, profile_id, enabled_subjects_json, custom_subjects_json,
             capture_sound_enabled, updated_at_utc_ms
         ) VALUES(?1, ?2, ?3, ?4, ?5, ?6)
         ON CONFLICT(account_id, profile_id) DO UPDATE SET
             enabled_subjects_json = excluded.enabled_subjects_json,
             custom_subjects_json = excluded.custom_subjects_json,
             capture_sound_enabled = excluded.capture_sound_enabled,
             updated_at_utc_ms = excluded.updated_at_utc_ms",
        params![
            account_id,
            profile_id,
            serde_json::to_string(&normalized.enabled_subjects)?,
            serde_json::to_string(&normalized.custom_subjects)?,
            i64::from(normalized.capture_sound_enabled),
            now_utc_ms,
        ],
    )?;
    Ok(normalized)
}

pub fn load_review_preferences(
    connection: &Connection,
    account_id: &str,
    profile_id: &str,
) -> Result<ReviewPreferences, PreferencesError> {
    ensure_profile(connection, account_id, profile_id)?;
    let stored = connection
        .query_row(
            "SELECT review_focus_policy
             FROM profile_preferences WHERE account_id = ?1 AND profile_id = ?2",
            params![account_id, profile_id],
            |row| row.get::<_, String>(0),
        )
        .optional()?;
    Ok(ReviewPreferences {
        focus_policy: match stored {
            Some(value) => ReviewFocusPolicy::parse(&value)?,
            None => ReviewFocusPolicy::Off,
        },
    })
}

pub fn save_review_preferences(
    connection: &Connection,
    account_id: &str,
    profile_id: &str,
    input: SaveReviewPreferences,
    now_utc_ms: i64,
) -> Result<ReviewPreferences, PreferencesError> {
    ensure_profile(connection, account_id, profile_id)?;
    connection.execute(
        "INSERT INTO profile_preferences(
             account_id, profile_id, enabled_subjects_json, custom_subjects_json,
             capture_sound_enabled, updated_at_utc_ms, review_focus_policy
         ) VALUES(?1, ?2, ?3, '[]', 1, ?4, ?5)
         ON CONFLICT(account_id, profile_id) DO UPDATE SET
             review_focus_policy = excluded.review_focus_policy,
             updated_at_utc_ms = excluded.updated_at_utc_ms",
        params![
            account_id,
            profile_id,
            serde_json::to_string(&DEFAULT_SUBJECTS)?,
            now_utc_ms,
            input.focus_policy.as_str(),
        ],
    )?;
    Ok(ReviewPreferences {
        focus_policy: input.focus_policy,
    })
}

fn default_subject_preferences() -> SubjectPreferences {
    SubjectPreferences {
        enabled_subjects: DEFAULT_SUBJECTS
            .iter()
            .map(|subject| (*subject).to_owned())
            .collect(),
        custom_subjects: Vec::new(),
        capture_sound_enabled: true,
    }
}

fn normalize_preferences(
    input: SaveSubjectPreferences,
) -> Result<SubjectPreferences, PreferencesError> {
    let custom_subjects = normalized_unique(input.custom_subjects)?;
    if custom_subjects.len() > MAX_CUSTOM_SUBJECTS
        || custom_subjects.iter().any(|subject| is_builtin(subject))
    {
        return Err(PreferencesError::InvalidInput);
    }
    let allowed_custom = custom_subjects
        .iter()
        .map(String::as_str)
        .collect::<HashSet<_>>();
    let enabled_subjects = normalized_unique(input.enabled_subjects)?;
    if enabled_subjects.is_empty()
        || enabled_subjects
            .iter()
            .any(|subject| !is_builtin(subject) && !allowed_custom.contains(subject.as_str()))
    {
        return Err(PreferencesError::InvalidInput);
    }
    Ok(SubjectPreferences {
        enabled_subjects,
        custom_subjects,
        capture_sound_enabled: input.capture_sound_enabled,
    })
}

fn normalized_unique(values: Vec<String>) -> Result<Vec<String>, PreferencesError> {
    let mut seen = HashSet::new();
    let mut normalized = Vec::new();
    for value in values {
        let value = value.trim().to_owned();
        if value.is_empty() || value.chars().count() > MAX_SUBJECT_CHARS {
            return Err(PreferencesError::InvalidInput);
        }
        if seen.insert(value.clone()) {
            normalized.push(value);
        }
    }
    Ok(normalized)
}

fn is_builtin(subject: &str) -> bool {
    DEFAULT_SUBJECTS.contains(&subject)
}

fn ensure_profile(
    connection: &Connection,
    account_id: &str,
    profile_id: &str,
) -> Result<(), PreferencesError> {
    let exists: i64 = connection.query_row(
        "SELECT EXISTS(SELECT 1 FROM learner_profiles WHERE id = ?1 AND account_id = ?2)",
        params![profile_id, account_id],
        |row| row.get(0),
    )?;
    if exists == 0 {
        return Err(PreferencesError::ProfileNotFound);
    }
    Ok(())
}
