use std::io::Cursor;

use base64::{Engine as _, engine::general_purpose::STANDARD};
use image::{DynamicImage, ImageBuffer, ImageFormat, Rgba};
use mistake_trainer_next_lib::{
    infrastructure::database::{open_encrypted_database, run_migrations},
    modules::{
        capture_inbox::{
            ApplyCaptureLayout, CaptureBatchState, CaptureInboxError, CaptureLayoutMode,
            CreateCaptureBatch, IngestCaptureItem, MergeCaptureCard, MoveCaptureItem,
            StageCaptureItemRole, UpdateCaptureDraft, apply_capture_layout,
            assign_capture_batch_subject, commit_ready_capture_drafts, create_capture_batch,
            delete_capture_draft, discard_capture_batch, get_capture_batch_detail,
            get_capture_item_preview, ingest_capture_item, merge_capture_card, move_capture_item,
            stage_capture_item_role, update_capture_batch, update_capture_draft,
        },
        profiles::{CreateProfile, create_profile},
    },
};
use tempfile::{TempDir, tempdir};

const ACCOUNT: &str = "account-1";
const DATABASE_KEY: &str = "capture-inbox-test-key";
const ASSET_KEY: [u8; 32] = [73; 32];

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

    fn blob_root(&self) -> std::path::PathBuf {
        self.directory.path().join("assets")
    }
}

fn png(seed: u8) -> Vec<u8> {
    png_sized(seed, 4, 3)
}

fn png_sized(seed: u8, width: u32, height: u32) -> Vec<u8> {
    let image = DynamicImage::ImageRgba8(ImageBuffer::from_pixel(
        width,
        height,
        Rgba([seed, seed.wrapping_add(1), seed.wrapping_add(2), 255]),
    ));
    let mut output = Cursor::new(Vec::new());
    image.write_to(&mut output, ImageFormat::Png).unwrap();
    output.into_inner()
}

#[test]
fn capture_preview_uses_readable_960_pixel_edge() {
    let library = TestLibrary::new();
    let mut connection = library.open();
    let batch = create_batch(
        &library,
        &mut connection,
        "math",
        CaptureBatchState::Collecting,
    );
    let item = ingest_capture_item(
        &mut connection,
        &library.blob_root(),
        &ASSET_KEY,
        IngestCaptureItem {
            account_id: ACCOUNT.to_owned(),
            profile_id: library.profile_id.clone(),
            batch_id: batch.id.clone(),
            client_upload_id: "large-preview".to_owned(),
            source_name: "large.png".to_owned(),
            source_sequence: None,
            bytes: png_sized(17, 1200, 800),
            now_utc_ms: 20,
        },
    )
    .expect("ingest large image");

    let preview = get_capture_item_preview(
        &connection,
        &library.blob_root(),
        &ASSET_KEY,
        ACCOUNT,
        &library.profile_id,
        &batch.id,
        &item.id,
    )
    .expect("derive preview");
    let encoded = preview.data_url.split_once(',').expect("data url").1;
    let bytes = STANDARD.decode(encoded).expect("base64 preview");
    let image = image::load_from_memory(&bytes).expect("decode preview");

    assert_eq!((image.width(), image.height()), (960, 640));
}

fn create_batch(
    library: &TestLibrary,
    connection: &mut rusqlite::Connection,
    subject: &str,
    state: CaptureBatchState,
) -> mistake_trainer_next_lib::modules::capture_inbox::CaptureBatchSummary {
    create_capture_batch(
        connection,
        CreateCaptureBatch {
            account_id: ACCOUNT.to_owned(),
            profile_id: library.profile_id.clone(),
            subject: subject.to_owned(),
            state,
            now_utc_ms: 10,
        },
    )
    .expect("create batch")
}

