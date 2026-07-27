#[cfg(feature = "local-ocr-runtime")]
use mistake_trainer_next_lib::infrastructure::{
    recognition_anchor_layout::AnchorRecognitionEngine,
    recognition_ppocr_runtime::PpOcrLocalRuntime,
};
use mistake_trainer_next_lib::{
    infrastructure::{
        capture_recognition_worker::{
            CaptureRecognitionChanged, CaptureRecognitionEngine, CaptureRecognitionEventSink,
            CaptureRecognitionManager, CaptureRecognitionWorkerContext, RecognitionAnalysis,
            RecognitionEngineError,
        },
        database::{open_encrypted_database, run_migrations},
    },
    modules::{
        capture_inbox::{
            CaptureBatchState, CreateCaptureBatch, IngestCaptureItem, create_capture_batch,
            ingest_capture_item,
        },
        capture_recognition::{
            ApplyCaptureRecognition, CaptureRecognitionDecision, CaptureRecognitionError,
            CaptureRecognitionFailurePoint, CaptureRecognitionJobState,
            CaptureRecognitionReasonCode, CaptureRecognitionRegionProposal,
            CaptureRecognitionReviewBand, CaptureRecognitionRole,
            CaptureRecognitionSuggestionState, CreateCaptureRecognitionJob,
            RevertCaptureRecognition, ReviewCaptureRecognitionSuggestion,
            StoreCaptureRecognitionSuggestion, apply_capture_recognition, cancel_recognition_job,
            capture_item_snapshot_hash, create_or_resume_recognition_job,
            get_active_recognition_job, latest_capture_recognition_operation,
            reset_abandoned_recognition_work, revert_capture_recognition, review_band,
            review_recognition_suggestion, store_recognition_suggestion,
        },
        profiles::{CreateProfile, create_profile},
    },
};
use std::{
    collections::VecDeque,
    io::Cursor,
    path::Path,
    sync::{
        Arc, Mutex,
        atomic::{AtomicBool, AtomicUsize, Ordering},
    },
    time::Duration,
};
use tempfile::{TempDir, tempdir};

const ACCOUNT: &str = "recognition-account";
const DATABASE_KEY: &str = "recognition-test-key";
const ASSET_KEY: [u8; 32] = [37; 32];

struct TestLibrary {
    directory: TempDir,
    profile_id: String,
}

impl TestLibrary {
    fn new() -> Self {
        let directory = tempdir().expect("temp directory");
        let mut connection =
            open_encrypted_database(&directory.path().join("library.db"), DATABASE_KEY)
                .expect("open database");
        run_migrations(&mut connection).expect("migrate database");
        let profile = create_profile(
            &mut connection,
            CreateProfile {
                account_id: ACCOUNT.to_owned(),
                name: "student".to_owned(),
                now_utc_ms: 1,
            },
        )
        .expect("create profile");
        Self {
            directory,
            profile_id: profile.id,
        }
    }

    fn open(&self) -> rusqlite::Connection {
        let mut connection =
            open_encrypted_database(&self.directory.path().join("library.db"), DATABASE_KEY)
                .expect("reopen database");
        run_migrations(&mut connection).expect("migrate database");
        connection
    }

    fn organizing_batch(&self, connection: &mut rusqlite::Connection) -> String {
        create_capture_batch(
            connection,
            CreateCaptureBatch {
                account_id: ACCOUNT.to_owned(),
                profile_id: self.profile_id.clone(),
                subject: "数学".to_owned(),
                state: CaptureBatchState::Organizing,
                now_utc_ms: 2,
            },
        )
        .expect("create batch")
        .id
    }

    fn insert_item(
        &self,
        connection: &rusqlite::Connection,
        batch_id: &str,
        item_id: &str,
        role: &str,
    ) {
        let asset_id = format!("asset-{item_id}");
        connection
            .execute(
                "INSERT INTO assets(
               id, account_id, plaintext_sha256, encrypted_path, byte_length,
               media_type, created_at_utc_ms
             ) VALUES(?1, ?2, ?3, ?4, 10, 'image/png', 3)",
                rusqlite::params![
                    asset_id,
                    ACCOUNT,
                    format!("hash-{item_id}"),
                    format!("blobs/{item_id}.mtb")
                ],
            )
            .unwrap();
        connection
            .execute(
                "INSERT INTO capture_items(
               id, batch_id, asset_id, client_upload_id, source_name,
               source_sequence, width, height, created_at_utc_ms, staged_role
             ) VALUES(?1, ?2, ?3, ?4, 'question.png', 0, 1200, 900, 4, ?5)",
                rusqlite::params![
                    item_id,
                    batch_id,
                    asset_id,
                    format!("upload-{item_id}"),
                    role
                ],
            )
            .unwrap();
    }

    fn ingest_image(
        &self,
        connection: &mut rusqlite::Connection,
        batch_id: &str,
        client_upload_id: &str,
    ) -> String {
        self.ingest_image_with_color(connection, batch_id, client_upload_id, [0, 0, 0])
    }

    fn ingest_image_with_color(
        &self,
        connection: &mut rusqlite::Connection,
        batch_id: &str,
        client_upload_id: &str,
        color: [u8; 3],
    ) -> String {
        let mut bytes = Cursor::new(Vec::new());
        image::DynamicImage::ImageRgb8(image::RgbImage::from_pixel(64, 64, image::Rgb(color)))
            .write_to(&mut bytes, image::ImageFormat::Png)
            .expect("encode test image");
        ingest_capture_item(
            connection,
            self.directory.path(),
            &ASSET_KEY,
            IngestCaptureItem {
                account_id: ACCOUNT.to_owned(),
                profile_id: self.profile_id.clone(),
                batch_id: batch_id.to_owned(),
                client_upload_id: client_upload_id.to_owned(),
                source_name: "sheet.png".to_owned(),
                source_sequence: None,
                bytes: bytes.into_inner(),
                now_utc_ms: 5,
            },
        )
        .expect("ingest image")
        .id
    }
}

fn start_input(
    library: &TestLibrary,
    batch_id: &str,
    item_ids: &[&str],
) -> CreateCaptureRecognitionJob {
    CreateCaptureRecognitionJob {
        account_id: ACCOUNT.to_owned(),
        profile_id: library.profile_id.clone(),
        batch_id: batch_id.to_owned(),
        item_ids: item_ids.iter().map(|value| (*value).to_owned()).collect(),
        engine: "ppocrv6-small-anchor".to_owned(),
        engine_version: "rapidocr-3.9.2+ppocrv6-small".to_owned(),
        now_utc_ms: 10,
    }
}

