import { readFileSync } from 'node:fs'
import { resolve } from 'node:path'
import { describe, expect, it } from 'vitest'

describe('repository quality gates', () => {
  it('keeps the documented local and CI commands available', () => {
    const packageJson = JSON.parse(readFileSync(resolve('package.json'), 'utf8')) as {
      scripts: Record<string, string>
    }

    expect(packageJson.scripts).toMatchObject({
      lint: expect.any(String),
      typecheck: expect.any(String),
      test: expect.any(String),
      'bindings:check': expect.any(String),
      'supabase:test': expect.any(String),
    })
  })

  it('runs the product gates in the Windows release workflow', () => {
    const workflow = readFileSync(resolve('.github/workflows/ci.yml'), 'utf8')

    for (const command of [
      'pnpm lint',
      'pnpm typecheck',
      'pnpm test',
      'cargo test --all-targets',
      'supabase test db',
      'pnpm tauri build',
    ]) {
      expect(workflow).toContain(command)
    }
  })

  it('builds the frontend before compiling Tauri test targets', () => {
    const workflow = readFileSync(resolve('.github/workflows/ci.yml'), 'utf8')
    const frontendBuild = workflow.indexOf('- run: pnpm build')
    const rustTests = workflow.indexOf('cargo test --all-targets')

    expect(frontendBuild).toBeGreaterThan(-1)
    expect(rustTests).toBeGreaterThan(frontendBuild)
  })
})
