import { existsSync, readFileSync } from 'node:fs'
import { resolve } from 'node:path'
import { describe, expect, it } from 'vitest'

const facadePath = resolve('src-tauri/src/modules/review_history.rs')
const repositoryPath = resolve(
  'src-tauri/src/modules/review_history_list_repository.rs',
)

const readSource = (path: string) => readFileSync(path, 'utf8')

describe('review history list repository boundary', () => {
  it('keeps the public list API behind one private repository operation', () => {
    const facade = readSource(facadePath)

    expect(facade).toMatch(
      /#\[path = "review_history_list_repository\.rs"\]\r?\nmod list_repository;/,
    )
    expect(facade).toMatch(/pub fn list_review_history\(/)
    expect(facade).toContain('list_repository::list_review_history(connection, query)')
    expect(facade).toMatch(/pub fn get_review_history_detail\(/)
    expect(facade).not.toContain('list_repository::get_review_history_detail')

    expect(existsSync(repositoryPath)).toBe(true)
    if (!existsSync(repositoryPath)) return

    const repository = readSource(repositoryPath)
    expect(repository).toMatch(/pub\(super\) fn list_review_history\(/)
    expect(repository.match(/pub\(super\) fn /g)).toHaveLength(1)
  })

  it('isolates list policy and projections while retaining audit detail ownership', () => {
    const facade = readSource(facadePath)
    if (!existsSync(repositoryPath)) return
    const repository = readSource(repositoryPath)

    for (const token of [
      'const FILTER_SQL',
      'struct ReviewHistoryCursor',
      'struct ValidatedQuery',
      'fn validate_query(',
      'fn encode_cursor(',
      'fn decode_cursor(',
      'fn escape_like(',
      'fn note_preview(',
      'URL_SAFE_NO_PAD',
      'named_params!',
      'ORDER BY e.occurred_at_utc_ms DESC, e.id DESC',
      'SELECT DISTINCT p.subject',
    ]) {
      expect(repository).toContain(token)
      expect(facade).not.toContain(token)
    }

    for (const token of [
      'pub fn get_review_history_detail(',
      'type DetailRow',
      'CurrentScheduleProjection',
      'is_current_device:',
      'review_ordinal:',
      'problem_review_count:',
      'LEFT JOIN schedule_states',
    ]) {
      expect(facade).toContain(token)
      expect(repository).not.toContain(token)
    }

    expect(facade).toContain('fn parse_rating(')
    expect(facade).toContain('fn bounded_i32(')
    expect(repository).toContain('parse_rating(&rating)?')
    expect(repository).toContain('bounded_i32(total_count)')
    expect(facade).toContain('const MAX_EVENT_ID_CHARS')
  })

  it('preserves account and profile isolation in both read models', () => {
    const facade = readSource(facadePath)
    if (!existsSync(repositoryPath)) return
    const repository = readSource(repositoryPath)

    for (const source of [facade, repository]) {
      expect(source).toContain('p.account_id = e.account_id')
      expect(source).toContain('p.profile_id = e.profile_id')
    }
    expect(repository).toContain('e.account_id = :account_id')
    expect(repository).toContain('e.profile_id = :profile_id')
    expect(facade).toContain('e.account_id = ?1 AND e.profile_id = ?2 AND e.id = ?3')
    expect(facade).not.toContain('pub device_id')
  })
})
