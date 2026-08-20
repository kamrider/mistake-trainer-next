use std::{cmp::Ordering, collections::BTreeMap};

use rusqlite::{Connection, params};
use time::OffsetDateTime;

use super::{
    DailyActivity, DailyPlanOverview, DashboardOverview, DueForecastDay, InsightsError,
    ReportSummary, SettingsOverview, SubjectActivity, WeakAreaSummary,
};

const DAY_MS: i64 = 86_400_000;
const REPORT_DAYS: i64 = 14;

pub(super) fn dashboard_overview(
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
        |row| row.get::<_, String>(0),
    )?;
    let (review_target, minutes_target): (i64, i64) = connection.query_row(
        "SELECT COALESCE(MAX(daily_review_target), 20),
                COALESCE(MAX(daily_minutes_target), 20)
         FROM profile_preferences
         WHERE account_id = ?1 AND profile_id = ?2",
        params![account_id, profile_id],
        |row| Ok((row.get(0)?, row.get(1)?)),
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
    let recent_average_duration_ms: Option<f64> = connection.query_row(
        "SELECT AVG(duration_ms) FROM review_events
         WHERE account_id = ?1 AND profile_id = ?2
           AND occurred_at_utc_ms >= ?3 AND occurred_at_utc_ms < ?4",
        params![
            account_id,
            profile_id,
            thirty_day_start_utc_ms,
            tomorrow_start_utc_ms
        ],
        |row| row.get(0),
    )?;

    let review_day_buckets = review_day_buckets(connection, account_id, profile_id, offset_ms)?;
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

    let completed_reviews = bounded_i32(reviewed_today_count);
    let review_target = bounded_i32(review_target);
    let due_reviews = bounded_i32(due_problem_count);
    let remaining_reviews = review_target.saturating_sub(completed_reviews).max(0);
    let suggested_reviews = due_reviews.max(remaining_reviews);
    let estimated_minutes = if suggested_reviews == 0 {
        0
    } else {
        let average_duration_ms = recent_average_duration_ms
            .filter(|duration| duration.is_finite() && *duration > 0.0)
            .unwrap_or(60_000.0);
        ((average_duration_ms * f64::from(suggested_reviews) / 60_000.0).ceil() as i32)
            .clamp(1, 240)
    };

    Ok(DashboardOverview {
        profile_name,
        active_problem_count: bounded_i32(active_problem_count),
        due_problem_count: bounded_i32(due_problem_count),
        reviewed_today_count: bounded_i32(reviewed_today_count),
        remembered_rate_30_days,
        current_streak_days,
        pending_capture_batch_count: bounded_i32(pending_capture_batch_count),
        pending_capture_item_count: bounded_i32(pending_capture_item_count),
        daily_plan: DailyPlanOverview {
            review_target,
            minutes_target: bounded_i32(minutes_target),
            completed_reviews,
            remaining_reviews,
            due_reviews,
            suggested_reviews,
            estimated_minutes,
        },
    })
}

