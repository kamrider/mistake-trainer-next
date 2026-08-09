import { globSync, readFileSync } from 'node:fs'
import { resolve } from 'node:path'
import { describe, expect, it } from 'vitest'

describe('repository quality gates', () => {
  it('keeps browser-native blocking dialogs out of product Vue surfaces', () => {
    for (const file of globSync(['src/**/*.vue', 'src/**/*.ts'])) {
      const source = readFileSync(file, 'utf8')
      expect(source, file).not.toMatch(/\bwindow\.(?:alert|confirm|prompt)\s*\(/)
    }
  })

  it('keeps the documented local and CI commands available', () => {
    const packageJson = JSON.parse(readFileSync(resolve('package.json'), 'utf8')) as {
      scripts: Record<string, string>
    }

    expect(packageJson.scripts).toMatchObject({
      lint: expect.any(String),
      typecheck: expect.any(String),
      test: expect.any(String),
      'bindings:check': expect.any(String),
      'contract:rust-boundaries': expect.any(String),
      'supabase:test': expect.any(String),
    })
  })

  it('runs the product gates in the Windows release workflow', () => {
    const workflow = readFileSync(resolve('.github/workflows/ci.yml'), 'utf8')

    for (const command of [
      'pnpm lint',
      'pnpm typecheck',
      'pnpm test',
      'pnpm contract:rust-boundaries',
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

  it('ships a hash-pinned local anchor runtime with a safe visual fallback', () => {
    const startup = readFileSync(resolve('src-tauri/src/lib.rs'), 'utf8')
    const worker = readFileSync(
      resolve('src-tauri/src/infrastructure/capture_recognition_worker.rs'),
      'utf8',
    )
    const product = readFileSync(
      resolve('src-tauri/src/infrastructure/recognition_product.rs'),
      'utf8',
    )
    const capability = readFileSync(resolve('src-tauri/src/modules/ocr_capability.rs'), 'utf8')
    const cargo = readFileSync(resolve('src-tauri/Cargo.toml'), 'utf8')
    const tauri = readFileSync(resolve('src-tauri/tauri.conf.json'), 'utf8')

    expect(startup).toContain('CaptureRecognitionManager::for_product')
    expect(startup).not.toContain('CaptureRecognitionManager::default()')
    expect(worker).toContain('ProductRecognitionEngine')
    expect(product).toContain('PpOcrLocalRuntime')
    expect(product).toContain('VisualSplitRecognitionEngine')
    expect(product).not.toContain('http://')
    expect(product).not.toContain('https://')
    expect(cargo).toContain('default = ["custom-protocol", "local-ocr-runtime"]')
    expect(tauri).toContain('ocr-runtime/win-x64/onnxruntime.dll')
    expect(tauri).toContain('Microsoft-ONNX-Runtime-LICENSE.txt')
    expect(capability).toContain('visual_split_feature_status()')
    expect(capability).toContain(
      'RECOGNITION_RUNTIME_AVAILABLE && cfg!(feature = "local-ocr-runtime")',
    )
  })

  it('checks Rust dependency boundaries before compiling Rust targets in CI', () => {
    const workflow = readFileSync(resolve('.github/workflows/ci.yml'), 'utf8')
    const appJobStart = workflow.indexOf('  app:')
    const nextJobStart = workflow.indexOf('  windows-package:')
    const appJob = workflow.slice(appJobStart, nextJobStart)
    const boundaryContract = appJob.indexOf('pnpm contract:rust-boundaries')
    const bindingsCompilation = appJob.indexOf('pnpm bindings:check')
    const rustFormat = appJob.indexOf('cargo fmt --manifest-path')

    expect(appJobStart).toBeGreaterThan(-1)
    expect(nextJobStart).toBeGreaterThan(appJobStart)
    expect(boundaryContract).toBeGreaterThan(-1)
    expect(bindingsCompilation).toBeGreaterThan(boundaryContract)
    expect(rustFormat).toBeGreaterThan(boundaryContract)
  })

  it('keeps signed update networking inside typed Rust commands', () => {
    const commands = readFileSync(resolve('src-tauri/src/commands/updates.rs'), 'utf8')
    const bindings = readFileSync(resolve('src-tauri/src/bindings.rs'), 'utf8')
    const startup = readFileSync(resolve('src-tauri/src/lib.rs'), 'utf8')
    const packageJson = readFileSync(resolve('package.json'), 'utf8')
    const defaultCapability = readFileSync(
      resolve('src-tauri/capabilities/default.json'),
      'utf8',
    )

    for (const command of [
      'windows_update_status',
      'windows_update_check',
      'windows_update_install',
    ]) {
      expect(commands).toContain(`fn ${command}`)
      expect(bindings).toContain(`commands::updates::${command}`)
    }
    expect(packageJson).not.toContain('@tauri-apps/plugin-updater')
    expect(defaultCapability).not.toContain('updater:')
    expect(startup).toContain('commands::updates::updater_config_is_ready(')
  })
})