fn ingest(
    library: &TestLibrary,
    connection: &mut rusqlite::Connection,
    batch_id: &str,
    client_id: &str,
    seed: u8,
    now: i64,
) -> mistake_trainer_next_lib::modules::capture_inbox::CaptureItemSummary {
    ingest_capture_item(
        connection,
        &library.blob_root(),
        &ASSET_KEY,
        IngestCaptureItem {
            account_id: ACCOUNT.to_owned(),
            profile_id: library.profile_id.clone(),
            batch_id: batch_id.to_owned(),
            client_upload_id: client_id.to_owned(),
            source_name: format!("../photo-{seed}.png"),
            source_sequence: None,
            bytes: png(seed),
            now_utc_ms: now,
        },
    )
    .expect("ingest item")
}

fn organize(
    library: &TestLibrary,
    connection: &mut rusqlite::Connection,
    batch_id: &str,
    revision: u32,
) -> u32 {
    update_capture_batch(
        connection,
        ACCOUNT,
        &library.profile_id,
        batch_id,
        revision,
        "math",
        true,
        50,
    )
    .expect("finish collecting")
    .revision
}

#[test]
fn loose_roles_persist_and_card_merge_is_atomic() {
    let library = TestLibrary::new();
    let mut connection = library.open();
    let batch = create_batch(
        &library,
        &mut connection,
        "math",
        CaptureBatchState::Collecting,
    );
    let question = ingest(&library, &mut connection, &batch.id, "question", 31, 20);
    let answer = ingest(&library, &mut connection, &batch.id, "answer", 32, 21);
    let revision = organize(&library, &mut connection, &batch.id, batch.revision + 2);

    let staged = stage_capture_item_role(
        &mut connection,
        StageCaptureItemRole {
            account_id: ACCOUNT.to_owned(),
            profile_id: library.profile_id.clone(),
            batch_id: batch.id.clone(),
            expected_revision: revision,
            item_id: answer.id.clone(),
            staged_role: "answer".to_owned(),
            now_utc_ms: 60,
        },
    )
    .expect("persist answer role");
    let created = merge_capture_card(
        &mut connection,
        MergeCaptureCard {
            account_id: ACCOUNT.to_owned(),
            profile_id: library.profile_id.clone(),
            batch_id: batch.id.clone(),
            expected_revision: staged.batch.revision,
            target_draft_id: None,
            item_ids: vec![question.id.clone()],
            new_draft_subject: None,
            now_utc_ms: 61,
        },
    )
    .expect("create card and attach question in one revision");
    let draft_id = created.drafts[0].id.clone();
    let merged = merge_capture_card(
        &mut connection,
        MergeCaptureCard {
            account_id: ACCOUNT.to_owned(),
            profile_id: library.profile_id.clone(),
            batch_id: batch.id.clone(),
            expected_revision: created.batch.revision,
            target_draft_id: Some(draft_id.clone()),
            item_ids: vec![answer.id.clone()],
            new_draft_subject: None,
            now_utc_ms: 62,
        },
    )
    .expect("merge answer into existing card");

    assert_eq!(merged.drafts.len(), 1);
    assert_eq!(merged.drafts[0].question_item_ids, vec![question.id]);
    assert_eq!(merged.drafts[0].answer_item_ids, vec![answer.id.clone()]);
    assert!(merged.drafts[0].ready);
    assert_eq!(
        merged
            .items
            .iter()
            .find(|item| item.id == answer.id)
            .unwrap()
            .staged_role,
        "answer"
    );

    drop(connection);
    let reopened = library.open();
    let restored = get_capture_batch_detail(&reopened, ACCOUNT, &library.profile_id, &batch.id)
        .expect("restore organized batch");
    assert_eq!(restored.drafts[0].id, draft_id);
    assert_eq!(restored.items[1].staged_role, "answer");
}

