import { existsSync, readFileSync } from 'node:fs'
import { resolve } from 'node:path'
import { describe, expect, it } from 'vitest'

const facadePath = resolve('src-tauri/src/modules/insights.rs')
const repositoryPath = resolve(
  'src-tauri/src/modules/insights_read_repository.rs',
)
const readSource = (path: string) => readFileSync(path, 'utf8')

describe('insights read repository boundary', () => {
  it('keeps the public insight API behind one private repository', () => {
    const facade = readSource(facadePath)

    expect(facade).toMatch(
      /#\[path = "insights_read_repository\.rs"\]\r?\nmod read_repository;/,
    )
    for (const operation of [
      'dashboard_overview',
      'report_summary',
      'settings_overview',
    ]) {
      expect(facade).toMatch(new RegExp(`pub fn ${operation}\\(`))
      expect(facade).toContain(`read_repository::${operation}(`)
    }

    expect(existsSync(repositoryPath)).toBe(true)
    if (!existsSync(repositoryPath)) return

    const repository = readSource(repositoryPath)
    expect(repository.match(/pub\(super\) fn /g)).toHaveLength(3)
  })

  it('isolates persistence and projection policy while retaining public contracts', () => {
    const facade = readSource(facadePath)
    if (!existsSync(repositoryPath)) return
    const repository = readSource(repositoryPath)

    for (const publicType of [
      'pub struct DailyActivity',
      'pub struct SubjectActivity',
      'pub struct ReportSummary',
      'pub struct DashboardOverview',
      'pub struct SettingsOverview',
      'pub enum InsightsError',
    ]) {
      expect(facade).toContain(publicType)
      expect(repository).not.toContain(publicType)
    }

    for (const token of [
      'const DAY_MS',
      'const REPORT_DAYS',
      'fn scalar(',
      'fn review_day_buckets(',
      'fn current_streak_from_buckets(',
      'fn bounded_i32(',
      'SELECT name FROM learner_profiles',
      'FROM review_events',
      'FROM capture_batches',
      'FROM sync_operations',
      'FROM sync_conflicts',
    ]) {
      expect(repository).toContain(token)
      expect(facade).not.toContain(token)
    }
    expect(repository).toContain('use rusqlite::{Connection, params};')
    expect(facade).not.toContain('params!')
  })

  it('locks time windows, ordering, and tenant scope in the repository', () => {
    if (!existsSync(repositoryPath)) return
    const repository = readSource(repositoryPath)

    for (const token of [
      '!(-840..=840).contains(&utc_offset_minutes)',
      'today_start_utc_ms - 29 * DAY_MS',
      'const REPORT_DAYS: i64 = 14',
      'account_id = ?1 AND profile_id = ?2',
      'e.account_id = p.account_id AND e.profile_id = p.profile_id',
      'ORDER BY COUNT(e.id) DESC, COUNT(DISTINCT p.id) DESC, p.subject',
      'i32::try_from(value).unwrap_or(i32::MAX)',
    ]) {
      expect(repository).toContain(token)
    }
  })
})
