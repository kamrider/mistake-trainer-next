$ErrorActionPreference = 'Stop'
Set-StrictMode -Version Latest

$repositoryRoot = Split-Path -Parent $PSScriptRoot

function Read-RepositoryFile([string]$relativePath) {
    $path = Join-Path $repositoryRoot $relativePath
    if (-not (Test-Path -LiteralPath $path -PathType Leaf)) {
        throw "Required architecture file is missing: $relativePath"
    }
    return [System.IO.File]::ReadAllText($path)
}

function Require-Pattern(
    [string]$relativePath,
    [string]$pattern,
    [string]$message
) {
    $content = Read-RepositoryFile $relativePath
    if ($content -notmatch $pattern) {
        throw "$message ($relativePath)"
    }
}

function Reject-Pattern(
    [string]$relativePath,
    [string]$pattern,
    [string]$message
) {
    $content = Read-RepositoryFile $relativePath
    if ($content -match $pattern) {
        throw "$message ($relativePath)"
    }
}

Require-Pattern 'src-tauri/src/application/ports/sync.rs' 'pub trait CloudPushTransport' `
    'Cloud push must remain an application port'
Require-Pattern 'src-tauri/src/application/ports/sync.rs' 'pub trait CloudPullTransport' `
    'Cloud pull must remain an application port'
Require-Pattern 'src-tauri/src/application/mod.rs' '(?m)^pub mod ports;' `
    'Application ports must remain reachable from the application boundary'
Require-Pattern 'src-tauri/src/application/ports/mod.rs' '(?m)^pub mod sync;' `
    'Sync ports must remain reachable from the application port boundary'
Require-Pattern 'src-tauri/src/infrastructure/supabase.rs' `
    'pub use crate::application::ports::sync::\{' `
    'The Supabase adapter must preserve its compatibility re-export'