fn accepted_job(
    library: &TestLibrary,
    connection: &mut rusqlite::Connection,
    batch_id: &str,
    item_id: &str,
) -> (String, String) {
    let job =
        create_or_resume_recognition_job(connection, start_input(library, batch_id, &[item_id]))
            .expect("create recognition job");
    let suggested = store_recognition_suggestion(
        connection,
        StoreCaptureRecognitionSuggestion {
            account_id: ACCOUNT.to_owned(),
            profile_id: library.profile_id.clone(),
            job_id: job.id.clone(),
            item_id: item_id.to_owned(),
            regions: vec![
                CaptureRecognitionRegionProposal {
                    rect: mistake_trainer_next_lib::modules::capture_inbox::NormalizedCropRect {
                        x: 0.0,
                        y: 0.0,
                        width: 1.0,
                        height: 0.5,
                    },
                    role: CaptureRecognitionRole::Question,
                    group_slot: Some(0),
                    confidence_basis_points: 9_300,
                },
                CaptureRecognitionRegionProposal {
                    rect: mistake_trainer_next_lib::modules::capture_inbox::NormalizedCropRect {
                        x: 0.0,
                        y: 0.5,
                        width: 1.0,
                        height: 0.5,
                    },
                    role: CaptureRecognitionRole::Answer,
                    group_slot: Some(0),
                    confidence_basis_points: 9_100,
                },
            ],
            confidence_basis_points: 9_100,
            reason_codes: vec![CaptureRecognitionReasonCode::MatchedQuestionAnswerAnchor],
            now_utc_ms: 20,
        },
    )
    .expect("store suggestion");
    let suggestion_id = suggested.suggestions[0].id.clone();
    review_recognition_suggestion(
        connection,
        ReviewCaptureRecognitionSuggestion {
            account_id: ACCOUNT.to_owned(),
            profile_id: library.profile_id.clone(),
            job_id: job.id.clone(),
            suggestion_id: suggestion_id.clone(),
            decision: CaptureRecognitionDecision::Accepted,
            edited_regions: None,
            now_utc_ms: 21,
        },
    )
    .expect("accept suggestion");
    (job.id, suggestion_id)
}

fn recursive_file_count(path: &std::path::Path) -> usize {
    let Ok(entries) = std::fs::read_dir(path) else {
        return 0;
    };
    entries
        .filter_map(Result::ok)
        .map(|entry| {
            let path = entry.path();
            if path.is_dir() {
                recursive_file_count(&path)
            } else {
                1
            }
        })
        .sum()
}

struct QueueRecognitionEngine {
    outcomes: Mutex<VecDeque<Result<RecognitionAnalysis, RecognitionEngineError>>>,
    calls: AtomicUsize,
    active: AtomicUsize,
    max_active: AtomicUsize,
}

impl QueueRecognitionEngine {
    fn new(outcomes: Vec<Result<RecognitionAnalysis, RecognitionEngineError>>) -> Self {
        Self {
            outcomes: Mutex::new(outcomes.into()),
            calls: AtomicUsize::new(0),
            active: AtomicUsize::new(0),
            max_active: AtomicUsize::new(0),
        }
    }
}

impl CaptureRecognitionEngine for QueueRecognitionEngine {
    fn analyze(
        &self,
        image_path: &Path,
        _staged_role: CaptureRecognitionRole,
    ) -> Result<RecognitionAnalysis, RecognitionEngineError> {
        assert!(
            image_path.is_file(),
            "worker must pass a private image file"
        );
        self.calls.fetch_add(1, Ordering::SeqCst);
        let active = self.active.fetch_add(1, Ordering::SeqCst) + 1;
        self.max_active.fetch_max(active, Ordering::SeqCst);
        let result = self
            .outcomes
            .lock()
            .expect("engine outcomes")
            .pop_front()
            .unwrap_or(Err(RecognitionEngineError::Failed));
        self.active.fetch_sub(1, Ordering::SeqCst);
        result
    }
}

struct BlockingRecognitionEngine {
    entered: Arc<AtomicBool>,
    release: Arc<AtomicBool>,
}

impl CaptureRecognitionEngine for BlockingRecognitionEngine {
    fn analyze(
        &self,
        image_path: &Path,
        _staged_role: CaptureRecognitionRole,
    ) -> Result<RecognitionAnalysis, RecognitionEngineError> {
        assert!(image_path.is_file());
        self.entered.store(true, Ordering::Release);
        while !self.release.load(Ordering::Acquire) {
            std::thread::sleep(Duration::from_millis(5));
        }
        Ok(valid_analysis())
    }
}

fn valid_analysis() -> RecognitionAnalysis {
    RecognitionAnalysis {
        regions: vec![CaptureRecognitionRegionProposal {
            rect: mistake_trainer_next_lib::modules::capture_inbox::NormalizedCropRect {
                x: 0.05,
                y: 0.05,
                width: 0.9,
                height: 0.4,
            },
            role: CaptureRecognitionRole::Question,
            group_slot: Some(0),
            confidence_basis_points: 9_200,
        }],
        confidence_basis_points: 9_200,
        reason_codes: vec![CaptureRecognitionReasonCode::ClearQuestionAnchor],
        pairing_tokens: Vec::new(),
    }
}

fn invalid_geometry_analysis() -> RecognitionAnalysis {
    RecognitionAnalysis {
        regions: vec![CaptureRecognitionRegionProposal {
            rect: mistake_trainer_next_lib::modules::capture_inbox::NormalizedCropRect {
                x: 0.8,
                y: 0.1,
                width: 0.4,
                height: 0.4,
            },
            role: CaptureRecognitionRole::Question,
            group_slot: Some(0),
            confidence_basis_points: 9_000,
        }],
        confidence_basis_points: 9_000,
        reason_codes: vec![CaptureRecognitionReasonCode::ClearQuestionAnchor],
        pairing_tokens: Vec::new(),
    }
}

fn worker_context(
    library: &TestLibrary,
    connection: rusqlite::Connection,
    batch_id: &str,
    job_id: &str,
) -> CaptureRecognitionWorkerContext {
    CaptureRecognitionWorkerContext {
        connection: Arc::new(Mutex::new(connection)),
        account_id: ACCOUNT.to_owned(),
        profile_id: library.profile_id.clone(),
        batch_id: batch_id.to_owned(),
        job_id: job_id.to_owned(),
        blob_root: library.directory.path().to_owned(),
        private_temp_root: library.directory.path().join("recognition-private-temp"),
        asset_key: ASSET_KEY,
    }
}

fn collecting_event_sink() -> (
    Arc<Mutex<Vec<CaptureRecognitionChanged>>>,
    CaptureRecognitionEventSink,
) {
    let events = Arc::new(Mutex::new(Vec::new()));
    let collected = Arc::clone(&events);
    let sink: CaptureRecognitionEventSink = Arc::new(move |event| {
        collected.lock().expect("event sink").push(event);
    });
    (events, sink)
}

#[test]
fn job_scope_accepts_only_owned_unassigned_organizing_items() {
    let library = TestLibrary::new();
    let mut connection = library.open();
    let batch_id = library.organizing_batch(&mut connection);
    library.insert_item(&connection, &batch_id, "item-1", "question");

    let job = create_or_resume_recognition_job(
        &mut connection,
        start_input(&library, &batch_id, &["item-1"]),
    )
    .expect("create job");
    assert_eq!(job.state, CaptureRecognitionJobState::Queued);
    assert_eq!(job.total_items, 1);

    let wrong_account = create_or_resume_recognition_job(
        &mut connection,
        CreateCaptureRecognitionJob {
            account_id: "other-account".to_owned(),
            now_utc_ms: 11,
            ..start_input(&library, &batch_id, &["item-1"])
        },
    )
    .expect_err("foreign account must not see the batch");
    assert!(matches!(
        wrong_account,
        CaptureRecognitionError::BatchNotFound
    ));

    connection
        .execute(
            "INSERT INTO capture_drafts(
           id, batch_id, position, created_at_utc_ms, updated_at_utc_ms
         ) VALUES('draft', ?1, 0, 12, 12)",
            [&batch_id],
        )
        .unwrap();
    connection
        .execute(
            "INSERT INTO capture_draft_items(draft_id, item_id, role, position)
         VALUES('draft', 'item-1', 'question', 0)",
            [],
        )
        .unwrap();
    let assigned = capture_item_snapshot_hash(
        &connection,
        ACCOUNT,
        &library.profile_id,
        &batch_id,
        "item-1",
    )
    .expect_err("assigned item is no longer eligible");
    assert!(matches!(assigned, CaptureRecognitionError::ItemNotFound));
}

