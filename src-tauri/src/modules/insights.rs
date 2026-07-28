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
pub struct DashboardOverview {
    pub profile_name: String,
    pub active_problem_count: i32,
    pub due_problem_count: i32,
    pub reviewed_today_count: i32,
    pub remembered_rate_30_days: Option<f64>,
    pub current_streak_days: i32,
    pub pending_capture_batch_count: i32,
    pub pending_capture_item_count: i32,
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
    #[error("timezone offset is outside the supported range")]
    InvalidTimezoneOffset,
    #[error("report query failed")]
    Database(#[from] rusqlite::Error),
}

pub fn dashboard_overview(
    connection: &Connection,
    account_id: &str,
    profile_id: &str,
    now_utc_ms: i64,
    utc_offset_minutes: i32,
) -> Result<DashboardOverview, InsightsError> {
    if !(-840..=840).contains(&utc_offset_minutes) {
        return Err(InsightsError::InvalidTimezoneOffset);
    }

    let offset_ms = i64::from(utc_offset_minutes) * 60_000;
    let today_bucket = (now_utc_ms + offset_ms).div_euclid(DAY_MS);
    let today_start_utc_ms = today_bucket * DAY_MS - offset_ms;
    let tomorrow_start_utc_ms = today_start_utc_ms + DAY_MS;
    let thirty_day_start_utc_ms = today_start_utc_ms - 29 * DAY_MS;

    let profile_name = connection.query_row(
        "SELECT name FROM learner_profiles WHERE account_id = ?1 AND id = ?2",
        params![account_id, profile_id],
        |row| row.get(0),
    )?;
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
        |row| row.get::<_, i64>(0),
    )?;
    let reviewed_today_count = connection.query_row(
        "SELECT COUNT(*) FROM review_events
         WHERE account_id = ?1 AND profile_id = ?2
           AND occurred_at_utc_ms >= ?3 AND occurred_at_utc_ms < ?4",
        params![
            account_id,
            profile_id,
            today_start_utc_ms,
            tomorrow_start_utc_ms
        ],
        |row| row.get::<_, i64>(0),
    )?;
    let (recent_review_count, recent_remembered_count): (i64, i64) = connection.query_row(
        "SELECT COUNT(*), COALESCE(SUM(CASE WHEN rating != 'again' THEN 1 ELSE 0 END), 0)
         FROM review_events
         WHERE account_id = ?1 AND profile_id = ?2
           AND occurred_at_utc_ms >= ?3 AND occurred_at_utc_ms < ?4",
        params![
            account_id,
            profile_id,
            thirty_day_start_utc_ms,
            tomorrow_start_utc_ms
        ],
        |row| Ok((row.get(0)?, row.get(1)?)),
    )?;
    let remembered_rate_30_days = (recent_review_count > 0)
        .then(|| recent_remembered_count as f64 / recent_review_count as f64);

    let review_day_buckets = {
        let mut statement = connection.prepare(
            "SELECT DISTINCT (occurred_at_utc_ms + ?3) / ?4 AS day_bucket
             FROM review_events
             WHERE account_id = ?1 AND profile_id = ?2
             ORDER BY day_bucket DESC",
        )?;
        statement
            .query_map(params![account_id, profile_id, offset_ms, DAY_MS], |row| {
                row.get::<_, i64>(0)
            })?
            .collect::<Result<Vec<_>, _>>()?
    };
    let current_streak_days = current_streak_from_buckets(&review_day_buckets, today_bucket);

    let pending_capture_batch_count = connection.query_row(
        "SELECT COUNT(*) FROM capture_batches
         WHERE account_id = ?1 AND profile_id = ?2 AND state != 'completed'",
        params![account_id, profile_id],
        |row| row.get::<_, i64>(0),
    )?;
    let pending_capture_item_count = connection.query_row(
        "SELECT COUNT(*) FROM capture_items i
         INNER JOIN capture_batches b ON b.id = i.batch_id
         WHERE b.account_id = ?1 AND b.profile_id = ?2 AND b.state != 'completed'",
        params![account_id, profile_id],
        |row| row.get::<_, i64>(0),
    )?;

    Ok(DashboardOverview {
        profile_name,
        active_problem_count: bounded_i32(active_problem_count),
        due_problem_count: bounded_i32(due_problem_count),
        reviewed_today_count: bounded_i32(reviewed_today_count),
        remembered_rate_30_days,
        current_streak_days,
        pending_capture_batch_count: bounded_i32(pending_capture_batch_count),
        pending_capture_item_count: bounded_i32(pending_capture_item_count),
    })
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
            if let Ok(index) = usize::try_from((day - start).div_euclid(DAY_MS))
                && let Some(item) = daily_activity.get_mut(index)
            {
                item.review_count = bounded_i32(count);
                item.duration_ms = duration as f64;
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

fn current_streak_from_buckets(day_buckets: &[i64], today_bucket: i64) -> i32 {
    let Some(latest_bucket) = day_buckets.first().copied() else {
        return 0;
    };
    let mut expected_bucket = if latest_bucket == today_bucket {
        today_bucket
    } else if latest_bucket == today_bucket - 1 {
        today_bucket - 1
    } else {
        return 0;
    };
    let mut streak = 0;
    for bucket in day_buckets {
        if *bucket > expected_bucket {
            continue;
        }
        if *bucket != expected_bucket {
            break;
        }
        streak += 1;
        expected_bucket -= 1;
    }
    streak
}

fn bounded_i32(value: i64) -> i32 {
    i32::try_from(value).unwrap_or(i32::MAX)
}