#[test]
fn new_card_inherits_subject_and_deleting_it_returns_images_without_deleting_assets() {
    let library = TestLibrary::new();
    let mut connection = library.open();
    let batch = create_batch(&library, &mut connection, "", CaptureBatchState::Collecting);
    let question = ingest(
        &library,
        &mut connection,
        &batch.id,
        "undo-question",
        33,
        20,
    );
    let answer = ingest(&library, &mut connection, &batch.id, "undo-answer", 34, 21);
    let organized = update_capture_batch(
        &mut connection,
        ACCOUNT,
        &library.profile_id,
        &batch.id,
        batch.revision + 2,
        "",
        true,
        50,
    )
    .expect("finish collecting");
    let staged = stage_capture_item_role(
        &mut connection,
        StageCaptureItemRole {
            account_id: ACCOUNT.to_owned(),
            profile_id: library.profile_id.clone(),
            batch_id: batch.id.clone(),
            expected_revision: organized.revision,
            item_id: answer.id.clone(),
            staged_role: "answer".to_owned(),
            now_utc_ms: 60,
        },
    )
    .expect("stage answer");
    let created = merge_capture_card(
        &mut connection,
        MergeCaptureCard {
            account_id: ACCOUNT.to_owned(),
            profile_id: library.profile_id.clone(),
            batch_id: batch.id.clone(),
            expected_revision: staged.batch.revision,
            target_draft_id: None,
            item_ids: vec![question.id.clone()],
            new_draft_subject: Some("physics".to_owned()),
            now_utc_ms: 61,
        },
    )
    .expect("create card with inherited subject");
    let draft_id = created.drafts[0].id.clone();
    let ready = merge_capture_card(
        &mut connection,
        MergeCaptureCard {
            account_id: ACCOUNT.to_owned(),
            profile_id: library.profile_id.clone(),
            batch_id: batch.id.clone(),
            expected_revision: created.batch.revision,
            target_draft_id: Some(draft_id.clone()),
            item_ids: vec![answer.id.clone()],
            new_draft_subject: None,
            now_utc_ms: 62,
        },
    )
    .expect("attach answer");

    assert_eq!(ready.drafts[0].subject, "physics");
    assert!(ready.drafts[0].ready);

    let undone = delete_capture_draft(
        &mut connection,
        ACCOUNT,
        &library.profile_id,
        &batch.id,
        &draft_id,
        ready.batch.revision,
        63,
    )
    .expect("undo card");
    assert!(undone.drafts.is_empty());
    assert_eq!(undone.unassigned_item_ids.len(), 2);
    assert!(undone.unassigned_item_ids.contains(&question.id));
    assert!(undone.unassigned_item_ids.contains(&answer.id));

    let counts: (i64, i64, i64) = connection
        .query_row(
            "SELECT (SELECT COUNT(*) FROM capture_items WHERE batch_id = ?1),
                    (SELECT COUNT(*) FROM capture_draft_items),
                    (SELECT COUNT(*) FROM assets WHERE account_id = ?2)",
            rusqlite::params![batch.id, ACCOUNT],
            |row| Ok((row.get(0)?, row.get(1)?, row.get(2)?)),
        )
        .expect("count retained data");
    assert_eq!(counts, (2, 0, 2));
}

#[test]
fn assigning_a_batch_subject_clears_card_overrides_and_updates_readiness_once() {
    let library = TestLibrary::new();
    let mut connection = library.open();
    let batch = create_batch(&library, &mut connection, "", CaptureBatchState::Collecting);
    ingest(
        &library,
        &mut connection,
        &batch.id,
        "bulk-subject-question",
        41,
        20,
    );
    ingest(
        &library,
        &mut connection,
        &batch.id,
        "bulk-subject-answer",
        42,
        21,
    );
    let revision = organize(&library, &mut connection, &batch.id, batch.revision + 2);
    let laid_out = apply_capture_layout(
        &mut connection,
        ApplyCaptureLayout {
            account_id: ACCOUNT.to_owned(),
            profile_id: library.profile_id.clone(),
            batch_id: batch.id.clone(),
            expected_revision: revision,
            mode: CaptureLayoutMode::Alternating,
            question_images_per_draft: 1,
            answer_images_per_draft: 1,
            split_index: None,
            now_utc_ms: 60,
        },
    )
    .expect("lay out one card");
    let draft_id = laid_out.drafts[0].id.clone();
    let overridden = update_capture_draft(
        &mut connection,
        UpdateCaptureDraft {
            account_id: ACCOUNT.to_owned(),
            profile_id: library.profile_id.clone(),
            batch_id: batch.id.clone(),
            expected_revision: laid_out.batch.revision,
            draft_id: draft_id.clone(),
            subject: "物理".to_owned(),
            tags: vec![],
            note: String::new(),
            now_utc_ms: 61,
        },
    )
    .expect("override subject");

    let assigned = assign_capture_batch_subject(
        &mut connection,
        ACCOUNT,
        &library.profile_id,
        &batch.id,
        overridden.batch.revision,
        "化学",
        62,
    )
    .expect("assign batch subject");

    assert_eq!(assigned.batch.subject, "化学");
    assert_eq!(assigned.batch.revision, overridden.batch.revision + 1);
    assert_eq!(assigned.drafts[0].subject, "化学");
    assert!(assigned.drafts[0].ready);
    let override_is_null: i64 = connection
        .query_row(
            "SELECT subject_override IS NULL FROM capture_drafts WHERE id = ?1",
            [draft_id],
            |row| row.get(0),
        )
        .unwrap();
    assert_eq!(override_is_null, 1);
}