#[test]
fn duplicate_or_empty_scope_is_rejected_before_creating_a_job() {
    let library = TestLibrary::new();
    let mut connection = library.open();
    let batch_id = library.organizing_batch(&mut connection);
    library.insert_item(&connection, &batch_id, "item-1", "question");

    for item_ids in [Vec::<&str>::new(), vec!["item-1", "item-1"]] {
        let error = create_or_resume_recognition_job(
            &mut connection,
            start_input(&library, &batch_id, &item_ids),
        )
        .expect_err("invalid scope");
        assert!(matches!(error, CaptureRecognitionError::InvalidInput));
    }
    assert!(
        get_active_recognition_job(&connection, ACCOUNT, &library.profile_id, &batch_id,)
            .unwrap()
            .is_none()
    );
}

#[test]
fn starting_twice_resumes_the_existing_job() {
    let library = TestLibrary::new();
    let mut connection = library.open();
    let batch_id = library.organizing_batch(&mut connection);
    library.insert_item(&connection, &batch_id, "item-1", "question");
    let first = create_or_resume_recognition_job(
        &mut connection,
        start_input(&library, &batch_id, &["item-1"]),
    )
    .unwrap();
    let second = create_or_resume_recognition_job(
        &mut connection,
        start_input(&library, &batch_id, &["item-1"]),
    )
    .unwrap();

    assert_eq!(second.id, first.id);
    assert_eq!(
        connection
            .query_row(
                "SELECT COUNT(*) FROM capture_recognition_jobs WHERE batch_id = ?1",
                [&batch_id],
                |row| row.get::<_, i64>(0),
            )
            .unwrap(),
        1
    );
}

#[test]
fn item_snapshot_changes_when_the_manual_role_changes() {
    let library = TestLibrary::new();
    let mut connection = library.open();
    let batch_id = library.organizing_batch(&mut connection);
    library.insert_item(&connection, &batch_id, "item-1", "question");
    let before = capture_item_snapshot_hash(
        &connection,
        ACCOUNT,
        &library.profile_id,
        &batch_id,
        "item-1",
    )
    .unwrap();
    connection
        .execute(
            "UPDATE capture_items SET staged_role = 'answer' WHERE id = 'item-1'",
            [],
        )
        .unwrap();
    let after = capture_item_snapshot_hash(
        &connection,
        ACCOUNT,
        &library.profile_id,
        &batch_id,
        "item-1",
    )
    .unwrap();
    assert_ne!(after, before);
}

#[test]
fn suggestions_are_validated_and_confidence_bands_are_computed_in_rust() {
    assert_eq!(review_band(9_000), CaptureRecognitionReviewBand::High);
    assert_eq!(review_band(8_999), CaptureRecognitionReviewBand::Review);
    assert_eq!(review_band(6_500), CaptureRecognitionReviewBand::Review);
    assert_eq!(review_band(6_499), CaptureRecognitionReviewBand::Low);

    let library = TestLibrary::new();
    let mut connection = library.open();
    let batch_id = library.organizing_batch(&mut connection);
    library.insert_item(&connection, &batch_id, "item-1", "question");
    let job = create_or_resume_recognition_job(
        &mut connection,
        start_input(&library, &batch_id, &["item-1"]),
    )
    .unwrap();
    let result = store_recognition_suggestion(
        &mut connection,
        StoreCaptureRecognitionSuggestion {
            account_id: ACCOUNT.to_owned(),
            profile_id: library.profile_id.clone(),
            job_id: job.id,
            item_id: "item-1".to_owned(),
            regions: vec![CaptureRecognitionRegionProposal {
                rect: mistake_trainer_next_lib::modules::capture_inbox::NormalizedCropRect {
                    x: 0.0,
                    y: 0.0,
                    width: 1.0,
                    height: 1.0,
                },
                role: CaptureRecognitionRole::Question,
                group_slot: Some(0),
                confidence_basis_points: 9_200,
            }],
            confidence_basis_points: 9_200,
            reason_codes: vec![CaptureRecognitionReasonCode::ClearQuestionAnchor],
            now_utc_ms: 20,
        },
    )
    .expect("store suggestion");
    assert_eq!(result.state, CaptureRecognitionJobState::Review);
    assert_eq!(result.processed_items, 1);
    assert_eq!(
        result.suggestions[0].review_band,
        CaptureRecognitionReviewBand::High
    );
}

#[test]
fn a_single_exam_page_can_propose_more_than_ten_question_regions() {
    let library = TestLibrary::new();
    let mut connection = library.open();
    let batch_id = library.organizing_batch(&mut connection);
    library.insert_item(&connection, &batch_id, "twelve-question-page", "question");
    let job = create_or_resume_recognition_job(
        &mut connection,
        start_input(&library, &batch_id, &["twelve-question-page"]),
    )
    .unwrap();
    let regions = (0..12)
        .map(|index| CaptureRecognitionRegionProposal {
            rect: mistake_trainer_next_lib::modules::capture_inbox::NormalizedCropRect {
                x: 0.0,
                y: f64::from(index) * 0.075,
                width: 1.0,
                height: 0.07,
            },
            role: CaptureRecognitionRole::Question,
            group_slot: Some(index),
            confidence_basis_points: 7_500,
        })
        .collect::<Vec<_>>();

    let result = store_recognition_suggestion(
        &mut connection,
        StoreCaptureRecognitionSuggestion {
            account_id: ACCOUNT.to_owned(),
            profile_id: library.profile_id.clone(),
            job_id: job.id,
            item_id: "twelve-question-page".to_owned(),
            regions,
            confidence_basis_points: 7_500,
            reason_codes: vec![CaptureRecognitionReasonCode::ClearQuestionAnchor],
            now_utc_ms: 20,
        },
    )
    .expect("twelve regions from one source page must be valid");

    assert_eq!(result.suggestions.len(), 1);
    assert_eq!(result.suggestions[0].regions.len(), 12);
    assert_eq!(
        result.suggestions[0].review_band,
        CaptureRecognitionReviewBand::Review
    );
}