foreach ($contract in @(
    'CloudError',
    'CloudPullTransport',
    'CloudPushTransport',
    'DownloadedRemoteAsset',
    'ObjectUploadResult',
    'PushAcknowledgement',
    'RemoteObjectMetadata',
    'RemotePullChange'
)) {
    Require-Pattern 'src-tauri/src/infrastructure/supabase.rs' "pub use crate::application::ports::sync::\{[^}]*\b$contract\b[^}]*\};" `
        "The Supabase adapter must re-export $contract for compatibility"
    Reject-Pattern 'src-tauri/src/infrastructure/supabase.rs' `
        "(?m)^pub (?:enum|struct|trait) $contract\b" `
        "Cloud transport contract $contract must not be duplicated inside the Supabase adapter"
}
Reject-Pattern 'src-tauri/src/modules/sync_push.rs' 'infrastructure::supabase' `
    'Sync push must not depend on the Supabase adapter'
Reject-Pattern 'src-tauri/src/modules/sync_pull.rs' 'infrastructure::supabase' `
    'Sync pull must not depend on the Supabase adapter'
Require-Pattern 'src-tauri/src/modules/sync_push.rs' `
    'application::ports::sync::\{[^}]*\bCloudPushTransport\b' `
    'Sync push must depend positively on the application cloud push port'
Require-Pattern 'src-tauri/src/modules/sync_pull.rs' `
    'application::ports::sync::\{[^}]*\bCloudPullTransport\b' `
    'Sync pull must depend positively on the application cloud pull port'
Require-Pattern 'src-tauri/src/modules/sync_pull_decoder.rs' '(?m)^pub\(super\) fn validate_page\b' `
    'Remote page validation must remain in the sync pull decoder'
Require-Pattern 'src-tauri/src/modules/sync_pull_decoder.rs' '(?m)^pub\(super\) fn decode_page\b' `
    'Remote payload decoding must remain in the sync pull decoder'
Reject-Pattern 'src-tauri/src/modules/sync_pull.rs' '(?m)^fn validate_page' `
    'Remote page validation must not move back into sync pull orchestration'
Reject-Pattern 'src-tauri/src/modules/sync_pull.rs' '(?m)^fn decode_page' `
    'Remote payload decoding must not move back into sync pull orchestration'
Require-Pattern 'src-tauri/src/modules/sync_pull_transaction.rs' `
    '(?m)^pub\(super\) fn apply_page' `
    'Sync pull page application must remain in the private transaction child'
Require-Pattern 'src-tauri/src/modules/sync_pull.rs' `
    '(?m)^pub async fn pull_until_current' `
    'Sync pull remote paging must remain in the facade'
Require-Pattern 'src-tauri/src/modules/sync_pull.rs' `
    '(?m)^async fn stage_remote_asset' `
    'Sync pull remote asset staging orchestration must remain in the facade'
Require-Pattern 'src-tauri/src/modules/sync_pull.rs' `
    '(?m)^fn validate_download' `
    'Sync pull download validation must remain in the facade'
Reject-Pattern 'src-tauri/src/modules/sync_pull_transaction.rs' `
    'CloudPullTransport|download_object' `
    'Sync pull page transactions must not own cloud transport'

$syncPullTransactionFunctions = @(
    'apply_page',
    'apply_profile_merge',
    'apply_problem_merge',
    'apply_export_merge',
    'apply_tombstone_merge',
    'upsert_asset'
)
foreach ($function in $syncPullTransactionFunctions) {
    Require-Pattern 'src-tauri/src/modules/sync_pull_transaction.rs' `
        "(?m)^(?:pub\(super\) )?fn $function" `
        "Sync pull function $function must remain in the page transaction child"
    Reject-Pattern 'src-tauri/src/modules/sync_pull.rs' `
        "(?m)^fn $function" `
        "Sync pull function $function must not move back into remote orchestration"
}

Require-Pattern 'src-tauri/src/modules/capture_asset_repository.rs' `
    '(?m)^pub\(crate\) fn read_encrypted_blob\b' `
    'Capture blob reads must remain in the asset repository'
Require-Pattern 'src-tauri/src/modules/mod.rs' '(?m)^pub\(crate\) mod capture_asset_repository;' `
    'Capture asset repository must remain available to internal infrastructure callers'
foreach ($helper in @(
    'image_format_for_media_type',
    'read_encrypted_blob',
    'remove_encrypted_blob',
    'validate_relative_asset_path'
)) {
    Require-Pattern 'src-tauri/src/modules/capture_inbox.rs' `
        "pub\(crate\) use capture_asset_repository::\{[^}]*\b$helper\b[^}]*\};" `
        "Capture inbox must preserve the $helper compatibility re-export"
}
Reject-Pattern 'src-tauri/src/modules/capture_inbox.rs' 'fn validate_relative_asset_path' `
    'Capture path validation must not move back into the use-case module'
Reject-Pattern 'src-tauri/src/modules/capture_inbox.rs' 'File::open\(blob_root\.join' `
    'Capture blob I/O must not move back into the use-case module'
Require-Pattern 'src-tauri/src/modules/capture_inbox_repository.rs' '(?m)^pub fn get_capture_batch_detail' `
    'Capture batch detail reads must remain in the inbox read repository'
Require-Pattern 'src-tauri/src/modules/capture_inbox_repository.rs' '(?m)^pub fn list_capture_batches' `
    'Capture batch list reads must remain in the inbox read repository'
Require-Pattern 'src-tauri/src/modules/capture_inbox_repository.rs' '(?m)^pub fn query_batch' `
    'Capture batch summary reads must remain in the inbox read repository'
Require-Pattern 'src-tauri/src/modules/capture_inbox_repository.rs' '(?m)^pub fn get_capture_item' `
    'Capture item reads must remain in the inbox read repository'
Reject-Pattern 'src-tauri/src/modules/capture_inbox.rs' '(?m)^pub fn get_capture_batch_detail' `
    'Capture batch detail SQL must not move back into inbox orchestration'
Reject-Pattern 'src-tauri/src/modules/capture_inbox.rs' '(?m)^pub fn list_capture_batches' `
    'Capture batch list SQL must not move back into inbox orchestration'
Reject-Pattern 'src-tauri/src/modules/capture_inbox.rs' '(?m)^fn query_batch\(' `
    'Capture batch summary SQL must not move back into inbox orchestration'
Reject-Pattern 'src-tauri/src/modules/capture_inbox.rs' '(?m)^fn get_capture_item\(' `
    'Capture item SQL must not move back into inbox orchestration'
Require-Pattern 'src-tauri/src/modules/capture_crop.rs' '(?m)^pub fn get_capture_item_preview' `
    'Capture preview reads must remain in the crop transaction module'
Require-Pattern 'src-tauri/src/modules/capture_crop.rs' '(?m)^pub fn get_capture_crop_source_preview' `
    'Capture crop source preview reads must remain in the crop transaction module'
Require-Pattern 'src-tauri/src/modules/capture_crop.rs' '(?m)^pub\(crate\) fn encode_crop' `
    'Capture crop encoding must remain in the crop transaction module'
Require-Pattern 'src-tauri/src/modules/capture_crop.rs' '(?m)^pub fn apply_capture_crop' `
    'Capture crop apply must remain in the crop transaction module'
Require-Pattern 'src-tauri/src/modules/capture_crop.rs' '(?m)^pub fn revert_capture_crop' `
    'Capture crop revert must remain in the crop transaction module'
Require-Pattern 'src-tauri/src/modules/capture_crop.rs' '(?m)^fn ensure_crop_revision' `
    'Capture crop state and revision validation must remain in the crop transaction module'
Reject-Pattern 'src-tauri/src/modules/capture_inbox.rs' '(?m)^pub fn get_capture_item_preview' `
    'Capture preview implementation must not move back into inbox orchestration'
Reject-Pattern 'src-tauri/src/modules/capture_inbox.rs' '(?m)^pub fn get_capture_crop_source_preview' `
    'Capture crop source preview must not move back into inbox orchestration'
Reject-Pattern 'src-tauri/src/modules/capture_inbox.rs' '(?m)^pub\(crate\) fn encode_crop' `
    'Capture crop encoding must not move back into inbox orchestration'
Reject-Pattern 'src-tauri/src/modules/capture_inbox.rs' '(?m)^pub fn apply_capture_crop' `
    'Capture crop apply must not move back into inbox orchestration'
Reject-Pattern 'src-tauri/src/modules/capture_inbox.rs' '(?m)^pub fn revert_capture_crop' `
    'Capture crop revert must not move back into inbox orchestration'
Reject-Pattern 'src-tauri/src/modules/capture_inbox.rs' '(?m)^fn ensure_crop_revision' `
    'Capture crop validation must not move back into inbox orchestration'
Require-Pattern 'src-tauri/src/modules/capture_commit.rs' '(?m)^pub fn commit_ready_capture_drafts' `
    'Capture draft commit transaction must remain in the commit module'
Require-Pattern 'src-tauri/src/modules/capture_commit.rs' '(?m)^fn query_ready_drafts' `
    'Ready draft selection must remain in the commit module'
Require-Pattern 'src-tauri/src/modules/capture_commit.rs' '(?m)^fn query_draft_asset_links' `
    'Committed asset ordering must remain in the commit module'
Require-Pattern 'src-tauri/src/modules/capture_commit.rs' '(?m)^fn query_asset_sync_payload' `
    'Committed asset sync payloads must remain in the commit module'
Reject-Pattern 'src-tauri/src/modules/capture_inbox.rs' '(?m)^pub fn commit_ready_capture_drafts' `
    'Capture commit implementation must not move back into inbox orchestration'
Require-Pattern 'src-tauri/src/modules/capture_inbox.rs' `
    '(?m)^pub\(super\) fn ensure_organizing_revision' `
    'Shared organizing state validation must remain restricted to inbox descendants'
Reject-Pattern 'src-tauri/src/modules/capture_inbox.rs' '(?m)^fn query_ready_drafts' `
    'Ready draft SQL must not move back into inbox orchestration'
Reject-Pattern 'src-tauri/src/modules/capture_inbox.rs' '(?m)^fn query_draft_asset_links' `
    'Committed asset SQL must not move back into inbox orchestration'
Reject-Pattern 'src-tauri/src/modules/capture_inbox.rs' '(?m)^fn query_asset_sync_payload' `
    'Committed asset sync serialization must not move back into inbox orchestration'

Require-Pattern 'src-tauri/src/modules/capture_organizer_transaction.rs' `
    '(?m)^pub fn apply_capture_layout' `
    'Capture layout transactions must remain in the organizer module'
Require-Pattern 'src-tauri/src/modules/capture_organizer_transaction.rs' `
    '(?m)^pub fn apply_capture_pair_suggestions' `
    'Capture pair application transactions must remain in the organizer module'
Require-Pattern 'src-tauri/src/modules/capture_organizer_transaction.rs' `
    '(?m)^pub fn remove_capture_item' `
    'Capture item removal transactions must remain in the organizer module'
Require-Pattern 'src-tauri/src/modules/capture_inbox_transaction_support.rs' `
    '(?m)^pub\(super\) fn invalidate_active_pairs_for_item' `
    'Shared capture pair invalidation must remain in transaction support'
Require-Pattern 'src-tauri/src/modules/capture_inbox_transaction_support.rs' `
    '(?m)^pub\(super\) fn delete_asset_row_if_orphan' `
    'Shared capture orphan cleanup must remain in transaction support'
Require-Pattern 'src-tauri/src/modules/capture_inbox_transaction_support.rs' `
    '(?m)^pub\(super\) fn repack_link_positions' `
    'Shared capture link compaction must remain in transaction support'
Require-Pattern 'src-tauri/src/modules/capture_inbox_transaction_support.rs' `
    '(?m)^pub\(super\) fn touch_batch' `
    'Shared capture revision updates must remain in transaction support'
Require-Pattern 'src-tauri/src/modules/capture_organizer_transaction.rs' `
    '(?m)^fn invalidate_active_pairs_for_batch' `
    'Batch-wide pair invalidation must remain in organizer transactions'

$captureOrganizerFunctions = @(
    'apply_capture_layout',
    'move_capture_item',
    'stage_capture_item_role',
    'merge_capture_card',
    'apply_capture_pair_suggestions',
    'delete_capture_draft',
    'update_capture_draft',
    'remove_capture_item',
    'discard_capture_batch'
)
foreach ($function in $captureOrganizerFunctions) {
    Require-Pattern 'src-tauri/src/modules/capture_organizer_transaction.rs' `
        "(?m)^pub fn $function" `
        "Capture organizer function $function must remain in the organizer transaction module"
    Reject-Pattern 'src-tauri/src/modules/capture_inbox.rs' `
        "(?m)^pub fn $function" `
        "Capture organizer function $function must not move back into the facade"
}

$captureOrganizerHelpers = @(
    'invalidate_active_pairs_for_item',
    'invalidate_active_pairs_for_batch',
    'touch_batch',
    'repack_link_positions',
    'delete_asset_row_if_orphan'
)
foreach ($function in $captureOrganizerHelpers) {
    Reject-Pattern 'src-tauri/src/modules/capture_inbox.rs' `
        "(?m)^fn $function" `
        "Capture organizer helper $function must not move back into the facade"
}

$recognitionJobFunctions = @(
    'create_or_resume_recognition_job',
    'get_active_recognition_job',
    'get_recognition_job_by_id',
    'store_recognition_suggestion',
    'review_recognition_suggestion',
    'cancel_recognition_job',
    'reset_abandoned_recognition_work',
    'claim_next_recognition_item',
    'finish_recognition_item_without_suggestion',
    'fail_recognition_job'
)
foreach ($function in $recognitionJobFunctions) {
    Require-Pattern 'src-tauri/src/modules/capture_recognition_job.rs' `
        "(?m)^pub fn $function" `
        "Recognition job lifecycle function $function must remain in the job module"
    Reject-Pattern 'src-tauri/src/modules/capture_recognition.rs' `
        "(?m)^pub fn $function" `
        "Recognition job lifecycle function $function must not move back into the recognition facade"
}
Require-Pattern 'src-tauri/src/modules/capture_recognition_job.rs' '(?m)^fn list_suggestions' `
    'Recognition suggestion reads must remain in the job lifecycle module'
Reject-Pattern 'src-tauri/src/modules/capture_recognition.rs' '(?m)^fn list_suggestions' `
    'Recognition suggestion SQL must not move back into the recognition facade'

Require-Pattern 'src-tauri/src/modules/capture_recognition_transaction.rs' `
    '(?m)^pub fn apply_capture_recognition' `
    'Recognition apply must remain in the recognition transaction module'
Require-Pattern 'src-tauri/src/modules/capture_recognition_revert.rs' `
    '(?m)^pub fn revert_capture_recognition' `
    'Recognition revert must remain in the dedicated revert module'
Require-Pattern 'src-tauri/src/modules/capture_recognition_revert.rs' `
    '(?m)^pub fn latest_capture_recognition_operation' `
    'Recognition operation reads must remain in the dedicated revert module'
Require-Pattern 'src-tauri/src/modules/capture_recognition_revert.rs' `
    '(?m)^fn validate_recognition_revert_state' `
    'Recognition revert validation must remain in the dedicated revert module'
Require-Pattern 'src-tauri/src/modules/capture_recognition_transaction.rs' `
    '(?m)^fn mark_recognition_suggestions_stale' `
    'Recognition stale persistence must remain in the recognition transaction module'
Require-Pattern 'src-tauri/src/modules/capture_recognition_transaction.rs' `
    '(?m)^fn cleanup_staged_recognition_assets' `
    'Recognition staged asset cleanup must remain in the recognition transaction module'
Reject-Pattern 'src-tauri/src/modules/capture_recognition_transaction.rs' `
    '(?m)^(?:pub )?fn (?:revert_capture_recognition|latest_capture_recognition_operation|validate_recognition_revert_state)' `
    'Recognition revert and operation reads must not move back into apply transactions'
Reject-Pattern 'src-tauri/src/modules/capture_recognition_revert.rs' `
    '(?m)^(?:pub )?fn (?:apply_capture_recognition|mark_recognition_suggestions_stale|cleanup_staged_recognition_assets)' `
    'Recognition apply and staging cleanup must not move into revert ownership'
$recognitionLedgerTypes = @(
    'RecognitionOperationLedger',
    'RecognitionLedgerSource',
    'RecognitionLedgerItem',
    'RecognitionLedgerDraft'
)
foreach ($type in $recognitionLedgerTypes) {
    Require-Pattern 'src-tauri/src/modules/capture_recognition_operation_ledger.rs' `
        "(?s)#\[serde\(rename_all = `"camelCase`"\)\]\s+pub\(super\) struct $type" `
        "Recognition ledger type $type must remain internal and camel-case serialized"
}
foreach ($field in @(
    'source_items: Vec<RecognitionLedgerSource>',
    'created_items: Vec<RecognitionLedgerItem>',
    'created_drafts: Vec<RecognitionLedgerDraft>',
    'superseded_by_derivation_id: String',
    'derivation_id: String',
    'source_sequence: i64',
    'draft_id: Option<String>',
    'role: Option<String>',
    'position: Option<i64>',
    'position: i64'
)) {
    Require-Pattern 'src-tauri/src/modules/capture_recognition_operation_ledger.rs' `
        "pub\(super\) $([regex]::Escape($field))" `
        "Recognition ledger field $field must preserve its serialized shape"
}
Reject-Pattern 'src-tauri/src/modules/capture_recognition_operation_ledger.rs' `
    '(?i)\b(?:select|insert|update|delete)\b|rusqlite|std::(?:fs|path)|\bFile\b|\bConnection\b' `
    'Recognition operation serialization must remain free of SQL and filesystem ownership'
