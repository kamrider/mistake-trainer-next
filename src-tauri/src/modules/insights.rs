use rusqlite::{Connection, params};
use serde::Serialize;
use specta::Type;
use thiserror::Error;

const DAY_MS: i64 = 86_400_000;
const REPORT_DAYS: i64 = 14;

#[derive(Clone, Debug, Serialize, Type)]
#[serde(rename_all = "camelCase")]
pub struct DailyActivity {
    pub day_start_utc_ms: f64,
    pub review_count: i32,
    pub duration_ms: f64,
}

#[derive(Clone, Debug, Serialize, Type)]
#[serde(rename_all = "camelCase")]
pub struct SubjectActivity {
    pub subject: String,
    pub problem_count: i32,
    pub review_count: i32,
}

#[derive(Clone, Debug, Serialize, Type)]
#[serde(rename_all = "camelCase")]
pub struct ReportSummary {
    pub active_problem_count: i32,
    pub due_problem_count: i32,
    pub review_count: i32,
    pub remembered_rate: f64,
    pub total_duration_ms: f64,
    pub current_streak_days: i32,
    pub daily_activity: Vec<DailyActivity>,
    pub subject_activity: Vec<SubjectActivity>,
}

#[derive(Clone, Debug, Serialize, Type)]
#[serde(rename_all = "camelCase")]
pub struct SettingsOverview {
    pub active_problem_count: i32,
    pub archived_problem_count: i32,
    pub trashed_problem_count: i32,
    pub pending_operation_count: i32,
    pub failed_operation_count: i32,
    pub unresolved_conflict_count: i32,
    pub local_encryption_ready: bool,
    pub cloud_sync_configured: bool,
}