#[test]
fn review_rejects_low_confidence_acceptance_and_cancel_is_idempotent() {
    let library = TestLibrary::new();
    let mut connection = library.open();
    let batch_id = library.organizing_batch(&mut connection);
    library.insert_item(&connection, &batch_id, "item-1", "question");
    let queued = create_or_resume_recognition_job(
        &mut connection,
        start_input(&library, &batch_id, &["item-1"]),
    )
    .unwrap();
    let cancelled = cancel_recognition_job(
        &mut connection,
        ACCOUNT,
        &library.profile_id,
        &queued.id,
        10,
    )
    .unwrap();
    assert_eq!(cancelled.state, CaptureRecognitionJobState::Cancelled);
    assert_eq!(
        cancel_recognition_job(
            &mut connection,
            ACCOUNT,
            &library.profile_id,
            &queued.id,
            11,
        )
        .unwrap()
        .state,
        CaptureRecognitionJobState::Cancelled
    );

    let second_batch = library.organizing_batch(&mut connection);
    library.insert_item(&connection, &second_batch, "item-low", "question");
    let job = create_or_resume_recognition_job(
        &mut connection,
        start_input(&library, &second_batch, &["item-low"]),
    )
    .unwrap();
    let reviewed = store_recognition_suggestion(
        &mut connection,
        StoreCaptureRecognitionSuggestion {
            account_id: ACCOUNT.to_owned(),
            profile_id: library.profile_id.clone(),
            job_id: job.id.clone(),
            item_id: "item-low".to_owned(),
            regions: vec![CaptureRecognitionRegionProposal {
                rect: mistake_trainer_next_lib::modules::capture_inbox::NormalizedCropRect {
                    x: 0.0,
                    y: 0.0,
                    width: 1.0,
                    height: 1.0,
                },
                role: CaptureRecognitionRole::Question,
                group_slot: None,
                confidence_basis_points: 6_000,
            }],
            confidence_basis_points: 6_000,
            reason_codes: vec![CaptureRecognitionReasonCode::WeakAnchor],
            now_utc_ms: 20,
        },
    )
    .unwrap();
    let suggestion_id = reviewed.suggestions[0].id.clone();
    let error = review_recognition_suggestion(
        &mut connection,
        ReviewCaptureRecognitionSuggestion {
            account_id: ACCOUNT.to_owned(),
            profile_id: library.profile_id.clone(),
            job_id: job.id.clone(),
            suggestion_id: suggestion_id.clone(),
            decision: CaptureRecognitionDecision::Accepted,
            edited_regions: None,
            now_utc_ms: 21,
        },
    )
    .expect_err("low-confidence suggestions must remain skipped");
    assert!(matches!(error, CaptureRecognitionError::InvalidSuggestion));

    let rejected = review_recognition_suggestion(
        &mut connection,
        ReviewCaptureRecognitionSuggestion {
            account_id: ACCOUNT.to_owned(),
            profile_id: library.profile_id.clone(),
            job_id: job.id,
            suggestion_id,
            decision: CaptureRecognitionDecision::Rejected,
            edited_regions: None,
            now_utc_ms: 22,
        },
    )
    .unwrap();
    assert_eq!(
        rejected.suggestions[0].state,
        CaptureRecognitionSuggestionState::Rejected
    );
}

#[test]
fn accepted_regions_apply_atomically_to_the_material_library_and_can_revert() {
    let library = TestLibrary::new();
    let mut connection = library.open();
    let batch_id = library.organizing_batch(&mut connection);
    let item_id = library.ingest_image(&mut connection, &batch_id, "real-image");
    let (job_id, suggestion_id) = accepted_job(&library, &mut connection, &batch_id, &item_id);
    let before_revision: u32 = connection
        .query_row(
            "SELECT revision FROM capture_batches WHERE id = ?1",
            [&batch_id],
            |row| row.get(0),
        )
        .unwrap();
    let before_sync_count: i64 = connection
        .query_row("SELECT COUNT(*) FROM sync_operations", [], |row| row.get(0))
        .unwrap();
    let before_asset_count: i64 = connection
        .query_row("SELECT COUNT(*) FROM assets", [], |row| row.get(0))
        .unwrap();
    let before_blob_count = recursive_file_count(&library.directory.path().join("blobs"));

    let report = apply_capture_recognition(
        &mut connection,
        ApplyCaptureRecognition {
            account_id: ACCOUNT.to_owned(),
            profile_id: library.profile_id.clone(),
            batch_id: batch_id.clone(),
            job_id: job_id.clone(),
            expected_revision: before_revision,
            accepted_suggestion_ids: vec![suggestion_id],
            blob_root: library.directory.path().to_owned(),
            asset_key: ASSET_KEY,
            now_utc_ms: 30,
            failure_point: None,
        },
    )
    .expect("apply accepted regions");

    assert_eq!(report.applied_suggestion_count, 1);
    assert_eq!(report.created_draft_count, 0);
    assert_eq!(report.created_item_count, 2);
    assert_eq!(report.unmatched_answer_count, 0);
    assert_eq!(report.detail.batch.revision, before_revision + 1);
    assert!(report.detail.drafts.is_empty());
    assert_eq!(report.detail.unassigned_item_ids.len(), 2);
    assert!(
        report
            .detail
            .items
            .iter()
            .all(|item| item.draft_id.is_none())
    );
    assert!(
        report
            .detail
            .items
            .iter()
            .any(|item| item.staged_role == "question")
    );
    assert!(
        report
            .detail
            .items
            .iter()
            .any(|item| item.staged_role == "answer")
    );
    assert_eq!(
        connection
            .query_row("SELECT COUNT(*) FROM capture_draft_items", [], |row| {
                row.get::<_, i64>(0)
            })
            .unwrap(),
        0
    );
    assert_eq!(
        connection
            .query_row("SELECT COUNT(*) FROM problems", [], |row| row
                .get::<_, i64>(0))
            .unwrap(),
        0
    );
    assert_eq!(
        connection
            .query_row("SELECT COUNT(*) FROM sync_operations", [], |row| {
                row.get::<_, i64>(0)
            })
            .unwrap(),
        before_sync_count
    );
    let operation =
        latest_capture_recognition_operation(&connection, ACCOUNT, &library.profile_id, &batch_id)
            .unwrap()
            .expect("operation summary");
    assert_eq!(operation.operation_id, report.operation_id);
    assert!(!operation.reverted);

    let reverted = revert_capture_recognition(
        &mut connection,
        RevertCaptureRecognition {
            account_id: ACCOUNT.to_owned(),
            profile_id: library.profile_id.clone(),
            batch_id: batch_id.clone(),
            operation_id: report.operation_id,
            expected_revision: report.detail.batch.revision,
            blob_root: library.directory.path().to_owned(),
            now_utc_ms: 31,
        },
    )
    .expect("revert immediately");
    assert_eq!(reverted.reverted_item_count, 2);
    assert_eq!(reverted.detail.items.len(), 1);
    assert_eq!(reverted.detail.items[0].id, item_id);
    assert!(reverted.detail.drafts.is_empty());
    assert_eq!(
        connection
            .query_row(
                "SELECT state FROM capture_recognition_jobs WHERE id = ?1",
                [job_id],
                |row| row.get::<_, String>(0),
            )
            .unwrap(),
        "cancelled"
    );
    assert_eq!(
        connection
            .query_row("SELECT COUNT(*) FROM assets", [], |row| row
                .get::<_, i64>(0))
            .unwrap(),
        before_asset_count
    );
    assert_eq!(
        recursive_file_count(&library.directory.path().join("blobs")),
        before_blob_count
    );
    assert!(
        latest_capture_recognition_operation(&connection, ACCOUNT, &library.profile_id, &batch_id,)
            .unwrap()
            .expect("reverted operation summary")
            .reverted
    );
}