Require-Pattern 'src-tauri/src/modules/capture_recognition.rs' `
    'pub use capture_recognition_transaction::apply_capture_recognition;' `
    'Recognition apply must remain publicly exposed through the stable facade'
Require-Pattern 'src-tauri/src/modules/capture_recognition.rs' `
    'pub use capture_recognition_revert::\{[^}]*latest_capture_recognition_operation[^}]*revert_capture_recognition[^}]*\};' `
    'Recognition revert and operation reads must remain publicly exposed through the stable facade'

$recognitionTransactionFunctions = @(
    'apply_capture_recognition',
    'revert_capture_recognition',
    'latest_capture_recognition_operation',
    'validate_recognition_revert_state',
    'mark_recognition_suggestions_stale',
    'cleanup_staged_recognition_assets'
)
foreach ($function in $recognitionTransactionFunctions) {
    Reject-Pattern 'src-tauri/src/modules/capture_recognition.rs' `
        "(?m)^(?:pub )?fn $function" `
        "Recognition transaction function $function must not move back into the facade"
}

Require-Pattern 'src-tauri/src/modules/backup_package_repository.rs' `
    '(?m)^pub\(super\) fn safe_relative_path\b' `
    'Backup path validation must remain in the package repository'
Require-Pattern 'src-tauri/src/modules/backup_package_repository.rs' `
    '(?m)^pub\(super\) fn copy_and_hash\b' `
    'Backup copy and integrity hashing must remain in the package repository'
