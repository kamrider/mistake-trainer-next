import { readFileSync, readdirSync } from 'node:fs'
import { dirname, extname, relative, resolve } from 'node:path'
import { describe, expect, it } from 'vitest'

const repositoryRoot = resolve('.')

function normalizePath(path: string) {
  return path.replaceAll('\\', '/')
}

function sourceFiles(root: string, extensions: ReadonlySet<string>): string[] {
  const absoluteRoot = resolve(repositoryRoot, root)
  const files: string[] = []

  function visit(directory: string) {
    for (const entry of readdirSync(directory, { withFileTypes: true })) {
      const path = resolve(directory, entry.name)
      if (entry.isDirectory()) visit(path)
      else if (extensions.has(extname(entry.name))) files.push(path)
    }
  }

  visit(absoluteRoot)
  return files.sort()
}

function repositoryPath(path: string) {
  return normalizePath(relative(repositoryRoot, path))
}

function importSpecifiers(source: string) {
  const specifiers: Array<{ line: number; value: string }> = []
  const pattern = /\b(?:from\s*|import\s*\()\s*['"]([^'"]+)['"]/g
  for (const match of source.matchAll(pattern)) {
    const index = match.index ?? 0
    specifiers.push({
      line: source.slice(0, index).split(/\r?\n/).length,
      value: match[1]!,
    })
  }
  return specifiers
}

function resolvesIntoRoot(file: string, specifier: string, root: 'app' | 'modules') {
  if (specifier === `@/${root}` || specifier.startsWith(`@/${root}/`)) return true
  if (!specifier.startsWith('.')) return false
  const imported = normalizePath(resolve(dirname(file), specifier))
  const absoluteRoot = `${normalizePath(resolve(repositoryRoot, `src/${root}`))}/`
  return imported.startsWith(absoluteRoot)
}

function frontendFeatureToAppImports() {
  return sourceFiles('src/modules', new Set(['.ts', '.vue']))
    .filter(file => !/\.(?:test|spec)\.ts$/.test(file))
    .flatMap((file) => {
      const source = readFileSync(file, 'utf8')
      return importSpecifiers(source)
        .filter(({ value }) => resolvesIntoRoot(file, value, 'app'))
        .map(({ line, value }) => `${repositoryPath(file)}:${line} -> ${value}`)
    })
}

function frontendSharedToInnerImports() {
  return sourceFiles('src/shared', new Set(['.ts', '.vue']))
    .filter(file => !/\.(?:test|spec)\.ts$/.test(file))
    .flatMap((file) => {
      const source = readFileSync(file, 'utf8')
      return importSpecifiers(source)
        .filter(({ value }) =>
          resolvesIntoRoot(file, value, 'app')
          || resolvesIntoRoot(file, value, 'modules'),
        )
        .map(({ line, value }) => `${repositoryPath(file)}:${line} -> ${value}`)
    })
}

function appFeatureInternalImports() {
  return sourceFiles('src/app', new Set(['.ts', '.vue']))
    .filter(file => !/\.(?:test|spec)\.ts$/.test(file))
    .flatMap((file) => {
      const source = readFileSync(file, 'utf8')
      return importSpecifiers(source)
        .filter(({ value }) => resolvesIntoRoot(file, value, 'modules'))
        .filter(({ value }) => !/^@\/modules\/[^/]+$/.test(value))
        .map(({ line, value }) => `${repositoryPath(file)}:${line} -> ${value}`)
    })
}

function rustUseStatements(source: string) {
  return source.match(/\buse\s+crate(?:::|\s*::\s*\{)[\s\S]*?;/g) ?? []
}

function rustProductionSource(source: string) {
  return source.replace(/#\[cfg\(test\)\][\s\S]*$/, '')
}

function hasRustDependency(source: string, layer: string) {
  if (source.includes(`crate::${layer}::`)) return true
  return rustUseStatements(source).some(statement =>
    new RegExp(`(?:^|[,{\\s])${layer}\\s*::`).test(statement),
  )
}

function rustLayerViolations(
  root: string,
  forbiddenLayers: readonly string[],
  excludedFiles: readonly string[] = [],
) {
  return sourceFiles(root, new Set(['.rs']))
    .filter(file => !file.endsWith('_tests.rs'))
    .filter(file => !excludedFiles.includes(repositoryPath(file).split('/').at(-1)!))
    .flatMap((file) => {
      const source = rustProductionSource(readFileSync(file, 'utf8'))
      return forbiddenLayers
        .filter(layer => hasRustDependency(source, layer))
        .map(layer => `${repositoryPath(file)} -> crate::${layer}`)
    })
}

function rustModuleInfrastructureImports() {
  return sourceFiles('src-tauri/src/modules', new Set(['.rs']))
    .filter(file => !file.endsWith('_tests.rs'))
    .filter(file => hasRustDependency(
      rustProductionSource(readFileSync(file, 'utf8')),
      'infrastructure',
    ))
    .map(repositoryPath)
    .sort()
}

describe('architecture dependency direction', () => {
  it('keeps production frontend features independent from app implementations', () => {
    expect(frontendFeatureToAppImports()).toEqual([])
  })

  it('keeps shared frontend code independent from app and feature implementations', () => {
    expect(frontendSharedToInnerImports()).toEqual([])
  })

  it('requires app consumers to use feature public entrypoints', () => {
    expect(appFeatureInternalImports()).toEqual([])
  })

  it('keeps synchronization lifecycle workflow outside the App composition root', () => {
    const appSource = readFileSync(resolve(repositoryRoot, 'src/app/App.vue'), 'utf8')
    const lifecycleSource = readFileSync(
      resolve(repositoryRoot, 'src/app/composables/useApplicationSyncLifecycle.ts'),
      'utf8',
    )

    expect(appSource).toContain('useApplicationSyncLifecycle')
    for (const token of [
      'function restoreCloudAndSync',
      'function runCloudRestoreAndSync',
      'function performSync',
      "window.addEventListener('online'",
      "document.addEventListener('visibilitychange'",
      'automaticSyncCooldownMs',
    ]) {
      expect(appSource).not.toContain(token)
    }
    for (const token of [
      'function restoreCloudAndSync',
      'runCloudRestoreAndSync',
      'performSync',
      'createSyncController',
      "window.addEventListener('online'",
      "document.addEventListener('visibilitychange'",
      '15_000',
    ]) {
      expect(lifecycleSource).toContain(token)
    }
  })

  it('keeps library recovery workflow outside the App composition root', () => {
    const appSource = readFileSync(resolve(repositoryRoot, 'src/app/App.vue'), 'utf8')
    const controllerSource = readFileSync(
      resolve(repositoryRoot, 'src/app/composables/useLibraryRecoveryController.ts'),
      'utf8',
    )

    expect(appSource).toContain('useLibraryRecoveryController')
    for (const token of [
      'createRecoverySingleFlight',
      'function runLibraryRecovery',
      'function reconnectLibrary',
      'function prepareRecoveryBackup',
      'function confirmRecoveryBackup',
      'function confirmFreshStart',
    ]) {
      expect(appSource).not.toContain(token)
    }
    for (const token of [
      'createRecoverySingleFlight',
      'function runRecovery',
      'function reconnectLibrary',
      'function prepareRecoveryBackup',
      'function confirmRecoveryBackup',
      'function confirmFreshStart',
      '恢复操作没有完成，原资料库状态没有被覆盖，请稍后重试。',
      'readonly(busy)',
      'readonly(message)',
      'readonly(candidate)',
      'readonly(restoreDialogOpen)',
      'readonly(freshStartDialogOpen)',
    ]) {
      expect(controllerSource).toContain(token)
    }
  })

  it('keeps profile and workspace policy outside the App composition root', () => {
    const appSource = readFileSync(resolve(repositoryRoot, 'src/app/App.vue'), 'utf8')
    const controllerSource = readFileSync(
      resolve(repositoryRoot, 'src/app/composables/useProfileManagement.ts'),
      'utf8',
    )

    expect(appSource).toContain('useProfileManagement')
    for (const token of [
      'const previewProfile',
      'const shellProfiles = computed',
      'const shellActiveProfileId = computed',
      'mutateProfile',
      'function createProfile',
      'function renameProfile',
      'function deleteProfile',
      'function selectProfile',
      'workspaceTransitionGuard.attempt()',
    ]) {
      expect(appSource).not.toContain(token)
    }
    for (const token of [
      'const previewProfile',
      'const shellProfiles = computed',
      'const shellActiveProfileId = computed',
      'function createProfile',
      'function renameProfile',
      'function deleteProfile',
      'function selectProfile',
      'attemptWorkspaceTransition',
      'const deletesActiveProfile',
      '{ refreshWorkspace: true, scheduleSync: true }',
      '{ refreshWorkspace: false, scheduleSync: true }',
      '{ refreshWorkspace: true, scheduleSync: false }',
      'profiles: readonly(profiles)',
      'activeProfileId: readonly(activeProfileId)',
    ]) {
      expect(controllerSource).toContain(token)
    }
  })

  it('keeps settings storage lifecycle outside the Settings composition view', () => {
    const settingsSource = readFileSync(
      resolve(repositoryRoot, 'src/app/views/SettingsView.vue'),
      'utf8',
    )
    const controllerSource = readFileSync(
      resolve(repositoryRoot, 'src/app/composables/useSettingsStorageLifecycle.ts'),
      'utf8',
    )

    expect(settingsSource).toContain('useSettingsStorageLifecycle')
    for (const token of [
      'const storageStatus = ref',
      'const storageMigrationReceipt = ref',
      'const storageDialogOpen = ref',
      'const storageMigrating = ref',
      'const storageMigrationError = ref',
      'const storageReceiptCopy = computed',
      'async function loadStorageStatus',
      'async function loadStorageMigrationReceipt',
      'function openStorageMigration',
      'async function closeStorageMigration',
      'async function confirmStorageMigration',
    ]) {
      expect(settingsSource).not.toContain(token)
    }
    for (const token of [
      'const status = ref',
      'const receipt = ref',
      'const dialogOpen = ref',
      'const busy = ref',
      'const receiptCopy = computed',
      'async function loadStatus',
      'async function loadReceipt',
      'function openMigration',
      'async function closeMigration',
      'async function confirmMigration',
      "value.outcome === 'moved'",
      "value.outcome === 'cleanup_required'",
      "value.outcome === 'rolled_back'",
      '迁移没有开始或没有完成，原资料库保持不变，请检查目标磁盘后重试。',
      'options.enterRestarting()',
      'options.restoreMigrationFocus()',
      'status: readonly(status)',
      'dialogOpen: readonly(dialogOpen)',
      'busy: readonly(busy)',
    ]) {
      expect(controllerSource).toContain(token)
    }
  })

  it('keeps settings backup policy outside the Settings composition view', () => {
    const settingsSource = readFileSync(
      resolve(repositoryRoot, 'src/app/views/SettingsView.vue'),
      'utf8',
    )
    const controllerSource = readFileSync(
      resolve(repositoryRoot, 'src/app/composables/useSettingsBackupOperations.ts'),
      'utf8',
    )

    for (const token of [
      'const restoreDialogOpen = ref',
      'const automaticBackupStatus = ref',
      'const automaticBackupBusy = ref',
      'async function createBackup',
      'async function createPortableBackup',
      'async function prepareRestore',
      'async function preparePortableRestore',
      'async function loadAutomaticBackupStatus',
      'async function configureAutomaticBackup',
      'async function disableAutomaticBackup',
      'function openRestoreDialog',
      'async function confirmRestore',
      'async function closeRestoreDialog',
    ]) {
      expect(settingsSource).not.toContain(token)
    }
    for (const token of [
      "| 'automatic'",
      'const automaticStatus = ref',
      'const restoreDialogOpen = ref',
      'const navigationBusy = busy',
      'async function createBackup',
      'async function prepareRestore',
      'async function restoreBackup',
      'async function loadAutomaticStatus',
      'async function configureAutomaticBackup',
      'async function disableAutomaticBackup',
      'function openRestoreDialog',
      'async function closeRestoreDialog',
      'async function confirmRestore',
      '自动备份设置没有更新；现有备份和资料库保持不变。',
      '自动备份没有停用；请稍后重试。',
      'options.restoreFocus()',
      'phase: readonly(phase)',
      'automaticStatus: readonly(automaticStatus)',
      'restoreDialogOpen: readonly(restoreDialogOpen)',
    ]) {
      expect(controllerSource).toContain(token)
    }
  })

  it('keeps explicit Windows update workflow outside the Settings composition view', () => {
    const settingsSource = readFileSync(
      resolve(repositoryRoot, 'src/app/views/SettingsView.vue'),
      'utf8',
    )
    const controllerSource = readFileSync(
      resolve(repositoryRoot, 'src/app/composables/useSettingsWindowsUpdate.ts'),
      'utf8',
    )

    expect(settingsSource).toContain('useSettingsWindowsUpdate')
    for (const token of [
      'const windowsCompatibility = ref',
      'const windowsUpdateStatus = ref',
      'const windowsUpdateReport = ref',
      'const checkingWindowsUpdate = ref',
      'const installingWindowsUpdate = ref',
      'const windowsUpdateMessage = ref',
      'async function loadWindowsCompatibility',
      'async function loadWindowsUpdateStatus',
      'async function checkWindowsUpdate',
      'async function installWindowsUpdate',
      'function formatUpdatePublication',
    ]) {
      expect(settingsSource).not.toContain(token)
    }
    for (const token of [
      'const compatibility = ref',
      'const status = ref',
      'const report = ref',
      'const checking = ref',
      'const installing = ref',
      'const message = ref',
      'let checkTask:',
      'let installTask:',
      'async function loadCompatibility',
      'async function loadStatus',
      'function check()',
      'function install()',
      'options.operations.install(version)',
      'report.value = undefined',
      'options.restoreFocus()',
      'compatibility: readonly(compatibility)',
      'report: readonly(report)',
      'checking: readonly(checking)',
      'installing: readonly(installing)',
    ]) {
      expect(controllerSource).toContain(token)
    }
  })

  it('keeps diagnostics export workflow outside the Settings composition view', () => {
    const settingsSource = readFileSync(
      resolve(repositoryRoot, 'src/app/views/SettingsView.vue'),
      'utf8',
    )
    const controllerSource = readFileSync(
      resolve(repositoryRoot, 'src/app/composables/useSettingsDiagnosticsExport.ts'),
      'utf8',
    )

    expect(settingsSource).toContain('useSettingsDiagnosticsExport')
    for (const token of [
      'const diagnosticsReceipt = ref',
      'const exportingDiagnostics = ref',
      'const diagnosticsMessage = ref',
      'async function exportDiagnostics',
    ]) {
      expect(settingsSource).not.toContain(token)
    }
    for (const token of [
      'const receipt = ref',
      'const busy = ref',
      'const message = ref',
      'let activeTask:',
      'function exportDiagnostics()',
      'receipt.value = undefined',
      'options.restoreFocus()',
      '诊断报告没有生成，现有资料不会受到影响；请检查磁盘空间和保存位置后重试。',
      'receipt: readonly(receipt)',
      'busy: readonly(busy)',
      'message: readonly(message)',
    ]) {
      expect(controllerSource).toContain(token)
    }
  })

  it('keeps device access and lock policy outside the Settings composition view', () => {
    const settingsSource = readFileSync(
      resolve(repositoryRoot, 'src/app/views/SettingsView.vue'),
      'utf8',
    )
    const controllerSource = readFileSync(
      resolve(repositoryRoot, 'src/app/composables/useSettingsCloudSession.ts'),
      'utf8',
    )

    for (const token of [
      'const deviceAccessStatus = ref',
      'const deviceAccessError = ref',
      'async function loadDeviceAccessStatus',
      'async function closeLibraryLock',
      'closeCloudLibraryLock',
    ]) {
      expect(settingsSource).not.toContain(token)
    }
    for (const token of [
      'const deviceAccessStatus = ref',
      'const deviceAccessError = ref',
      'async function loadDeviceAccessStatus',
      'async function closeLibraryLock',
      'options.restoreLockFocus(closedMode)',
      "lockDialogMode.value === 'sign-out'",
      'await disconnectCloud()',
      'await options.operations.lockLibrary()',
      'deviceAccessStatus: readonly(deviceAccessStatus)',
      'deviceAccessError: readonly(deviceAccessError)',
    ]) {
      expect(controllerSource).toContain(token)
    }
  })

  it('keeps crop presentation workflow outside the Capture composition view', () => {
    const captureSource = readFileSync(
      resolve(repositoryRoot, 'src/app/views/CaptureView.vue'),
      'utf8',
    )
    const controllerSource = readFileSync(
      resolve(repositoryRoot, 'src/modules/capture/composables/useCaptureCropPresentation.ts'),
      'utf8',
    )

    expect(captureSource).toContain('useCaptureCropPresentation')
    for (const token of [
      'createModalReturnFocusController',
      'function cropLauncherFor',
      'function cropResultControlFor',
      'function recognitionEditControlFor',
      'const cropReturnFocus',
      'const recognitionCropReturnFocus',
      'const developmentCropEditor = ref',
      'async function openVisibleCropEditor',
      'async function closeVisibleCropEditor',
      'async function applyVisibleCrop',
      'async function openRecognitionCropEditor',
      'async function closeRecognitionCropEditor',
      'async function saveRecognitionCropEditor',
    ]) {
      expect(captureSource).not.toContain(token)
    }
    for (const token of [
      'createModalReturnFocusController',
      "enabledButton('crop-item-id'",
      "enabledButton('crop-result-item-id'",
      "enabledButton('recognition-edit-suggestion-id'",
      'const developmentCropEditor = ref',
      'async function openVisibleCropEditor',
      'async function closeVisibleCropEditor',
      'async function applyVisibleCrop',
      'async function openRecognitionCropEditor',
      'async function closeRecognitionCropEditor',
      'async function saveRecognitionCropEditor',
      'function clearPendingFocus',
      'developmentCropEditor: readonly(developmentCropEditor)',
    ]) {
      expect(controllerSource).toContain(token)
    }
  })

  it('keeps quality analysis policy outside the Capture composition view', () => {
    const captureSource = readFileSync(
      resolve(repositoryRoot, 'src/app/views/CaptureView.vue'),
      'utf8',
    )
    const controllerSource = readFileSync(
      resolve(repositoryRoot, 'src/modules/capture/composables/useCaptureQualityAnalysis.ts'),
      'utf8',
    )

    expect(captureSource).toContain('useCaptureQualityAnalysis')
    for (const token of [
      'const qualityReports = ref',
      'const qualityErrors = ref',
      'const qualityCheckingItemId = ref',
      'const qualityDismissedItemIds = ref',
      'function qualityCropSeed',
      'async function checkCaptureQuality',
      'function dismissCaptureQuality',
      "'图片仍可继续使用，也可以稍后重新检查。'",
    ]) {
      expect(captureSource).not.toContain(token)
    }
    for (const token of [
      'const reports = ref',
      'const errors = ref',
      'const checkingItemId = ref',
      'const dismissedItemIds = ref',
      'watch(options.activeBatchId, reset',
      'function cropSeed',
      'function check(itemId: string)',
      'function dismiss(itemId: string)',
      'generation !== requestGeneration',
      'reports: shallowReadonly(reports)',
    ]) {
      expect(controllerSource).toContain(token)
    }
  })

  it('keeps batch data ownership outside the Capture composition view', () => {
    const captureSource = readFileSync(
      resolve(repositoryRoot, 'src/app/views/CaptureView.vue'),
      'utf8',
    )
    const controllerSource = readFileSync(
      resolve(repositoryRoot, 'src/modules/capture/composables/useCaptureBatchData.ts'),
      'utf8',
    )

    expect(captureSource).toContain('useCaptureBatchData')
    for (const token of [
      'const batches = ref',
      'const detail = ref<CaptureBatchDetail>',
      'let requestedDetailBatchId',
      'async function loadBatches',
      'async function loadDetail',
      "'采集箱连接中断，请重新打开应用后重试。'",
      "'没有读取到这个采集批次，请返回后重试。'",
    ]) {
      expect(captureSource).not.toContain(token)
    }
    for (const token of [
      'const batches = ref',
      'const detail = ref<CaptureBatchDetail>',
      "const requestedBatchId = ref('')",
      'function setDetailRequestedHandler',
      'async function loadBatches',
      'async function loadDetail',
      'requestedBatchId.value !== batchId',
      'function replaceDetail',
      'function clearDetail',
      'function hydrateDevelopment',
      'batches: shallowReadonly(batches)',
      'detail: shallowReadonly(detail)',
    ]) {
      expect(controllerSource).toContain(token)
    }
  })

  it('keeps pair-suggestion organizer policy outside the Capture composition view', () => {
    const captureSource = readFileSync(
      resolve(repositoryRoot, 'src/app/views/CaptureView.vue'),
      'utf8',
    )
    const organizerSource = readFileSync(
      resolve(repositoryRoot, 'src/modules/capture/composables/useCaptureOrganizerActions.ts'),
      'utf8',
    )

    expect(captureSource).toContain('applyPairSuggestions: async input =>')
    for (const token of [
      'async function applyPairSuggestions',
      "result.error.code === 'capture_input_invalid'",
      "'题答匹配没有应用；素材牌库和现有题卡保持不变。'",
      "'这组题答素材刚刚被移动、改角色或已加入其他题卡，已刷新并保留你的现有整理。'",
    ]) {
      expect(captureSource).not.toContain(token)
    }
    for (const token of [
      'applyPairSuggestions: (pairIds: string[])',
      'options.operations.applyPairSuggestions',
      "reloadOnErrorCodes: ['capture_revision_conflict', 'capture_input_invalid']",
      "error.code === 'capture_input_invalid'",
      'options.onNotice(',
    ]) {
      expect(organizerSource).toContain(token)
    }
  })

  it('keeps complete draft persistence policy outside the Capture composition view', () => {
    const captureSource = readFileSync(
      resolve(repositoryRoot, 'src/app/views/CaptureView.vue'),
      'utf8',
    )
    const controllerSource = readFileSync(
      resolve(repositoryRoot, 'src/modules/capture/composables/useCaptureDraftPersistence.ts'),
      'utf8',
    )

    expect(captureSource).toContain('useCaptureDraftPersistence')
    for (const token of [
      'useCaptureDraftSaveQueue',
      'const draftSaveQueueState = ref',
      'async function persistDraftUpdate',
      'async function retryDraftSave',
      'function updateDraft',
      "result.error.code === 'capture_revision_conflict'",
      "'当前采集批次已经切换，本次草稿没有保存。'",
      "'草稿文字保存没有完成；本次编辑仍保留在当前输入框中，请再次修改或重试。'",
    ]) {
      expect(captureSource).not.toContain(token)
    }
    for (const token of [
      'useCaptureDraftSaveQueue',
      'const state = ref<CaptureDraftSaveQueueState>',
      'async function perform',
      "result.error.code === 'capture_revision_conflict'",
      'function updateDraft',
      'async function retry()',
      'watch(options.isBlocked',
      'queue.retainBatch(batchId)',
      'unsaved,',
      'retryAvailable,',
      'persistenceBusy,',
    ]) {
      expect(controllerSource).toContain(token)
    }
  })

  it('keeps Rust domain code independent from outer layers', () => {
    expect(rustLayerViolations('src-tauri/src/domain', [
      'application',
      'commands',
      'infrastructure',
      'modules',
    ])).toEqual([])
  })

  it('keeps Rust application policy independent from implementations', () => {
    expect(rustLayerViolations('src-tauri/src/application', [
      'commands',
      'infrastructure',
      'modules',
    ], ['startup.rs'])).toEqual([])
  })

  it('ratchets remaining Rust module-to-infrastructure dependencies', () => {
    expect(rustModuleInfrastructureImports()).toEqual([
      'src-tauri/src/modules/auth_sync.rs',
      'src-tauri/src/modules/backup_creation.rs',
      'src-tauri/src/modules/backup_portability.rs',
      'src-tauri/src/modules/backup_validation.rs',
      'src-tauri/src/modules/product_check.rs',
      'src-tauri/src/modules/storage_migration.rs',
      'src-tauri/src/modules/storage_migration_snapshot.rs',
    ])
  })
})