#[cfg(feature = "local-ocr-runtime")]
#[tokio::test]
#[ignore = "requires the local real-image corpus and hash-pinned OCR runtime"]
async fn real_double_column_page_becomes_twelve_recoverable_encrypted_drafts() {
    let component_directory = std::path::PathBuf::from(
        std::env::var_os("MISTAKE_TRAINER_OCR_COMPONENT_DIR")
            .expect("MISTAKE_TRAINER_OCR_COMPONENT_DIR is required"),
    );
    let runtime_library_path = std::path::PathBuf::from(
        std::env::var_os("MISTAKE_TRAINER_ORT_DLL").expect("MISTAKE_TRAINER_ORT_DLL is required"),
    );
    let corpus_directory = std::path::PathBuf::from(
        std::env::var_os("MISTAKE_TRAINER_OCR_REAL_IMAGE_DIR")
            .expect("MISTAKE_TRAINER_OCR_REAL_IMAGE_DIR is required"),
    );
    let source_bytes = std::fs::read(corpus_directory.join("sample-0006.png"))
        .expect("read real double-column page");

    let library = TestLibrary::new();
    let mut connection = library.open();
    let batch_id = library.organizing_batch(&mut connection);
    let source_item_id = ingest_capture_item(
        &mut connection,
        library.directory.path(),
        &ASSET_KEY,
        IngestCaptureItem {
            account_id: ACCOUNT.to_owned(),
            profile_id: library.profile_id.clone(),
            batch_id: batch_id.clone(),
            client_upload_id: "real-double-column".to_owned(),
            source_name: "sample-0006.png".to_owned(),
            source_sequence: None,
            bytes: source_bytes,
            now_utc_ms: 5,
        },
    )
    .expect("ingest encrypted real page")
    .id;
    let job = create_or_resume_recognition_job(
        &mut connection,
        start_input(&library, &batch_id, &[&source_item_id]),
    )
    .expect("create real recognition job");
    let context = worker_context(&library, connection, &batch_id, &job.id);
    let runtime_temp = library.directory.path().join("ocr-runtime-temp");
    let runtime = PpOcrLocalRuntime::from_verified_small_component(
        &component_directory,
        &runtime_library_path,
        &runtime_temp,
        2,
    )
    .expect("initialize verified OCR runtime");
    let manager =
        CaptureRecognitionManager::with_engine(Arc::new(AnchorRecognitionEngine::new(runtime)));
    let (_, sink) = collecting_event_sink();

    let reviewed = manager
        .run_job(context.clone(), sink)
        .await
        .expect("run encrypted real page")
        .expect("recognition job remains available");
    assert_eq!(reviewed.state, CaptureRecognitionJobState::Review);
    assert_eq!(reviewed.suggestions.len(), 1);
    assert_eq!(reviewed.suggestions[0].regions.len(), 12);
    assert_eq!(
        reviewed.suggestions[0].review_band,
        CaptureRecognitionReviewBand::Review
    );
    assert_eq!(
        reviewed.suggestions[0]
            .regions
            .iter()
            .map(|region| region.group_slot)
            .collect::<Vec<_>>(),
        (0..12).map(Some).collect::<Vec<_>>()
    );
    assert_eq!(recursive_file_count(&context.private_temp_root), 0);
    assert_eq!(recursive_file_count(&runtime_temp), 0);

    let suggestion_id = reviewed.suggestions[0].id.clone();
    let mut connection = context.connection.lock().expect("connection");
    review_recognition_suggestion(
        &mut connection,
        ReviewCaptureRecognitionSuggestion {
            account_id: ACCOUNT.to_owned(),
            profile_id: library.profile_id.clone(),
            job_id: job.id.clone(),
            suggestion_id: suggestion_id.clone(),
            decision: CaptureRecognitionDecision::Accepted,
            edited_regions: None,
            now_utc_ms: 20,
        },
    )
    .expect("accept twelve real regions");
    let revision = connection
        .query_row(
            "SELECT revision FROM capture_batches WHERE id = ?1",
            [&batch_id],
            |row| row.get::<_, u32>(0),
        )
        .unwrap();
    let report = apply_capture_recognition(
        &mut connection,
        ApplyCaptureRecognition {
            account_id: ACCOUNT.to_owned(),
            profile_id: library.profile_id.clone(),
            batch_id: batch_id.clone(),
            job_id: job.id,
            expected_revision: revision,
            accepted_suggestion_ids: vec![suggestion_id],
            blob_root: library.directory.path().to_owned(),
            asset_key: ASSET_KEY,
            now_utc_ms: 21,
            failure_point: None,
        },
    )
    .expect("apply twelve encrypted crops");

    assert_eq!(report.created_item_count, 12);
    assert_eq!(report.created_draft_count, 0);
    assert_eq!(report.detail.items.len(), 12);
    assert!(report.detail.drafts.is_empty());
    assert_eq!(report.detail.unassigned_item_ids.len(), 12);
    assert!(
        report
            .detail
            .items
            .iter()
            .all(|item| item.draft_id.is_none())
    );
    assert_eq!(
        connection
            .query_row(
                "SELECT COUNT(*) FROM capture_items
                 WHERE id = ?1 AND superseded_by_derivation_id IS NOT NULL",
                [&source_item_id],
                |row| row.get::<_, i64>(0),
            )
            .unwrap(),
        1
    );
    assert_eq!(
        connection
            .query_row(
                "SELECT COUNT(*) FROM capture_source_retention WHERE batch_id = ?1",
                [&batch_id],
                |row| row.get::<_, i64>(0),
            )
            .unwrap(),
        1
    );
    assert_eq!(
        connection
            .query_row("SELECT COUNT(*) FROM problems", [], |row| row
                .get::<_, i64>(0))
            .unwrap(),
        0
    );
    drop(connection);

    let mut reopened = library.open();
    let persisted_active_items = reopened
        .query_row(
            "SELECT COUNT(*) FROM capture_items
             WHERE batch_id = ?1 AND superseded_by_derivation_id IS NULL",
            [&batch_id],
            |row| row.get::<_, i64>(0),
        )
        .unwrap();
    assert_eq!(persisted_active_items, 12);
    let reverted = revert_capture_recognition(
        &mut reopened,
        RevertCaptureRecognition {
            account_id: ACCOUNT.to_owned(),
            profile_id: library.profile_id.clone(),
            batch_id,
            operation_id: report.operation_id,
            expected_revision: report.detail.batch.revision,
            blob_root: library.directory.path().to_owned(),
            now_utc_ms: 22,
        },
    )
    .expect("restore the retained source page");
    assert_eq!(reverted.reverted_item_count, 12);
    assert_eq!(reverted.detail.items.len(), 1);
    assert_eq!(reverted.detail.items[0].id, source_item_id);
    assert!(reverted.detail.drafts.is_empty());
}

#[test]
fn changed_source_is_marked_stale_without_mutating_the_batch() {
    let library = TestLibrary::new();
    let mut connection = library.open();
    let batch_id = library.organizing_batch(&mut connection);
    let item_id = library.ingest_image(&mut connection, &batch_id, "stale-image");
    let (job_id, suggestion_id) = accepted_job(&library, &mut connection, &batch_id, &item_id);
    let revision: u32 = connection
        .query_row(
            "SELECT revision FROM capture_batches WHERE id = ?1",
            [&batch_id],
            |row| row.get(0),
        )
        .unwrap();
    connection
        .execute(
            "UPDATE capture_items SET staged_role = 'answer' WHERE id = ?1",
            [&item_id],
        )
        .unwrap();

    let error = apply_capture_recognition(
        &mut connection,
        ApplyCaptureRecognition {
            account_id: ACCOUNT.to_owned(),
            profile_id: library.profile_id.clone(),
            batch_id: batch_id.clone(),
            job_id,
            expected_revision: revision,
            accepted_suggestion_ids: vec![suggestion_id.clone()],
            blob_root: library.directory.path().to_owned(),
            asset_key: ASSET_KEY,
            now_utc_ms: 30,
            failure_point: None,
        },
    )
    .expect_err("changed source must not apply");
    assert!(matches!(error, CaptureRecognitionError::Stale));
    assert_eq!(
        connection
            .query_row(
                "SELECT revision FROM capture_batches WHERE id = ?1",
                [&batch_id],
                |row| row.get::<_, u32>(0),
            )
            .unwrap(),
        revision
    );
    assert_eq!(
        connection
            .query_row(
                "SELECT state FROM capture_recognition_suggestions WHERE id = ?1",
                [suggestion_id],
                |row| row.get::<_, String>(0),
            )
            .unwrap(),
        "stale"
    );
    assert_eq!(
        connection
            .query_row(
                "SELECT COUNT(*) FROM capture_recognition_operations WHERE batch_id = ?1",
                [&batch_id],
                |row| row.get::<_, i64>(0),
            )
            .unwrap(),
        0
    );
}

