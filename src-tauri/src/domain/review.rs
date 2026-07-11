use serde::{Deserialize, Serialize};
use uuid::Uuid;

#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum SimpleRating {
    Forgot,
    Remembered,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum FsrsRating {
    Again,
    Hard,
    Good,
    Easy,
}

impl SimpleRating {
    pub const fn into_fsrs(self) -> FsrsRating {
        match self {
            Self::Forgot => FsrsRating::Again,
            Self::Remembered => FsrsRating::Good,
        }
    }
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ReviewEvent {
    pub id: Uuid,
    pub problem_id: Uuid,
    pub device_id: Uuid,
    pub rating: FsrsRating,
    pub occurred_at_utc_ms: i64,
    pub duration_ms: u32,
    pub algorithm_version: String,
}

impl ReviewEvent {
    pub fn new(
        id: Uuid,
        problem_id: Uuid,
        device_id: Uuid,
        rating: FsrsRating,
        occurred_at_utc_ms: i64,
        duration_ms: u32,
    ) -> Self {
        Self {
            id,
            problem_id,
            device_id,
            rating,
            occurred_at_utc_ms,
            duration_ms,
            algorithm_version: "fsrs-6".to_owned(),
        }
    }
}

pub fn ordered_events(mut events: Vec<ReviewEvent>) -> Vec<ReviewEvent> {
    events.sort_by(|left, right| {
        left.occurred_at_utc_ms
            .cmp(&right.occurred_at_utc_ms)
            .then_with(|| left.id.cmp(&right.id))
    });
    events
}