Require-Pattern 'src-tauri/src/modules/backup_package_repository.rs' `
    '(?m)^pub\(super\) fn write_new_synced\b' `
    'Generic synchronized package writes must remain in the package repository'
Reject-Pattern 'src-tauri/src/modules/backup.rs' 'fn safe_relative_path' `
    'Backup path validation must not move back into orchestration'
Reject-Pattern 'src-tauri/src/modules/backup.rs' 'fn copy_and_hash' `
    'Backup filesystem copying must not move back into orchestration'
Require-Pattern 'src-tauri/src/modules/backup_restore_repository.rs' 'fn read_pending_marker' `
    'Pending restore marker reads must remain in the restore repository'
Require-Pattern 'src-tauri/src/modules/backup_restore_repository.rs' 'fn write_control_file' `
    'Restore control writes must remain in the restore repository'
Require-Pattern 'src-tauri/src/modules/backup_restore_repository.rs' 'fn ensure_owned_directory_if_present' `
    'Restore directory ownership checks must remain in the restore repository'
Require-Pattern 'src-tauri/src/modules/backup_restore_repository.rs' 'fn restore_directory_name' `
    'Restore directory naming must remain in the restore repository'
Require-Pattern 'src-tauri/src/modules/backup_restore_repository.rs' 'fn read_restore_receipt' `
    'Restore receipt reads must remain in the restore repository'
Require-Pattern 'src-tauri/src/modules/backup_restore_repository.rs' 'fn write_restore_candidate_metadata' `
    'Restore candidate metadata writes must remain in the restore repository'