#[test]
fn failures_before_staging_after_staging_and_inside_transaction_leave_no_partial_apply() {
    for failure_point in [
        CaptureRecognitionFailurePoint::BeforeStaging,
        CaptureRecognitionFailurePoint::AfterStaging,
        CaptureRecognitionFailurePoint::InTransaction,
    ] {
        let library = TestLibrary::new();
        let mut connection = library.open();
        let batch_id = library.organizing_batch(&mut connection);
        let item_id = library.ingest_image(
            &mut connection,
            &batch_id,
            &format!("fault-{failure_point:?}"),
        );
        let (job_id, suggestion_id) = accepted_job(&library, &mut connection, &batch_id, &item_id);
        let before_revision: u32 = connection
            .query_row(
                "SELECT revision FROM capture_batches WHERE id = ?1",
                [&batch_id],
                |row| row.get(0),
            )
            .unwrap();
        let before_asset_count: i64 = connection
            .query_row("SELECT COUNT(*) FROM assets", [], |row| row.get(0))
            .unwrap();
        let before_blob_count = recursive_file_count(&library.directory.path().join("blobs"));

        let error = apply_capture_recognition(
            &mut connection,
            ApplyCaptureRecognition {
                account_id: ACCOUNT.to_owned(),
                profile_id: library.profile_id.clone(),
                batch_id: batch_id.clone(),
                job_id: job_id.clone(),
                expected_revision: before_revision,
                accepted_suggestion_ids: vec![suggestion_id.clone()],
                blob_root: library.directory.path().to_owned(),
                asset_key: ASSET_KEY,
                now_utc_ms: 30,
                failure_point: Some(failure_point),
            },
        )
        .expect_err("fault injection must abort the whole apply");
        assert!(
            matches!(error, CaptureRecognitionError::InjectedFailure),
            "unexpected error at {failure_point:?}: {error}"
        );

        assert_eq!(
            connection
                .query_row(
                    "SELECT revision FROM capture_batches WHERE id = ?1",
                    [&batch_id],
                    |row| row.get::<_, u32>(0),
                )
                .unwrap(),
            before_revision,
            "batch revision changed at {failure_point:?}"
        );
        assert_eq!(
            connection
                .query_row(
                    "SELECT COUNT(*) FROM capture_items
                     WHERE batch_id = ?1 AND superseded_by_derivation_id IS NULL",
                    [&batch_id],
                    |row| row.get::<_, i64>(0),
                )
                .unwrap(),
            1,
            "active item set changed at {failure_point:?}"
        );
        for table in [
            "capture_drafts",
            "asset_derivations",
            "capture_recognition_operations",
        ] {
            let count: i64 = connection
                .query_row(
                    &format!("SELECT COUNT(*) FROM {table} WHERE batch_id = ?1"),
                    [&batch_id],
                    |row| row.get(0),
                )
                .unwrap();
            assert_eq!(count, 0, "{table} changed at {failure_point:?}");
        }
        assert_eq!(
            connection
                .query_row("SELECT COUNT(*) FROM assets", [], |row| row
                    .get::<_, i64>(0))
                .unwrap(),
            before_asset_count,
            "asset rows changed at {failure_point:?}"
        );
        assert_eq!(
            recursive_file_count(&library.directory.path().join("blobs")),
            before_blob_count,
            "blob files changed at {failure_point:?}"
        );
        assert_eq!(
            recursive_file_count(&library.directory.path().join(".staging")),
            0,
            "staging files leaked at {failure_point:?}"
        );
        assert_eq!(
            connection
                .query_row(
                    "SELECT state FROM capture_recognition_jobs WHERE id = ?1",
                    [job_id],
                    |row| row.get::<_, String>(0),
                )
                .unwrap(),
            "review"
        );
        assert_eq!(
            connection
                .query_row(
                    "SELECT state FROM capture_recognition_suggestions WHERE id = ?1",
                    [suggestion_id],
                    |row| row.get::<_, String>(0),
                )
                .unwrap(),
            "accepted"
        );
    }
}

#[test]
fn revert_refuses_to_delete_recognition_items_after_manual_edits() {
    let library = TestLibrary::new();
    let mut connection = library.open();
    let batch_id = library.organizing_batch(&mut connection);
    let item_id = library.ingest_image(&mut connection, &batch_id, "edited-image");
    let (job_id, suggestion_id) = accepted_job(&library, &mut connection, &batch_id, &item_id);
    let revision: u32 = connection
        .query_row(
            "SELECT revision FROM capture_batches WHERE id = ?1",
            [&batch_id],
            |row| row.get(0),
        )
        .unwrap();
    let report = apply_capture_recognition(
        &mut connection,
        ApplyCaptureRecognition {
            account_id: ACCOUNT.to_owned(),
            profile_id: library.profile_id.clone(),
            batch_id: batch_id.clone(),
            job_id,
            expected_revision: revision,
            accepted_suggestion_ids: vec![suggestion_id],
            blob_root: library.directory.path().to_owned(),
            asset_key: ASSET_KEY,
            now_utc_ms: 30,
            failure_point: None,
        },
    )
    .unwrap();
    let generated_item_id = report.detail.items[0].id.clone();
    connection
        .execute(
            "UPDATE capture_items SET staged_role = 'answer' WHERE id = ?1",
            [&generated_item_id],
        )
        .unwrap();

    let error = revert_capture_recognition(
        &mut connection,
        RevertCaptureRecognition {
            account_id: ACCOUNT.to_owned(),
            profile_id: library.profile_id.clone(),
            batch_id: batch_id.clone(),
            operation_id: report.operation_id,
            expected_revision: report.detail.batch.revision,
            blob_root: library.directory.path().to_owned(),
            now_utc_ms: 31,
        },
    )
    .expect_err("manual edits make automatic revert unsafe");
    assert!(matches!(error, CaptureRecognitionError::RevertConflict));
    assert_eq!(
        connection
            .query_row(
                "SELECT COUNT(*) FROM capture_items
                 WHERE batch_id = ?1 AND superseded_by_derivation_id IS NULL",
                [&batch_id],
                |row| row.get::<_, i64>(0),
            )
            .unwrap(),
        2
    );
}

#[tokio::test]
async fn worker_processes_encrypted_items_sequentially_and_cleans_private_plaintext() {
    let library = TestLibrary::new();
    let mut connection = library.open();
    let batch_id = library.organizing_batch(&mut connection);
    let item_ids = (0..3)
        .map(|index| {
            library.ingest_image(
                &mut connection,
                &batch_id,
                &format!("worker-success-{index}"),
            )
        })
        .collect::<Vec<_>>();
    let item_refs = item_ids.iter().map(String::as_str).collect::<Vec<_>>();
    let job = create_or_resume_recognition_job(
        &mut connection,
        start_input(&library, &batch_id, &item_refs),
    )
    .expect("create worker job");
    let context = worker_context(&library, connection, &batch_id, &job.id);
    let engine = Arc::new(QueueRecognitionEngine::new(vec![
        Ok(valid_analysis()),
        Ok(valid_analysis()),
        Ok(valid_analysis()),
    ]));
    let manager = CaptureRecognitionManager::with_engine(engine.clone());
    let (events, sink) = collecting_event_sink();

    let completed = manager
        .run_job(context.clone(), sink)
        .await
        .expect("run worker")
        .expect("job remains available");

    assert_eq!(completed.state, CaptureRecognitionJobState::Review);
    assert_eq!(completed.processed_items, 3);
    assert_eq!(completed.suggestions.len(), 3);
    assert_eq!(engine.calls.load(Ordering::SeqCst), 3);
    assert_eq!(engine.max_active.load(Ordering::SeqCst), 1);
    assert_eq!(recursive_file_count(&context.private_temp_root), 0);
    let events = events.lock().expect("events");
    assert_eq!(
        events.last().map(|event| event.state.as_str()),
        Some("review")
    );
    assert_eq!(events.last().map(|event| event.processed_items), Some(3));
    let serialized = serde_json::to_string(&*events).expect("serialize events");
    assert!(!serialized.contains("recognition-private-temp"));
    assert!(!serialized.contains("encrypted_path"));
}

