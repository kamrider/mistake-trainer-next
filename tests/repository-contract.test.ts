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

  it('routes desktop commands through the reproducible MSVC toolchain wrapper', () => {
    const packageJson = JSON.parse(readFileSync(resolve('package.json'), 'utf8')) as {
      scripts: Record<string, string>
    }
    const wrapper = readFileSync(resolve('scripts/tauri-msvc.cmd'), 'utf8')

    expect(packageJson.scripts.tauri).toContain('scripts\\tauri-msvc.cmd')
    expect(wrapper).toContain('node_modules\\.bin\\tauri.cmd')
    expect(wrapper).not.toContain('pnpm exec tauri')
    expect(wrapper).toContain('strawberry-perl')
  })

  it('ships product visual splitting without an OCR release or model gate', () => {
    const startup = readFileSync(resolve('src-tauri/src/lib.rs'), 'utf8')
    const worker = readFileSync(
      resolve('src-tauri/src/infrastructure/capture_recognition_worker.rs'),
      'utf8',
    )
    const capability = readFileSync(resolve('src-tauri/src/modules/ocr_capability.rs'), 'utf8')

    expect(startup).toContain('CaptureRecognitionManager::for_product')
    expect(startup).not.toContain('CaptureRecognitionManager::default()')
    expect(worker).toContain('VisualSplitRecognitionEngine')
    expect(worker).not.toContain('LazyPpOcrRecognitionEngine')
    expect(worker).not.toContain('PpOcrLocalRuntime')
    expect(capability).toContain('visual_split_feature_status()')
    expect(capability).toContain(
      'RECOGNITION_RUNTIME_AVAILABLE && cfg!(feature = "local-ocr-runtime")',
    )
  })
})