#[test]
fn deleting_a_card_compacts_remaining_positions_and_keeps_all_images() {
    let library = TestLibrary::new();
    let mut connection = library.open();
    let batch = create_batch(
        &library,
        &mut connection,
        "math",
        CaptureBatchState::Collecting,
    );
    for index in 0..4 {
        ingest(
            &library,
            &mut connection,
            &batch.id,
            &format!("delete-position-{index}"),
            40 + index,
            20 + i64::from(index),
        );
    }
    let collected =
        get_capture_batch_detail(&connection, ACCOUNT, &library.profile_id, &batch.id).unwrap();
    let revision = organize(
        &library,
        &mut connection,
        &batch.id,
        collected.batch.revision,
    );
    let arranged = apply_capture_layout(
        &mut connection,
        ApplyCaptureLayout {
            account_id: ACCOUNT.to_owned(),
            profile_id: library.profile_id.clone(),
            batch_id: batch.id.clone(),
            expected_revision: revision,
            mode: CaptureLayoutMode::Alternating,
            question_images_per_draft: 1,
            answer_images_per_draft: 1,
            split_index: None,
            now_utc_ms: 60,
        },
    )
    .expect("arrange two cards");
    let first_draft_id = arranged.drafts[0].id.clone();
    let second_draft_id = arranged.drafts[1].id.clone();
    let deleted_item_ids = [
        arranged.drafts[0].question_item_ids[0].clone(),
        arranged.drafts[0].answer_item_ids[0].clone(),
    ];

    let undone = delete_capture_draft(
        &mut connection,
        ACCOUNT,
        &library.profile_id,
        &batch.id,
        &first_draft_id,
        arranged.batch.revision,
        61,
    )
    .expect("delete first card");

    assert_eq!(undone.drafts.len(), 1);
    assert_eq!(undone.drafts[0].id, second_draft_id);
    assert_eq!(undone.drafts[0].position, 0);
    assert_eq!(undone.items.len(), 4);
    assert_eq!(undone.unassigned_item_ids.len(), 2);
    assert!(
        deleted_item_ids
            .iter()
            .all(|item_id| undone.unassigned_item_ids.contains(item_id))
    );
}

#[test]
fn encrypted_items_survive_restart_and_upload_retries_are_idempotent() {
    let library = TestLibrary::new();
    let batch_id;
    let item_id;
    {
        let mut connection = library.open();
        let batch = create_batch(
            &library,
            &mut connection,
            "math",
            CaptureBatchState::Collecting,
        );
        batch_id = batch.id;
        let item = ingest(&library, &mut connection, &batch_id, "upload-1", 11, 20);
        item_id = item.id.clone();
        let retried = ingest(&library, &mut connection, &batch_id, "upload-1", 99, 21);
        assert_eq!(
            retried.id, item_id,
            "same client id must not create a second item"
        );
        assert_eq!(retried.source_name, "photo-11.png");
    }

    let connection = library.open();
    let detail = get_capture_batch_detail(&connection, ACCOUNT, &library.profile_id, &batch_id)
        .expect("restore batch after reopening");
    assert_eq!((detail.items.len(), detail.batch.item_count), (1, 1));
    let preview = get_capture_item_preview(
        &connection,
        &library.blob_root(),
        &ASSET_KEY,
        ACCOUNT,
        &library.profile_id,
        &batch_id,
        &item_id,
    )
    .expect("decrypt preview");
    assert!(preview.data_url.starts_with("data:image/png;base64,"));
}