#[tokio::test]
async fn worker_isolates_bad_geometry_and_engine_failures_then_continues() {
    let library = TestLibrary::new();
    let mut connection = library.open();
    let batch_id = library.organizing_batch(&mut connection);
    let item_ids = (0..3)
        .map(|index| {
            library.ingest_image(
                &mut connection,
                &batch_id,
                &format!("worker-isolation-{index}"),
            )
        })
        .collect::<Vec<_>>();
    let item_refs = item_ids.iter().map(String::as_str).collect::<Vec<_>>();
    let job = create_or_resume_recognition_job(
        &mut connection,
        start_input(&library, &batch_id, &item_refs),
    )
    .expect("create worker job");
    let context = worker_context(&library, connection, &batch_id, &job.id);
    let engine = Arc::new(QueueRecognitionEngine::new(vec![
        Ok(invalid_geometry_analysis()),
        Err(RecognitionEngineError::Failed),
        Ok(valid_analysis()),
    ]));
    let manager = CaptureRecognitionManager::with_engine(engine);
    let (_, sink) = collecting_event_sink();

    let completed = manager
        .run_job(context.clone(), sink)
        .await
        .expect("run worker")
        .expect("job remains available");

    assert_eq!(completed.state, CaptureRecognitionJobState::Review);
    assert_eq!(completed.processed_items, 3);
    assert_eq!(completed.suggestions.len(), 1);
    let connection = context.connection.lock().expect("connection");
    let states = {
        let mut statement = connection
            .prepare(
                "SELECT state FROM capture_recognition_job_items
                 WHERE job_id = ?1 ORDER BY position",
            )
            .unwrap();
        statement
            .query_map([&job.id], |row| row.get::<_, String>(0))
            .unwrap()
            .collect::<Result<Vec<_>, _>>()
            .unwrap()
    };
    assert_eq!(states, ["failed", "failed", "complete"]);
    assert_eq!(recursive_file_count(&context.private_temp_root), 0);
}

#[tokio::test]
async fn worker_skips_a_corrupt_encrypted_asset_and_continues() {
    let library = TestLibrary::new();
    let mut connection = library.open();
    let batch_id = library.organizing_batch(&mut connection);
    let corrupt_item = library.ingest_image(&mut connection, &batch_id, "worker-corrupt-1");
    let healthy_item = library.ingest_image_with_color(
        &mut connection,
        &batch_id,
        "worker-corrupt-2",
        [255, 255, 255],
    );
    let corrupt_path: String = connection
        .query_row(
            "SELECT a.encrypted_path
             FROM capture_items i JOIN assets a ON a.id = i.asset_id
             WHERE i.id = ?1",
            [&corrupt_item],
            |row| row.get(0),
        )
        .unwrap();
    std::fs::write(
        library.directory.path().join(corrupt_path),
        b"not-a-valid-asset",
    )
    .expect("corrupt encrypted fixture");
    let job = create_or_resume_recognition_job(
        &mut connection,
        start_input(&library, &batch_id, &[&corrupt_item, &healthy_item]),
    )
    .expect("create worker job");
    let context = worker_context(&library, connection, &batch_id, &job.id);
    let engine = Arc::new(QueueRecognitionEngine::new(vec![Ok(valid_analysis())]));
    let manager = CaptureRecognitionManager::with_engine(engine.clone());
    let (_, sink) = collecting_event_sink();

    let completed = manager
        .run_job(context.clone(), sink)
        .await
        .expect("run worker")
        .expect("job remains available");

    assert_eq!(completed.state, CaptureRecognitionJobState::Review);
    assert_eq!(completed.processed_items, 2);
    assert_eq!(completed.suggestions.len(), 1);
    assert_eq!(engine.calls.load(Ordering::SeqCst), 1);
    let connection = context.connection.lock().expect("connection");
    let states = {
        let mut statement = connection
            .prepare(
                "SELECT state FROM capture_recognition_job_items
                 WHERE job_id = ?1 ORDER BY position",
            )
            .unwrap();
        statement
            .query_map([&job.id], |row| row.get::<_, String>(0))
            .unwrap()
            .collect::<Result<Vec<_>, _>>()
            .unwrap()
    };
    assert_eq!(states, ["failed", "complete"]);
    assert_eq!(recursive_file_count(&context.private_temp_root), 0);
}

#[tokio::test]
async fn missing_model_fails_the_whole_job_without_leaking_plaintext() {
    let library = TestLibrary::new();
    let mut connection = library.open();
    let batch_id = library.organizing_batch(&mut connection);
    let first = library.ingest_image(&mut connection, &batch_id, "worker-model-missing-1");
    let second = library.ingest_image(&mut connection, &batch_id, "worker-model-missing-2");
    let job = create_or_resume_recognition_job(
        &mut connection,
        start_input(&library, &batch_id, &[&first, &second]),
    )
    .expect("create worker job");
    let context = worker_context(&library, connection, &batch_id, &job.id);
    let engine = Arc::new(QueueRecognitionEngine::new(vec![Err(
        RecognitionEngineError::ModelMissing,
    )]));
    let manager = CaptureRecognitionManager::with_engine(engine);
    let (_, sink) = collecting_event_sink();

    let completed = manager
        .run_job(context.clone(), sink)
        .await
        .expect("run worker")
        .expect("job remains available");

    assert_eq!(completed.state, CaptureRecognitionJobState::Failed);
    assert_eq!(completed.processed_items, 2);
    let connection = context.connection.lock().expect("connection");
    let failure_code: String = connection
        .query_row(
            "SELECT failure_code FROM capture_recognition_jobs WHERE id = ?1",
            [&job.id],
            |row| row.get(0),
        )
        .unwrap();
    assert_eq!(failure_code, "model_missing");
    assert_eq!(recursive_file_count(&context.private_temp_root), 0);
}

#[tokio::test]
async fn unavailable_runtime_fails_the_whole_job_after_one_engine_call() {
    let library = TestLibrary::new();
    let mut connection = library.open();
    let batch_id = library.organizing_batch(&mut connection);
    let first = library.ingest_image(&mut connection, &batch_id, "worker-runtime-missing-1");
    let second = library.ingest_image(&mut connection, &batch_id, "worker-runtime-missing-2");
    let job = create_or_resume_recognition_job(
        &mut connection,
        start_input(&library, &batch_id, &[&first, &second]),
    )
    .expect("create worker job");
    let context = worker_context(&library, connection, &batch_id, &job.id);
    let engine = Arc::new(QueueRecognitionEngine::new(vec![Err(
        RecognitionEngineError::RuntimeUnavailable,
    )]));
    let manager = CaptureRecognitionManager::with_engine(engine.clone());
    let (_, sink) = collecting_event_sink();

    let completed = manager
        .run_job(context.clone(), sink)
        .await
        .expect("run worker")
        .expect("job remains available");

    assert_eq!(completed.state, CaptureRecognitionJobState::Failed);
    assert_eq!(completed.processed_items, 2);
    assert_eq!(engine.calls.load(Ordering::SeqCst), 1);
    let connection = context.connection.lock().expect("connection");
    let failure_code: String = connection
        .query_row(
            "SELECT failure_code FROM capture_recognition_jobs WHERE id = ?1",
            [&job.id],
            |row| row.get(0),
        )
        .unwrap();
    assert_eq!(failure_code, "runtime_unavailable");
    assert_eq!(recursive_file_count(&context.private_temp_root), 0);
}