#[derive(Debug, Error)]
pub enum InsightsError {
    #[error("report query failed")]
    Database(#[from] rusqlite::Error),
}

pub fn report_summary(
    connection: &Connection,
    account_id: &str,
    profile_id: &str,
    now_utc_ms: i64,
) -> Result<ReportSummary, InsightsError> {
    let active_problem_count = scalar(
        connection,
        "SELECT COUNT(*) FROM problems WHERE account_id = ?1 AND profile_id = ?2 AND status = 'active'",
        account_id,
        profile_id,
    )?;
    let due_problem_count = connection.query_row(
        "SELECT COUNT(*) FROM problems p
         LEFT JOIN schedule_states s ON s.problem_id = p.id
         WHERE p.account_id = ?1 AND p.profile_id = ?2 AND p.status = 'active'
           AND (s.due_at_utc_ms IS NULL OR s.due_at_utc_ms <= ?3)",
        params![account_id, profile_id, now_utc_ms],
        |row| row.get(0),
    )?;
    let (review_count, remembered_count, total_duration_ms): (i64, i64, i64) = connection
        .query_row(
            "SELECT COUNT(*),
                    COALESCE(SUM(CASE WHEN rating != 'again' THEN 1 ELSE 0 END), 0),
                    COALESCE(SUM(duration_ms), 0)
             FROM review_events WHERE account_id = ?1 AND profile_id = ?2",
            params![account_id, profile_id],
            |row| Ok((row.get(0)?, row.get(1)?, row.get(2)?)),
        )?;
    let remembered_rate = if review_count == 0 {
        0.0
    } else {
        remembered_count as f64 / review_count as f64
    };
    let today = now_utc_ms.div_euclid(DAY_MS) * DAY_MS;
    let start = today - (REPORT_DAYS - 1) * DAY_MS;
    let mut daily_activity = (0..REPORT_DAYS)
        .map(|offset| DailyActivity {
            day_start_utc_ms: (start + offset * DAY_MS) as f64,
            review_count: 0,
            duration_ms: 0.0,
        })
        .collect::<Vec<_>>();
    {
        let mut statement = connection.prepare(
            "SELECT (occurred_at_utc_ms / ?1) * ?1 AS day_start, COUNT(*), COALESCE(SUM(duration_ms), 0)
             FROM review_events
             WHERE account_id = ?2 AND profile_id = ?3 AND occurred_at_utc_ms >= ?4 AND occurred_at_utc_ms < ?5
             GROUP BY day_start ORDER BY day_start",
        )?;
        let rows = statement.query_map(
            params![DAY_MS, account_id, profile_id, start, today + DAY_MS],
            |row| {
                Ok((
                    row.get::<_, i64>(0)?,
                    row.get::<_, i64>(1)?,
                    row.get::<_, i64>(2)?,
                ))
            },
        )?;
        for row in rows {
            let (day, count, duration) = row?;
            if let Ok(index) = usize::try_from((day - start).div_euclid(DAY_MS)) {
                if let Some(item) = daily_activity.get_mut(index) {
                    item.review_count = bounded_i32(count);
                    item.duration_ms = duration as f64;
                }
            }
        }
    }
    let current_streak_days = current_streak(&daily_activity);
    let subject_activity = {
        let mut statement = connection.prepare(
            "SELECT CASE WHEN trim(p.subject) = '' THEN '未分类' ELSE p.subject END,
                    COUNT(DISTINCT p.id), COUNT(e.id)
             FROM problems p
             LEFT JOIN review_events e ON e.problem_id = p.id
               AND e.account_id = p.account_id AND e.profile_id = p.profile_id
             WHERE p.account_id = ?1 AND p.profile_id = ?2 AND p.status = 'active'
             GROUP BY CASE WHEN trim(p.subject) = '' THEN '未分类' ELSE p.subject END
             ORDER BY COUNT(e.id) DESC, COUNT(DISTINCT p.id) DESC, p.subject
             LIMIT 8",
        )?;
        statement
            .query_map(params![account_id, profile_id], |row| {
                Ok(SubjectActivity {
                    subject: row.get(0)?,
                    problem_count: bounded_i32(row.get(1)?),
                    review_count: bounded_i32(row.get(2)?),
                })
            })?
            .collect::<Result<Vec<_>, _>>()?
    };

    Ok(ReportSummary {
        active_problem_count: bounded_i32(active_problem_count),
        due_problem_count: bounded_i32(due_problem_count),
        review_count: bounded_i32(review_count),
        remembered_rate,
        total_duration_ms: total_duration_ms as f64,
        current_streak_days,
        daily_activity,
        subject_activity,
    })
}

pub fn settings_overview(
    connection: &Connection,
    account_id: &str,
    profile_id: &str,
) -> Result<SettingsOverview, InsightsError> {
    let status_count = |status: &str| {
        connection.query_row(
            "SELECT COUNT(*) FROM problems WHERE account_id = ?1 AND profile_id = ?2 AND status = ?3",
            params![account_id, profile_id, status],
            |row| row.get(0),
        )
    };
    Ok(SettingsOverview {
        active_problem_count: bounded_i32(status_count("active")?),
        archived_problem_count: bounded_i32(status_count("archived")?),
        trashed_problem_count: bounded_i32(status_count("trashed")?),
        pending_operation_count: bounded_i32(connection.query_row(
            "SELECT COUNT(*) FROM sync_operations WHERE account_id = ?1 AND profile_id = ?2 AND status = 'pending'",
            params![account_id, profile_id],
            |row| row.get(0),
        )?),
        failed_operation_count: bounded_i32(connection.query_row(
            "SELECT COUNT(*) FROM sync_operations WHERE account_id = ?1 AND profile_id = ?2 AND status = 'failed'",
            params![account_id, profile_id],
            |row| row.get(0),
        )?),
        unresolved_conflict_count: bounded_i32(connection.query_row(
            "SELECT COUNT(*) FROM sync_conflicts WHERE account_id = ?1 AND profile_id = ?2 AND resolved_at_utc_ms IS NULL",
            params![account_id, profile_id],
            |row| row.get(0),
        )?),
        local_encryption_ready: true,
        cloud_sync_configured: false,
    })
}

fn scalar(
    connection: &Connection,
    sql: &str,
    account_id: &str,
    profile_id: &str,
) -> Result<i64, rusqlite::Error> {
    connection.query_row(sql, params![account_id, profile_id], |row| row.get(0))
}

fn current_streak(days: &[DailyActivity]) -> i32 {
    let mut days = days.iter().rev();
    let Some(today) = days.next() else { return 0 };
    let mut streak = 0;
    if today.review_count > 0 {
        streak = 1;
    } else if days.clone().next().is_none_or(|day| day.review_count == 0) {
        return 0;
    }
    for day in days {
        if day.review_count == 0 {
            break;
        }
        streak += 1;
    }
    streak
}

fn bounded_i32(value: i64) -> i32 {
    i32::try_from(value).unwrap_or(i32::MAX)
}