Reject-Pattern 'src-tauri/src/modules/backup_restore_repository.rs' '(?m)^pub\(super\) fn write_new_synced' `
    'Generic package writes must not be owned by the restore repository'
Reject-Pattern 'src-tauri/src/modules/backup.rs' '(?m)^fn read_pending_marker' `
    'Pending restore marker reads must not move back into backup orchestration'
Reject-Pattern 'src-tauri/src/modules/backup.rs' '(?m)^fn write_control_file' `
    'Restore control writes must not move back into backup orchestration'
Reject-Pattern 'src-tauri/src/modules/backup.rs' '(?m)^fn ensure_owned_directory_if_present' `
    'Restore directory ownership checks must not move back into backup orchestration'
Reject-Pattern 'src-tauri/src/modules/backup.rs' '(?m)^fn restore_directory_name' `
    'Restore directory naming must not move back into backup orchestration'
Reject-Pattern 'src-tauri/src/modules/backup.rs' 'MAX_RESTORE_CONTROL_BYTES|RESTORE_RECEIPT_FILE' `
    'Restore receipt storage details must not leak into backup orchestration'

$backupRestoreFunctions = @(
    'prepare_backup_restore',
    'validate_restore_candidate',
    'schedule_backup_restore',
    'begin_pending_restore',
    'record_failed_restore',
    'take_restore_receipt'
)
foreach ($function in $backupRestoreFunctions) {
    Require-Pattern 'src-tauri/src/modules/backup_restore.rs' `
        "(?m)^pub fn $function" `
        "Backup restore function $function must remain in the restore lifecycle module"
    Reject-Pattern 'src-tauri/src/modules/backup.rs' `
        "(?m)^pub fn $function" `
        "Backup restore function $function must not move back into the backup facade"
}
Require-Pattern 'src-tauri/src/modules/backup_restore.rs' '(?m)^pub struct RestoreSwap' `
    'Backup restore swap ownership must remain in the restore lifecycle module'
Reject-Pattern 'src-tauri/src/modules/backup.rs' '(?m)^pub struct RestoreSwap' `
    'Backup restore swap ownership must not move back into the backup facade'

