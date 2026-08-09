import { existsSync, readFileSync } from 'node:fs'
import { resolve } from 'node:path'
import { describe, expect, it } from 'vitest'

const facadePath = resolve('src-tauri/src/modules/review.rs')
const submissionPath = resolve('src-tauri/src/modules/review_submission.rs')

const readSource = (path: string) => readFileSync(path, 'utf8')

describe('review submission boundary', () => {
  it('keeps the stable review API behind a private submission child', () => {
    const facade = readSource(facadePath)

    expect(facade).toMatch(
      /#\[path = "review_submission\.rs"\]\r?\nmod submission;/,
    )
    expect(facade).toMatch(/pub fn submit_review\(/)
    expect(facade).toMatch(/pub\(crate\) fn rebuild_schedule_for_problem\(/)
    expect(facade).toContain('submission::submit_review(')
    expect(facade).toContain('submission::rebuild_schedule_for_problem(')
    expect(facade).toContain('pub(crate) const ALGORITHM_VERSION')
    expect(facade).toContain('pub(crate) const PARAMETER_VERSION')
  })

  it('isolates atomic submission and deterministic schedule rebuilding', () => {
    const facade = readSource(facadePath)
    const submissionExists = existsSync(submissionPath)

    expect(submissionExists).toBe(true)
    if (!submissionExists) return

    const submission = readSource(submissionPath)
    const exposedOperations = [
      ...submission.matchAll(/pub\(super\) fn ([a-z_]+)\(/g),
    ].map((match) => match[1])
    expect(exposedOperations).toEqual([
      'submit_review',
      'rebuild_schedule_for_problem',
    ])

    for (const name of [
      'rebuild_schedule',
      'rating_label',
      'parse_rating',
    ]) {
      expect(submission).toMatch(
        new RegExp(`(?:^|\\n)(?:const )?fn ${name}\\(`),
      )
      expect(facade).not.toMatch(new RegExp(`\\bfn ${name}\\(`))
    }
    for (const token of [
      'struct StoredEvent',
      'struct ReviewEventPayload',
      'const DESIRED_RETENTION',
      'const MILLIS_PER_DAY',
      'FSRS::default()',
      'ORDER BY occurred_at_utc_ms, id',
      'INSERT INTO review_events',
      'INSERT INTO schedule_states',
      'INSERT INTO sync_operations',
      'UPDATE review_sessions',
      'start_interval_focus_if_due',
      'transaction.commit()',
    ]) {
      expect(submission).toContain(token)
    }
    expect(facade).not.toContain('const DESIRED_RETENTION')
    expect(facade).not.toContain('const MILLIS_PER_DAY')

    let previousIndex = -1
    for (const orderedStep of [
      'INSERT INTO review_events',
      'INSERT INTO schedule_states',
      'INSERT INTO sync_operations',
      'UPDATE review_sessions',
      'start_interval_focus_if_due',
      'transaction.commit()',
    ]) {
      const currentIndex = submission.indexOf(orderedStep, previousIndex + 1)
      expect(currentIndex).toBeGreaterThan(previousIndex)
      previousIndex = currentIndex
    }
    for (const queueOperation of [
      'active_queue_state',
      'list_review_queue',
      'start_manual_review_queue',
      'start_exam_review_queue',
      'navigate_exam',
      'begin_exam_grading',
      'query_new_review_entries',
      'queue_entries_for_ids',
    ]) {
      expect(facade).toMatch(new RegExp(`\\bfn ${queueOperation}\\(`))
      expect(submission).not.toMatch(new RegExp(`\\bfn ${queueOperation}\\(`))
    }
  })
})
