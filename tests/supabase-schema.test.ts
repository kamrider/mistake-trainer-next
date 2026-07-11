import { readFileSync } from 'node:fs'
import { resolve } from 'node:path'
import { describe, expect, it } from 'vitest'

const migrationPath = resolve('supabase/migrations/202607110001_initial_sync.sql')

describe('Supabase sync schema', () => {
  it('enables RLS for every account-owned cloud table', () => {
    const sql = readFileSync(migrationPath, 'utf8')
    const tables = [
      'learner_profiles',
      'problems',
      'assets',
      'problem_assets',
      'review_events',
      'schedule_states',
      'export_snapshots',
      'tombstones',
    ]

    for (const table of tables) {
      expect(sql).toContain(`alter table public.${table} enable row level security;`)
      expect(sql).toContain(`on public.${table}`)
    }
  })

  it('uses an ordered server sequence and immutable review events', () => {
    const sql = readFileSync(migrationPath, 'utf8')

    expect(sql).toContain('create sequence public.app_change_seq')
    expect(sql).toContain("default nextval('public.app_change_seq')")
    expect(sql).toContain('prevent_review_event_mutation')
    expect(sql).toContain('pull_profile_changes')
  })

  it('keeps private assets inside an account-prefixed storage path', () => {
    const sql = readFileSync(migrationPath, 'utf8')

    expect(sql).toContain("values ('mistake-assets', 'mistake-assets', false)")
    expect(sql).toContain("(storage.foldername(name))[1] = auth.uid()::text")
  })
})