$backupRestoreRepositoryFunctions = @(
    'read_pending_marker',
    'write_control_file',
    'ensure_owned_directory_if_present',
    'restore_directory_name'
)
foreach ($function in $backupRestoreRepositoryFunctions) {
    Reject-Pattern 'src-tauri/src/modules/backup_restore.rs' `
        "(?m)^(?:pub(?:\(super\))? )?fn $function" `
        "Restore repository function $function must not move into lifecycle orchestration"
}

Require-Pattern 'src-tauri/src/modules/backup_schema_validation.rs' `
    '(?m)^pub\(super\) fn ensure_database_budget' `
    'Backup database size validation must remain in the schema validation module'
Require-Pattern 'src-tauri/src/modules/backup_schema_validation.rs' `
    '(?m)^pub\(super\) fn ensure_single_account' `
    'Backup account and schema integrity policy must remain in the schema validation module'
Require-Pattern 'src-tauri/src/modules/backup_schema_validation.rs' `
    '(?m)^fn table_columns_match' `
    'Backup table shape inspection must remain in the schema validation module'
Require-Pattern 'src-tauri/src/modules/backup_schema_validation.rs' `
    '(?m)^fn index_columns_match' `
    'Backup index shape inspection must remain in the schema validation module'

$backupSchemaDefinitions = @(
    'ensure_database_budget',
    'ensure_single_account',
    'table_exists',
    'column_exists',
    'table_columns_match',
    'index_columns_match'
)
foreach ($function in $backupSchemaDefinitions) {
    Reject-Pattern 'src-tauri/src/modules/backup.rs' `
        "(?m)^(?:pub(?:\(super\))? )?fn $function" `
        "Backup schema validation function $function must not move back into orchestration"
}

Require-Pattern 'src-tauri/src/modules/sync_conflict_merge.rs' 'fn merge_problem_versions' `
    'Problem three-way merge policy must remain in the pure conflict merge module'
Require-Pattern 'src-tauri/src/modules/sync_conflict_merge.rs' 'fn merge_profile_versions' `
    'Profile three-way merge policy must remain in the pure conflict merge module'
Require-Pattern 'src-tauri/src/modules/sync_conflict_merge.rs' 'fn merge_export_versions' `
    'Export three-way merge policy must remain in the pure conflict merge module'
Reject-Pattern 'src-tauri/src/modules/sync_conflicts.rs' '(?m)^fn merge_field' `
    'Field merge policy must not move back into SQLite conflict orchestration'