#[test]
fn deterministic_layouts_preserve_manual_work_and_report_revision_conflicts() {
    let library = TestLibrary::new();
    let mut connection = library.open();
    let batch = create_batch(
        &library,
        &mut connection,
        "math",
        CaptureBatchState::Collecting,
    );
    let batch_id = batch.id;
    for index in 0..5 {
        ingest(
            &library,
            &mut connection,
            &batch_id,
            &format!("upload-{index}"),
            20 + index,
            20 + i64::from(index),
        );
    }
    let collecting =
        get_capture_batch_detail(&connection, ACCOUNT, &library.profile_id, &batch_id).unwrap();
    let revision = organize(
        &library,
        &mut connection,
        &batch_id,
        collecting.batch.revision,
    );
    let alternating = apply_capture_layout(
        &mut connection,
        ApplyCaptureLayout {
            account_id: ACCOUNT.to_owned(),
            profile_id: library.profile_id.clone(),
            batch_id: batch_id.clone(),
            expected_revision: revision,
            mode: CaptureLayoutMode::Alternating,
            question_images_per_draft: 1,
            answer_images_per_draft: 1,
            split_index: None,
            now_utc_ms: 60,
        },
    )
    .unwrap();
    assert_eq!(alternating.drafts.len(), 3);
    assert!(alternating.drafts[0].ready);
    assert!(alternating.drafts[1].ready);
    assert!(!alternating.drafts[2].ready);

    let moved_item = alternating.drafts[0].answer_item_ids[0].clone();
    let moved = move_capture_item(
        &mut connection,
        MoveCaptureItem {
            account_id: ACCOUNT.to_owned(),
            profile_id: library.profile_id.clone(),
            batch_id: batch_id.clone(),
            expected_revision: alternating.batch.revision,
            item_id: moved_item,
            target_draft_id: Some(alternating.drafts[1].id.clone()),
            target_role: Some("question".to_owned()),
            target_position: 1,
            now_utc_ms: 70,
        },
    )
    .unwrap();
    assert_eq!(moved.drafts[1].question_item_ids.len(), 2);

    ingest(&library, &mut connection, &batch_id, "late-upload", 42, 80);
    let after_upload =
        get_capture_batch_detail(&connection, ACCOUNT, &library.profile_id, &batch_id).unwrap();
    assert_eq!(after_upload.drafts[1].question_item_ids.len(), 2);
    assert_eq!(after_upload.unassigned_item_ids.len(), 1);

    let stale = move_capture_item(
        &mut connection,
        MoveCaptureItem {
            account_id: ACCOUNT.to_owned(),
            profile_id: library.profile_id.clone(),
            batch_id,
            expected_revision: moved.batch.revision,
            item_id: after_upload.unassigned_item_ids[0].clone(),
            target_draft_id: None,
            target_role: None,
            target_position: 0,
            now_utc_ms: 90,
        },
    )
    .expect_err("stale organizer must not overwrite a newer upload");
    assert!(matches!(stale, CaptureInboxError::RevisionConflict));
}