#[tokio::test]
async fn shutdown_cancels_after_preprocessing_and_cleans_private_plaintext() {
    let library = TestLibrary::new();
    let mut connection = library.open();
    let batch_id = library.organizing_batch(&mut connection);
    let item_id = library.ingest_image(&mut connection, &batch_id, "worker-cancel");
    let job = create_or_resume_recognition_job(
        &mut connection,
        start_input(&library, &batch_id, &[&item_id]),
    )
    .expect("create worker job");
    let context = worker_context(&library, connection, &batch_id, &job.id);
    let entered = Arc::new(AtomicBool::new(false));
    let release = Arc::new(AtomicBool::new(false));
    let manager = CaptureRecognitionManager::with_engine(Arc::new(BlockingRecognitionEngine {
        entered: Arc::clone(&entered),
        release: Arc::clone(&release),
    }));
    let (_, sink) = collecting_event_sink();
    let running_manager = manager.clone();
    let running_context = context.clone();
    let handle = tokio::spawn(async move { running_manager.run_job(running_context, sink).await });

    for _ in 0..200 {
        if entered.load(Ordering::Acquire) {
            break;
        }
        tokio::time::sleep(Duration::from_millis(5)).await;
    }
    assert!(
        entered.load(Ordering::Acquire),
        "engine should start after the plaintext file is prepared"
    );
    manager.shutdown().await;
    release.store(true, Ordering::Release);
    let completed = handle
        .await
        .expect("worker task")
        .expect("worker result")
        .expect("cancelled job remains available");

    assert_eq!(completed.state, CaptureRecognitionJobState::Cancelled);
    assert_eq!(recursive_file_count(&context.private_temp_root), 0);
}

#[tokio::test]
async fn worker_exits_cleanly_when_its_batch_is_discarded() {
    let library = TestLibrary::new();
    let mut connection = library.open();
    let batch_id = library.organizing_batch(&mut connection);
    let item_id = library.ingest_image(&mut connection, &batch_id, "worker-discard");
    let job = create_or_resume_recognition_job(
        &mut connection,
        start_input(&library, &batch_id, &[&item_id]),
    )
    .expect("create worker job");
    let context = worker_context(&library, connection, &batch_id, &job.id);
    let entered = Arc::new(AtomicBool::new(false));
    let release = Arc::new(AtomicBool::new(false));
    let manager = CaptureRecognitionManager::with_engine(Arc::new(BlockingRecognitionEngine {
        entered: Arc::clone(&entered),
        release: Arc::clone(&release),
    }));
    let (_, sink) = collecting_event_sink();
    let running_manager = manager.clone();
    let running_context = context.clone();
    let handle = tokio::spawn(async move { running_manager.run_job(running_context, sink).await });

    for _ in 0..200 {
        if entered.load(Ordering::Acquire) {
            break;
        }
        tokio::time::sleep(Duration::from_millis(5)).await;
    }
    assert!(entered.load(Ordering::Acquire));
    context
        .connection
        .lock()
        .expect("connection")
        .execute("DELETE FROM capture_batches WHERE id = ?1", [&batch_id])
        .expect("discard batch");
    release.store(true, Ordering::Release);

    assert!(
        handle
            .await
            .expect("worker task")
            .expect("worker result")
            .is_none(),
        "discarded jobs should disappear without surfacing a worker failure"
    );
    assert_eq!(recursive_file_count(&context.private_temp_root), 0);
}

#[test]
fn startup_resets_abandoned_running_items_to_pending() {
    let library = TestLibrary::new();
    let mut connection = library.open();
    let batch_id = library.organizing_batch(&mut connection);
    let item_id = library.ingest_image(&mut connection, &batch_id, "worker-restart");
    let job = create_or_resume_recognition_job(
        &mut connection,
        start_input(&library, &batch_id, &[&item_id]),
    )
    .expect("create worker job");
    connection
        .execute(
            "UPDATE capture_recognition_jobs SET state = 'running' WHERE id = ?1",
            [&job.id],
        )
        .unwrap();
    connection
        .execute(
            "UPDATE capture_recognition_job_items
             SET state = 'running' WHERE job_id = ?1 AND item_id = ?2",
            [&job.id, &item_id],
        )
        .unwrap();

    assert_eq!(
        reset_abandoned_recognition_work(&mut connection, 99).unwrap(),
        1
    );
    let (job_state, item_state, processed): (String, String, i64) = connection
        .query_row(
            "SELECT j.state, ji.state, j.processed_items
             FROM capture_recognition_jobs j
             JOIN capture_recognition_job_items ji ON ji.job_id = j.id
             WHERE j.id = ?1",
            [&job.id],
            |row| Ok((row.get(0)?, row.get(1)?, row.get(2)?)),
        )
        .unwrap();
    assert_eq!(job_state, "queued");
    assert_eq!(item_state, "pending");
    assert_eq!(processed, 0);
}

#[test]
fn startup_replays_the_whole_interrupted_job_so_private_pairing_state_stays_consistent() {
    let library = TestLibrary::new();
    let mut connection = library.open();
    let batch_id = library.organizing_batch(&mut connection);
    let first_id = library.ingest_image(&mut connection, &batch_id, "restart-first");
    let second_id = library.ingest_image(&mut connection, &batch_id, "restart-second");
    let job = create_or_resume_recognition_job(
        &mut connection,
        start_input(&library, &batch_id, &[&first_id, &second_id]),
    )
    .expect("create worker job");
    connection
        .execute(
            "UPDATE capture_recognition_job_items SET state = 'running'
             WHERE job_id = ?1 AND item_id = ?2",
            [&job.id, &first_id],
        )
        .unwrap();
    store_recognition_suggestion(
        &mut connection,
        StoreCaptureRecognitionSuggestion {
            account_id: ACCOUNT.to_owned(),
            profile_id: library.profile_id.clone(),
            job_id: job.id.clone(),
            item_id: first_id,
            regions: valid_analysis().regions,
            confidence_basis_points: 9_200,
            reason_codes: vec![CaptureRecognitionReasonCode::ClearQuestionAnchor],
            now_utc_ms: 20,
        },
    )
    .expect("finish first item");
    connection
        .execute(
            "UPDATE capture_recognition_job_items SET state = 'running'
             WHERE job_id = ?1 AND item_id = ?2",
            [&job.id, &second_id],
        )
        .unwrap();

    assert_eq!(
        reset_abandoned_recognition_work(&mut connection, 99).unwrap(),
        2
    );
    assert_eq!(
        connection
            .query_row(
                "SELECT COUNT(*) FROM capture_recognition_suggestions WHERE job_id = ?1",
                [&job.id],
                |row| row.get::<_, i64>(0),
            )
            .unwrap(),
        0
    );
    assert_eq!(
        connection
            .query_row(
                "SELECT COUNT(*) FROM capture_recognition_job_items
                 WHERE job_id = ?1 AND state = 'pending'",
                [&job.id],
                |row| row.get::<_, i64>(0),
            )
            .unwrap(),
        2
    );
    assert_eq!(
        connection
            .query_row(
                "SELECT processed_items FROM capture_recognition_jobs WHERE id = ?1",
                [&job.id],
                |row| row.get::<_, i64>(0),
            )
            .unwrap(),
        0
    );
}