Reject-Pattern 'src-tauri/src/modules/sync_conflicts.rs' '(?m)^enum FieldDecision' `
    'Field merge decisions must not move back into SQLite conflict orchestration'

Require-Pattern 'src-tauri/src/modules/sync_conflict_resolution.rs' `
    '(?m)^pub fn resolve_sync_conflict_field' `
    'Field conflict resolution must remain in the resolution transaction module'
Require-Pattern 'src-tauri/src/modules/sync_conflict_resolution.rs' `
    '(?m)^pub fn resolve_sync_conflict_entity' `
    'Entity conflict resolution must remain in the resolution transaction module'
Require-Pattern 'src-tauri/src/modules/sync_conflict_resolution.rs' `
    '(?m)^fn resolve_rows' `
    'Conflict resolution orchestration must remain transaction-local'
Require-Pattern 'src-tauri/src/modules/sync_conflict_resolution.rs' `
    '(?m)^fn apply_remote_delete' `
    'Remote deletion application must remain in the resolution transaction module'
Require-Pattern 'src-tauri/src/modules/sync_conflict_resolution.rs' `
    '(?m)^fn finalize_resolved_entity' `
    'Conflict revision and outbox finalization must remain transaction-local'
Reject-Pattern 'src-tauri/src/modules/sync_conflicts.rs' `
    '(?m)^pub fn resolve_sync_conflict_(field|entity)' `
    'Sync conflict facade must not own resolution transaction bodies'
Reject-Pattern 'src-tauri/src/modules/sync_conflicts.rs' `
    '(?m)^fn (resolve_rows|apply_remote_delete|finalize_resolved_entity)' `
    'Sync conflict facade must not absorb resolution internals'
Require-Pattern 'src-tauri/src/modules/sync_conflicts.rs' `
    '(?m)^pub fn list_sync_conflicts' `
    'Conflict list reads must remain in the sync conflict facade'

Require-Pattern 'src-tauri/src/modules/capture_lan_api.rs' `
    '(?m)^pub\(super\) fn build_router' `
    'Capture LAN HTTP routing must remain in the LAN API module'
Require-Pattern 'src-tauri/src/modules/capture_lan_api.rs' `
    '(?m)^async fn upload_item' `
    'Capture LAN upload streaming must remain in the LAN API module'
Require-Pattern 'src-tauri/src/modules/capture_lan_api.rs' `
    '(?m)^fn authorize' `
    'Capture LAN request authorization must remain in the LAN API module'
Require-Pattern 'src-tauri/src/modules/capture_lan_api.rs' `
    '(?m)^async fn harden_api_response' `
    'Capture LAN authenticated responses must remain non-cacheable'

$captureLanApiDefinitions = @(
    'build_router',
    'upload_item',
    'authorize'
)
foreach ($function in $captureLanApiDefinitions) {
    Reject-Pattern 'src-tauri/src/modules/capture_lan.rs' `
        "(?m)^(?:pub(?:\(super\))? )?(?:async )?fn $function" `
        "Capture LAN API function $function must not move back into the LAN session module"
}
Reject-Pattern 'src-tauri/src/modules/capture_lan.rs' `
    '(?m)^struct ApiError' `
    'Capture LAN API errors must not move back into the LAN session module'

Require-Pattern 'src-tauri/src/modules/legacy_scan.rs' `
    '(?m)^pub fn build_legacy_import_plan' `
    'Legacy import plan construction must remain in the legacy scan module'
Require-Pattern 'src-tauri/src/modules/legacy_scan.rs' `
    '#\[path = "legacy_scan_filesystem.rs"\]' `
    'Legacy scan must keep its private filesystem child'
Require-Pattern 'src-tauri/src/modules/legacy_scan.rs' `
    'pub use legacy_scan_filesystem::legacy_tree_fingerprint;' `
    'Legacy tree fingerprinting must preserve its compatibility export'
Require-Pattern 'src-tauri/src/modules/legacy_scan.rs' `
    'pub\(super\) use legacy_scan_filesystem::\{[^}]*MAX_ASSET_BYTES[^}]*is_safe_relative_path[^}]*read_bounded[^}]*\};' `
    'Legacy scan must preserve filesystem helpers for sibling transactions'
Require-Pattern 'src-tauri/src/modules/legacy_scan.rs' `
    '(?m)^pub fn scan_legacy_storage' `
    'Legacy storage scanning must remain in the legacy scan module'
Require-Pattern 'src-tauri/src/modules/legacy_scan_filesystem.rs' `
    '(?m)^pub fn legacy_tree_fingerprint' `
    'Legacy tree fingerprinting must remain in the filesystem child'
Require-Pattern 'src-tauri/src/modules/legacy_scan_filesystem.rs' `
    '(?m)^fn collect_fingerprint_files' `
    'Legacy fingerprint traversal must remain in the filesystem child'
Require-Pattern 'src-tauri/src/modules/legacy_scan_filesystem.rs' `
    'const MAX_DIRECTORY_DEPTH: usize = 32;' `
    'Legacy fingerprint traversal must retain a nesting-depth budget'
Require-Pattern 'src-tauri/src/modules/legacy_scan_filesystem.rs' `
    'const MAX_FINGERPRINT_ENTRIES: usize = MAX_RECORDS \+ MAX_DIRECTORY_ENTRIES;' `
    'Legacy fingerprint traversal must retain a global entry budget'
Require-Pattern 'src-tauri/src/modules/legacy_scan_filesystem.rs' `
    '\.take\(remaining\.saturating_add\(1\)\)' `
    'Legacy fingerprint traversal must enforce its entry budget while reading directories'
Require-Pattern 'src-tauri/src/modules/legacy_scan_filesystem.rs' `
    'depth > MAX_DIRECTORY_DEPTH' `
    'Legacy fingerprint traversal must enforce its depth budget before recursing'
Require-Pattern 'src-tauri/src/modules/legacy_scan_filesystem.rs' `
    '(?m)^pub\(in crate::modules::legacy\) fn is_safe_relative_path' `
    'Legacy path safety validation must remain in the filesystem child'
Require-Pattern 'src-tauri/src/modules/legacy_scan_filesystem.rs' `
    '(?m)^pub\(in crate::modules::legacy\) fn read_bounded' `
    'Legacy bounded file reads must remain in the filesystem child'
Require-Pattern 'src-tauri/src/modules/legacy_scan_filesystem.rs' `
    '(?m)^pub\(in crate::modules::legacy\) const MAX_ASSET_BYTES: u64 = 64 \* 1024 \* 1024;' `
    'Legacy asset byte budget must remain visible only inside the legacy boundary'
Require-Pattern 'src-tauri/src/modules/legacy_scan_filesystem.rs' `
    '(?m)^pub\(super\) fn sha256_file' `
    'Legacy asset hashing must remain in the filesystem child'
Require-Pattern 'src-tauri/src/modules/legacy_scan_filesystem.rs' `
    '(?m)^pub\(in crate::modules::legacy\) enum BoundedReadError' `
    'Legacy bounded-read errors must remain in the filesystem child'
Reject-Pattern 'src-tauri/src/modules/legacy_scan.rs' `
    '(?m)^(?:pub(?:\(super\))? )?fn (?:legacy_tree_fingerprint|collect_fingerprint_files|is_safe_relative_path|read_bounded|sha256_file)' `
    'Legacy filesystem implementations must not move back into parsing and reporting'
Reject-Pattern 'src-tauri/src/modules/legacy_scan.rs' `
    '(?m)^pub(?:\(super\)|\(in crate::modules::legacy\)) enum BoundedReadError' `
    'Legacy bounded-read errors must not move back into parsing and reporting'
Reject-Pattern 'src-tauri/src/modules/legacy_scan_filesystem.rs' `
    '(?im)serde|serde_json|LegacyScanReport|LegacyIssue|OffsetDateTime|rusqlite|^\s*(?:INSERT\s+INTO|UPDATE\s+\S+\s+SET|DELETE\s+FROM)\b' `
    'Legacy filesystem ownership must remain free of parsing, reporting, time, and database behavior'

$legacyScanFunctions = @(
    'build_legacy_import_plan',
    'legacy_tree_fingerprint',
    'scan_legacy_storage'
)
foreach ($function in $legacyScanFunctions) {
    Reject-Pattern 'src-tauri/src/modules/legacy.rs' `
        "(?m)^pub fn $function" `
        "Legacy scan function $function must not move back into import orchestration"
}

$legacyScanTypes = @(
    'LegacyStore',
    'MemberSource'
)
foreach ($type in $legacyScanTypes) {
    Reject-Pattern 'src-tauri/src/modules/legacy.rs' `
        "(?m)^struct $type" `
        "Legacy scan type $type must not move back into import orchestration"
}
Reject-Pattern 'src-tauri/src/modules/legacy.rs' `
    '(?m)^enum BoundedReadError' `
    'Legacy bounded-read errors must not move back into import orchestration'

Require-Pattern 'src-tauri/src/modules/legacy.rs' `
    'pub use legacy_import_transaction::import_legacy_plan;' `
    'Legacy import must remain publicly exposed through the stable facade'
Require-Pattern 'src-tauri/src/modules/legacy.rs' `
    'pub use legacy_rollback_transaction::rollback_legacy_import;' `
    'Legacy rollback must remain publicly exposed through the stable facade'
Require-Pattern 'src-tauri/src/modules/legacy.rs' `
    '(?m)^pub struct LegacyImportManager' `
    'Legacy candidate lifecycle must remain in the facade'
Require-Pattern 'src-tauri/src/modules/legacy.rs' `
    '(?m)^pub fn list_legacy_imports' `
    'Legacy import history reads must remain in the facade'
Require-Pattern 'src-tauri/src/modules/legacy.rs' `
    '(?m)^pub enum LegacyImportError' `
    'Legacy mutation errors must remain stable in the facade'

$legacyImportTransactionFunctions = @(
    'import_legacy_plan',
    'persist_legacy_import',
    'insert_import_sync_operation',
    'record_import_entity',
    'unique_profile_name',
    'validate_import_image',
    'plaintext_digest',
    'cleanup_legacy_staging'
)
foreach ($function in $legacyImportTransactionFunctions) {
    Require-Pattern 'src-tauri/src/modules/legacy_import_transaction.rs' `
        "(?m)^(?:pub )?fn $function" `
        "Legacy import function $function must remain in the import transaction child"
    Reject-Pattern 'src-tauri/src/modules/legacy.rs' `
        "(?m)^(?:pub )?fn $function" `
        "Legacy import function $function must not move back into the facade"
}

$legacyRollbackTransactionFunctions = @(
    'rollback_legacy_import',
    'enqueue_legacy_rollback_deletion',
    'import_entity_ids',
    'restore_quarantined_assets'
)
foreach ($function in $legacyRollbackTransactionFunctions) {
    Require-Pattern 'src-tauri/src/modules/legacy_rollback_transaction.rs' `
        "(?m)^(?:pub )?fn $function" `
        "Legacy rollback function $function must remain in the rollback transaction child"
    Reject-Pattern 'src-tauri/src/modules/legacy.rs' `
        "(?m)^(?:pub )?fn $function" `
        "Legacy rollback function $function must not move back into the facade"
}

Reject-Pattern 'src-tauri/src/modules/legacy.rs' `
    'fs::rename|transaction\.commit|INSERT INTO|struct StagedLegacyAsset|struct RemovedLegacyEntity' `
    'Legacy mutation file and transaction implementation must not move back into the facade'
Reject-Pattern 'src-tauri/src/modules/legacy_import_transaction.rs' `
    'legacy-import-rollback|enqueue_legacy_rollback_deletion|restore_quarantined_assets' `
    'Legacy rollback implementation must not move into the import transaction child'
Reject-Pattern 'src-tauri/src/modules/legacy_rollback_transaction.rs' `
    'encrypt_asset|StagedLegacyAsset|LegacyImportPhase|persist_legacy_import' `
    'Legacy import implementation must not move into the rollback transaction child'

Write-Output 'Rust architecture boundary contract passed.'