#[test]
fn split_questions_only_and_manual_layouts_never_drop_images() {
    let library = TestLibrary::new();
    let mut connection = library.open();
    let batch = create_batch(
        &library,
        &mut connection,
        "math",
        CaptureBatchState::Collecting,
    );
    for index in 0..5 {
        ingest(
            &library,
            &mut connection,
            &batch.id,
            &format!("layout-{index}"),
            45 + index,
            20 + i64::from(index),
        );
    }
    let collected =
        get_capture_batch_detail(&connection, ACCOUNT, &library.profile_id, &batch.id).unwrap();
    let revision = organize(
        &library,
        &mut connection,
        &batch.id,
        collected.batch.revision,
    );
    let split = apply_capture_layout(
        &mut connection,
        ApplyCaptureLayout {
            account_id: ACCOUNT.to_owned(),
            profile_id: library.profile_id.clone(),
            batch_id: batch.id.clone(),
            expected_revision: revision,
            mode: CaptureLayoutMode::Split,
            question_images_per_draft: 1,
            answer_images_per_draft: 1,
            split_index: Some(3),
            now_utc_ms: 60,
        },
    )
    .unwrap();
    assert_eq!(split.items.len(), 5);
    assert_eq!(split.drafts.len(), 3);
    assert_eq!(split.drafts.iter().filter(|draft| draft.ready).count(), 2);

    let questions_only = apply_capture_layout(
        &mut connection,
        ApplyCaptureLayout {
            account_id: ACCOUNT.to_owned(),
            profile_id: library.profile_id.clone(),
            batch_id: batch.id.clone(),
            expected_revision: split.batch.revision,
            mode: CaptureLayoutMode::QuestionsOnly,
            question_images_per_draft: 1,
            answer_images_per_draft: 1,
            split_index: None,
            now_utc_ms: 70,
        },
    )
    .unwrap();
    assert_eq!(questions_only.items.len(), 5);
    assert_eq!(questions_only.drafts.len(), 5);
    assert!(questions_only.drafts.iter().all(|draft| !draft.ready));

    let manual = apply_capture_layout(
        &mut connection,
        ApplyCaptureLayout {
            account_id: ACCOUNT.to_owned(),
            profile_id: library.profile_id.clone(),
            batch_id: batch.id,
            expected_revision: questions_only.batch.revision,
            mode: CaptureLayoutMode::Manual,
            question_images_per_draft: 1,
            answer_images_per_draft: 1,
            split_index: None,
            now_utc_ms: 80,
        },
    )
    .unwrap();
    assert_eq!(manual.items.len(), 5);
    assert!(manual.drafts.is_empty());
    assert_eq!(manual.unassigned_item_ids.len(), 5);
}