pub(super) fn report_summary(
    connection: &Connection,
    account_id: &str,
    profile_id: &str,
    now_utc_ms: i64,
    utc_offset_minutes: i32,
) -> Result<ReportSummary, InsightsError> {
    if !(-840..=840).contains(&utc_offset_minutes) {
        return Err(InsightsError::InvalidTimezoneOffset);
    }
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
    let offset_ms = i64::from(utc_offset_minutes) * 60_000;
    let today_bucket = (now_utc_ms + offset_ms).div_euclid(DAY_MS);
    let start_bucket = today_bucket - (REPORT_DAYS - 1);
    let start_utc_ms = start_bucket * DAY_MS - offset_ms;
    let tomorrow_start_utc_ms = (today_bucket + 1) * DAY_MS - offset_ms;
    let mut daily_activity = (0..REPORT_DAYS)
        .map(|offset| DailyActivity {
            day_start_utc_ms: ((start_bucket + offset) * DAY_MS - offset_ms) as f64,
            review_count: 0,
            duration_ms: 0.0,
        })
        .collect::<Vec<_>>();
    {
        let mut statement = connection.prepare(
            "SELECT (occurred_at_utc_ms + ?1) / ?2 AS day_bucket, COUNT(*), COALESCE(SUM(duration_ms), 0)
             FROM review_events
             WHERE account_id = ?3 AND profile_id = ?4 AND occurred_at_utc_ms >= ?5 AND occurred_at_utc_ms < ?6
             GROUP BY day_bucket ORDER BY day_bucket",
        )?;
        let rows = statement.query_map(
            params![
                offset_ms,
                DAY_MS,
                account_id,
                profile_id,
                start_utc_ms,
                tomorrow_start_utc_ms
            ],
            |row| {
                Ok((
                    row.get::<_, i64>(0)?,
                    row.get::<_, i64>(1)?,
                    row.get::<_, i64>(2)?,
                ))
            },
        )?;
        for row in rows {
            let (day_bucket, count, duration) = row?;
            if let Ok(index) = usize::try_from(day_bucket - start_bucket)
                && let Some(item) = daily_activity.get_mut(index)
            {
                item.review_count = bounded_i32(count);
                item.duration_ms = duration as f64;
            }
        }
    }
    let review_day_buckets = review_day_buckets(connection, account_id, profile_id, offset_ms)?;
    let current_streak_days = current_streak_from_buckets(&review_day_buckets, today_bucket);
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
    let weak_areas = weak_areas(connection, account_id, profile_id, now_utc_ms)?;
    let due_forecast = due_forecast(
        connection,
        account_id,
        profile_id,
        now_utc_ms,
        utc_offset_minutes,
    )?;

    Ok(ReportSummary {
        active_problem_count: bounded_i32(active_problem_count),
        due_problem_count: bounded_i32(due_problem_count),
        review_count: bounded_i32(review_count),
        remembered_rate,
        total_duration_ms: total_duration_ms as f64,
        current_streak_days,
        daily_activity,
        subject_activity,
        weak_areas,
        due_forecast,
    })
}

#[derive(Default)]
struct WeakAreaAccumulator {
    reviewed_count: i64,
    lapse_count: i64,
    duration_total_ms: i64,
    duration_count: i64,
}

fn weak_areas(
    connection: &Connection,
    account_id: &str,
    profile_id: &str,
    now_utc_ms: i64,
) -> Result<Vec<WeakAreaSummary>, InsightsError> {
    let cutoff = now_utc_ms.saturating_sub(30 * DAY_MS);
    let mut statement = connection.prepare(
        "SELECT CASE WHEN trim(p.subject) = '' THEN '未分类' ELSE p.subject END,
                p.tags_json, review.rating, review.duration_ms
         FROM review_events review
         JOIN problems p ON p.id = review.problem_id
         WHERE review.account_id = ?1 AND review.profile_id = ?2
           AND p.account_id = ?1 AND p.profile_id = ?2
           AND review.occurred_at_utc_ms >= ?3 AND review.occurred_at_utc_ms <= ?4",
    )?;
    let rows = statement
        .query_map(params![account_id, profile_id, cutoff, now_utc_ms], |row| {
            Ok((
                row.get::<_, String>(0)?,
                row.get::<_, String>(1)?,
                row.get::<_, String>(2)?,
                row.get::<_, Option<i64>>(3)?,
            ))
        })?
        .collect::<Result<Vec<_>, _>>()?;
    let mut areas = BTreeMap::<(String, String), WeakAreaAccumulator>::new();
    for (subject, tags_json, rating, duration_ms) in rows {
        accumulate_weak_area(
            areas.entry(("subject".to_owned(), subject)).or_default(),
            &rating,
            duration_ms,
        );
        for tag in serde_json::from_str::<Vec<String>>(&tags_json).unwrap_or_default() {
            if tag.starts_with("错因·") {
                accumulate_weak_area(
                    areas.entry(("reason".to_owned(), tag)).or_default(),
                    &rating,
                    duration_ms,
                );
            }
        }
    }
    let mut summaries = areas
        .into_iter()
        .filter(|(_, area)| area.reviewed_count >= 2)
        .map(|((kind, label), area)| WeakAreaSummary {
            label,
            kind,
            reviewed_count: bounded_i32(area.reviewed_count),
            lapse_count: bounded_i32(area.lapse_count),
            lapse_rate: area.lapse_count as f64 / area.reviewed_count as f64,
            average_duration_ms: if area.duration_count == 0 {
                0.0
            } else {
                area.duration_total_ms as f64 / area.duration_count as f64
            },
        })
        .collect::<Vec<_>>();
    summaries.sort_by(|left, right| {
        right
            .lapse_rate
            .partial_cmp(&left.lapse_rate)
            .unwrap_or(Ordering::Equal)
            .then_with(|| right.lapse_count.cmp(&left.lapse_count))
            .then_with(|| right.reviewed_count.cmp(&left.reviewed_count))
            .then_with(|| left.label.cmp(&right.label))
    });
    summaries.truncate(5);
    Ok(summaries)
}