#[test]
fn ready_drafts_commit_atomically_and_incomplete_drafts_remain() {
    let library = TestLibrary::new();
    let mut connection = library.open();
    let batch = create_batch(
        &library,
        &mut connection,
        "math",
        CaptureBatchState::Collecting,
    );
    let batch_id = batch.id;
    for index in 0..5 {
        ingest(
            &library,
            &mut connection,
            &batch_id,
            &format!("commit-{index}"),
            60 + index,
            20 + i64::from(index),
        );
    }
    let detail =
        get_capture_batch_detail(&connection, ACCOUNT, &library.profile_id, &batch_id).unwrap();
    let revision = organize(&library, &mut connection, &batch_id, detail.batch.revision);
    let arranged = apply_capture_layout(
        &mut connection,
        ApplyCaptureLayout {
            account_id: ACCOUNT.to_owned(),
            profile_id: library.profile_id.clone(),
            batch_id: batch_id.clone(),
            expected_revision: revision,
            mode: CaptureLayoutMode::Alternating,
            question_images_per_draft: 1,
            answer_images_per_draft: 1,
            split_index: None,
            now_utc_ms: 60,
        },
    )
    .unwrap();

    let report = commit_ready_capture_drafts(
        &mut connection,
        ACCOUNT,
        &library.profile_id,
        &batch_id,
        arranged.batch.revision,
        100,
    )
    .expect("commit ready drafts");
    assert_eq!(
        (report.committed_count, report.remaining_draft_count),
        (2, 1)
    );
    let counts: (i64, i64, i64, i64) = connection
        .query_row(
            "SELECT (SELECT COUNT(*) FROM problems),
                    (SELECT COUNT(*) FROM problem_assets),
                    (SELECT COUNT(*) FROM capture_items),
                    (SELECT COUNT(*) FROM sync_operations WHERE entity_type = 'problem')",
            [],
            |row| Ok((row.get(0)?, row.get(1)?, row.get(2)?, row.get(3)?)),
        )
        .unwrap();
    assert_eq!(counts, (2, 4, 1, 2));

    let failure_batch = create_batch(
        &library,
        &mut connection,
        "fail",
        CaptureBatchState::Collecting,
    );
    ingest(
        &library,
        &mut connection,
        &failure_batch.id,
        "fail-q",
        80,
        110,
    );
    ingest(
        &library,
        &mut connection,
        &failure_batch.id,
        "fail-a",
        81,
        111,
    );
    let failure_detail =
        get_capture_batch_detail(&connection, ACCOUNT, &library.profile_id, &failure_batch.id)
            .unwrap();
    let failure_revision = update_capture_batch(
        &mut connection,
        ACCOUNT,
        &library.profile_id,
        &failure_batch.id,
        failure_detail.batch.revision,
        "fail",
        true,
        112,
    )
    .unwrap()
    .revision;
    let failure_arranged = apply_capture_layout(
        &mut connection,
        ApplyCaptureLayout {
            account_id: ACCOUNT.to_owned(),
            profile_id: library.profile_id.clone(),
            batch_id: failure_batch.id.clone(),
            expected_revision: failure_revision,
            mode: CaptureLayoutMode::Alternating,
            question_images_per_draft: 1,
            answer_images_per_draft: 1,
            split_index: None,
            now_utc_ms: 113,
        },
    )
    .unwrap();
    connection
        .execute_batch(
            "CREATE TEMP TRIGGER reject_capture_problem BEFORE INSERT ON problems
             WHEN NEW.subject = 'fail' BEGIN SELECT RAISE(ABORT, 'injected failure'); END;",
        )
        .unwrap();
    let before: (i64, i64) = connection
        .query_row(
            "SELECT (SELECT COUNT(*) FROM problems), (SELECT COUNT(*) FROM capture_drafts)",
            [],
            |row| Ok((row.get(0)?, row.get(1)?)),
        )
        .unwrap();
    assert!(
        commit_ready_capture_drafts(
            &mut connection,
            ACCOUNT,
            &library.profile_id,
            &failure_batch.id,
            failure_arranged.batch.revision,
            120,
        )
        .is_err()
    );
    let after: (i64, i64) = connection
        .query_row(
            "SELECT (SELECT COUNT(*) FROM problems), (SELECT COUNT(*) FROM capture_drafts)",
            [],
            |row| Ok((row.get(0)?, row.get(1)?)),
        )
        .unwrap();
    assert_eq!(
        after, before,
        "failed commit must roll back every draft and problem"
    );
}

#[test]
fn discarding_a_batch_only_removes_truly_orphaned_assets() {
    let library = TestLibrary::new();
    let mut connection = library.open();
    let first = create_batch(
        &library,
        &mut connection,
        "math",
        CaptureBatchState::Collecting,
    );
    let second = create_batch(
        &library,
        &mut connection,
        "math",
        CaptureBatchState::Collecting,
    );
    ingest(&library, &mut connection, &first.id, "first", 91, 20);
    ingest(&library, &mut connection, &second.id, "second", 91, 21);
    let asset_path: String = connection
        .query_row("SELECT encrypted_path FROM assets", [], |row| row.get(0))
        .unwrap();

    discard_capture_batch(
        &mut connection,
        &library.blob_root(),
        ACCOUNT,
        &library.profile_id,
        &first.id,
    )
    .unwrap();
    assert_eq!(
        connection
            .query_row("SELECT COUNT(*) FROM assets", [], |row| row
                .get::<_, i64>(0))
            .unwrap(),
        1
    );
    assert!(library.blob_root().join(&asset_path).exists());

    discard_capture_batch(
        &mut connection,
        &library.blob_root(),
        ACCOUNT,
        &library.profile_id,
        &second.id,
    )
    .unwrap();
    assert_eq!(
        connection
            .query_row("SELECT COUNT(*) FROM assets", [], |row| row
                .get::<_, i64>(0))
            .unwrap(),
        0
    );
    assert!(!library.blob_root().join(asset_path).exists());
}