fn accumulate_weak_area(area: &mut WeakAreaAccumulator, rating: &str, duration_ms: Option<i64>) {
    area.reviewed_count += 1;
    area.lapse_count += i64::from(rating == "again");
    if let Some(duration_ms) = duration_ms.filter(|duration| *duration >= 0) {
        area.duration_total_ms = area.duration_total_ms.saturating_add(duration_ms);
        area.duration_count += 1;
    }
}

fn due_forecast(
    connection: &Connection,
    account_id: &str,
    profile_id: &str,
    now_utc_ms: i64,
    utc_offset_minutes: i32,
) -> Result<Vec<DueForecastDay>, InsightsError> {
    let offset_ms = i64::from(utc_offset_minutes) * 60_000;
    let today_bucket = (now_utc_ms + offset_ms).div_euclid(DAY_MS);
    let today_start_utc_ms = today_bucket * DAY_MS - offset_ms;
    let forecast_end_utc_ms = today_start_utc_ms + 7 * DAY_MS;
    let mut counts = [0_i64; 7];
    let mut overdue_count = 0_i64;
    let mut statement = connection.prepare(
        "SELECT schedule.due_at_utc_ms
         FROM schedule_states schedule
         JOIN problems p ON p.id = schedule.problem_id
         WHERE p.account_id = ?1 AND p.profile_id = ?2 AND p.status = 'active'
           AND schedule.due_at_utc_ms < ?3",
    )?;
    for due_at in statement.query_map(
        params![account_id, profile_id, forecast_end_utc_ms],
        |row| row.get::<_, i64>(0),
    )? {
        let due_at = due_at?;
        if due_at < today_start_utc_ms {
            overdue_count += 1;
            continue;
        }
        let index = ((due_at + offset_ms).div_euclid(DAY_MS) - today_bucket) as usize;
        if let Some(count) = counts.get_mut(index) {
            *count += 1;
        }
    }
    (0..7)
        .map(|index| {
            let local_day_start_ms = (today_bucket + index as i64) * DAY_MS;
            let local_date = OffsetDateTime::from_unix_timestamp(local_day_start_ms / 1_000)
                .map_err(|_| InsightsError::InvalidTimezoneOffset)?
                .date()
                .to_string();
            Ok(DueForecastDay {
                local_date,
                due_count: bounded_i32(counts[index]),
                overdue_count: if index == 0 {
                    bounded_i32(overdue_count)
                } else {
                    0
                },
            })
        })
        .collect()
}

pub(super) fn settings_overview(
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

fn review_day_buckets(
    connection: &Connection,
    account_id: &str,
    profile_id: &str,
    offset_ms: i64,
) -> Result<Vec<i64>, rusqlite::Error> {
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
        .collect::<Result<Vec<_>, _>>()
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
