use std::{
    collections::{HashMap, HashSet},
    path::{Path, PathBuf},
    sync::{
        Arc, Mutex as StdMutex,
        atomic::{AtomicBool, Ordering},
    },
};

use rusqlite::Connection;
use serde::Serialize;
use thiserror::Error;
use uuid::Uuid;

use crate::{
    infrastructure::{
        assets::decrypt_asset,
        recognition_product::{
            PRODUCT_ENGINE_NAME, PRODUCT_ENGINE_VERSION, ProductRecognitionEngine,
        },
    },
    modules::{
        capture_inbox::read_encrypted_blob,
        capture_recognition::{
            CaptureRecognitionError, CaptureRecognitionJob, CaptureRecognitionReasonCode,
            CaptureRecognitionRegionProposal, CaptureRecognitionRole,
            StoreCaptureRecognitionSuggestion, cancel_recognition_job, capture_item_snapshot_hash,
            claim_next_recognition_item, fail_recognition_job,
            finish_recognition_item_without_suggestion, get_recognition_job_by_id,
            store_recognition_suggestion,
        },
    },
};

#[derive(Clone, Debug, PartialEq)]
pub struct RecognitionAnalysis {
    pub regions: Vec<CaptureRecognitionRegionProposal>,
    pub confidence_basis_points: u16,
    pub reason_codes: Vec<CaptureRecognitionReasonCode>,
    /// Transient recognized anchors aligned with `regions`. They are mapped
    /// to job-scoped opaque slots immediately before persistence and are
    /// never written to the database, logs, events, or frontend state.
    pub pairing_tokens: Vec<Option<u16>>,
}

#[derive(Debug, Error)]
pub enum RecognitionEngineError {
    #[error("the recognition model is not available")]
    ModelMissing,
    #[error("the recognition runtime is not available")]
    RuntimeUnavailable,
    #[error("the recognition result was invalid")]
    InvalidResult,
    #[error("the recognition engine failed")]
    Failed,
}

pub trait CaptureRecognitionEngine: Send + Sync {
    fn analyze(
        &self,
        image_path: &Path,
        staged_role: CaptureRecognitionRole,
    ) -> Result<RecognitionAnalysis, RecognitionEngineError>;
}

struct UnavailableRecognitionEngine;

impl CaptureRecognitionEngine for UnavailableRecognitionEngine {
    fn analyze(
        &self,
        _image_path: &Path,
        _staged_role: CaptureRecognitionRole,
    ) -> Result<RecognitionAnalysis, RecognitionEngineError> {
        Err(RecognitionEngineError::ModelMissing)
    }
}

#[derive(Clone)]
pub struct CaptureRecognitionWorkerContext {
    pub connection: Arc<StdMutex<Connection>>,
    pub account_id: String,
    pub profile_id: String,
    pub batch_id: String,
    pub job_id: String,
    pub blob_root: PathBuf,
    pub private_temp_root: PathBuf,
    pub asset_key: [u8; 32],
}

#[derive(Clone, Debug, Serialize, PartialEq, Eq)]
#[serde(rename_all = "camelCase")]
pub struct CaptureRecognitionChanged {
    pub job_id: String,
    pub batch_id: String,
    pub state: String,
    pub processed_items: u32,
    pub total_items: u32,
}

pub type CaptureRecognitionEventSink = Arc<dyn Fn(CaptureRecognitionChanged) + Send + Sync>;

#[derive(Debug, Error)]
pub enum CaptureRecognitionWorkerError {
    #[error("capture recognition worker state lock failed")]
    Lock,
    #[error("capture recognition worker task failed")]
    Join,
    #[error("capture recognition worker filesystem failed")]
    Io(#[from] std::io::Error),
    #[error("capture recognition worker database operation failed")]
    Recognition(#[from] CaptureRecognitionError),
}

#[derive(Clone)]
pub struct CaptureRecognitionManager {
    mutation: Arc<tokio::sync::Mutex<()>>,
    cancellation: Arc<tokio::sync::Mutex<HashMap<String, Arc<AtomicBool>>>>,
    running: Arc<tokio::sync::Mutex<HashSet<String>>>,
    engine: Arc<dyn CaptureRecognitionEngine>,
    engine_execution: Arc<StdMutex<()>>,
    product_engine_configured: bool,
    engine_name: &'static str,
    engine_version: &'static str,
}

impl Default for CaptureRecognitionManager {
    fn default() -> Self {
        Self {
            mutation: Arc::new(tokio::sync::Mutex::new(())),
            cancellation: Arc::new(tokio::sync::Mutex::new(HashMap::new())),
            running: Arc::new(tokio::sync::Mutex::new(HashSet::new())),
            engine: Arc::new(UnavailableRecognitionEngine),
            engine_execution: Arc::new(StdMutex::new(())),
            product_engine_configured: false,
            engine_name: "unavailable",
            engine_version: "0",
        }
    }
}

impl CaptureRecognitionManager {
    /// Creates the product worker. The optional OCR runtime is initialized
    /// only on the first recognition request; startup never loads a model or
    /// DLL and missing components transparently retain safe visual review.
    pub fn for_product(
        control_root: &Path,
        resource_root: &Path,
        private_temp_root: &Path,
    ) -> Self {
        let Some(engine) =
            product_recognition_engine(control_root, resource_root, private_temp_root)
        else {
            return Self::default();
        };
        Self {
            mutation: Arc::new(tokio::sync::Mutex::new(())),
            cancellation: Arc::new(tokio::sync::Mutex::new(HashMap::new())),
            running: Arc::new(tokio::sync::Mutex::new(HashSet::new())),
            engine,
            engine_execution: Arc::new(StdMutex::new(())),
            product_engine_configured: true,
            engine_name: PRODUCT_ENGINE_NAME,
            engine_version: PRODUCT_ENGINE_VERSION,
        }
    }

    pub fn with_engine(engine: Arc<dyn CaptureRecognitionEngine>) -> Self {
        Self {
            mutation: Arc::new(tokio::sync::Mutex::new(())),
            cancellation: Arc::new(tokio::sync::Mutex::new(HashMap::new())),
            running: Arc::new(tokio::sync::Mutex::new(HashSet::new())),
            engine,
            engine_execution: Arc::new(StdMutex::new(())),
            product_engine_configured: true,
            engine_name: "test",
            engine_version: "1",
        }
    }

    pub const fn product_engine_configured(&self) -> bool {
        self.product_engine_configured
    }

    pub const fn engine_identity(&self) -> (&'static str, &'static str) {
        (self.engine_name, self.engine_version)
    }

    pub async fn lock_mutation(&self) -> tokio::sync::MutexGuard<'_, ()> {
        self.mutation.lock().await
    }

    pub async fn cancellation_flag(&self, job_id: &str) -> Arc<AtomicBool> {
        let mut cancellation = self.cancellation.lock().await;
        cancellation
            .entry(job_id.to_owned())
            .or_insert_with(|| Arc::new(AtomicBool::new(false)))
            .clone()
    }

    pub async fn cancel(&self, job_id: &str) {
        self.cancellation_flag(job_id)
            .await
            .store(true, Ordering::Release);
    }

    pub async fn clear(&self, job_id: &str) {
        self.cancellation.lock().await.remove(job_id);
    }

    pub async fn shutdown(&self) {
        for flag in self.cancellation.lock().await.values() {
            flag.store(true, Ordering::Release);
        }
    }

    pub async fn wait_for_idle(&self, timeout: std::time::Duration) -> bool {
        let deadline = tokio::time::Instant::now() + timeout;
        loop {
            if self.running.lock().await.is_empty() {
                return true;
            }
            if tokio::time::Instant::now() >= deadline {
                return false;
            }
            tokio::time::sleep(std::time::Duration::from_millis(10)).await;
        }
    }

    pub fn engine(&self) -> Arc<dyn CaptureRecognitionEngine> {
        Arc::clone(&self.engine)
    }

    pub async fn run_job(
        &self,
        context: CaptureRecognitionWorkerContext,
        emit: CaptureRecognitionEventSink,
    ) -> Result<Option<CaptureRecognitionJob>, CaptureRecognitionWorkerError> {
        {
            let mut running = self.running.lock().await;
            if !running.insert(context.job_id.clone()) {
                return Ok(None);
            }
        }
        let job_id = context.job_id.clone();
        let cancellation = self.cancellation_flag(&job_id).await;
        cancellation.store(false, Ordering::Release);
        let engine = Arc::clone(&self.engine);
        let engine_execution = Arc::clone(&self.engine_execution);
        let joined = tokio::task::spawn_blocking(move || {
            run_recognition_job_sync(context, engine, engine_execution, cancellation, emit)
        })
        .await;
        self.running.lock().await.remove(&job_id);
        self.clear(&job_id).await;
        match joined {
            Ok(result) => result,
            Err(_) => Err(CaptureRecognitionWorkerError::Join),
        }
    }
}

fn product_recognition_engine(
    control_root: &Path,
    resource_root: &Path,
    private_temp_root: &Path,
) -> Option<Arc<dyn CaptureRecognitionEngine>> {
    Some(Arc::new(ProductRecognitionEngine::new(
        control_root,
        resource_root,
        private_temp_root,
    )))
}

fn run_recognition_job_sync(
    context: CaptureRecognitionWorkerContext,
    engine: Arc<dyn CaptureRecognitionEngine>,
    engine_execution: Arc<StdMutex<()>>,
    cancellation: Arc<AtomicBool>,
    emit: CaptureRecognitionEventSink,
) -> Result<Option<CaptureRecognitionJob>, CaptureRecognitionWorkerError> {
    std::fs::create_dir_all(&context.private_temp_root)?;
    let mut pairing_slots = PairingSlotAllocator::default();
    loop {
        if cancellation.load(Ordering::Acquire) {
            return cancel_worker_job(&context, &emit);
        }
        let claimed = {
            let mut connection = context
                .connection
                .lock()
                .map_err(|_| CaptureRecognitionWorkerError::Lock)?;
            match claim_next_recognition_item(
                &mut connection,
                &context.account_id,
                &context.profile_id,
                &context.job_id,
                current_utc_millis(),
            ) {
                Ok(item) => item,
                Err(CaptureRecognitionError::JobNotFound) => return Ok(None),
                Err(error) => return Err(error.into()),
            }
        };
        let Some(claimed) = claimed else {
            let job = current_worker_job(&context)?;
            if let Some(job) = &job {
                emit(job_event(job));
            }
            return Ok(job);
        };
        if claimed.batch_id != context.batch_id {
            return Err(CaptureRecognitionError::InvalidInput.into());
        }

        let snapshot_matches = {
            let connection = context
                .connection
                .lock()
                .map_err(|_| CaptureRecognitionWorkerError::Lock)?;
            capture_item_snapshot_hash(
                &connection,
                &context.account_id,
                &context.profile_id,
                &context.batch_id,
                &claimed.item_id,
            )
            .map(|snapshot| snapshot == claimed.source_snapshot_hash)
            .unwrap_or(false)
        };
        if !snapshot_matches {
            let job = finish_worker_item(&context, &claimed.item_id, "stale")?;
            emit(job_event(&job));
            continue;
        }

        let job_directory =
            context
                .private_temp_root
                .join(format!("{}-{}", context.job_id, Uuid::now_v7()));
        std::fs::create_dir(&job_directory)?;
        let _cleanup = PrivateJobDirectory(job_directory.clone());
        let extension = match claimed.media_type.as_str() {
            "image/jpeg" => "jpg",
            "image/png" => "png",
            "image/webp" => "webp",
            _ => {
                let job = finish_worker_item(&context, &claimed.item_id, "failed")?;
                emit(job_event(&job));
                continue;
            }
        };
        let image_path = job_directory.join(format!("input.{extension}"));
        let encrypted = match read_encrypted_blob(&context.blob_root, &claimed.encrypted_path) {
            Ok(encrypted) => encrypted,
            Err(_) => {
                let job = finish_worker_item(&context, &claimed.item_id, "failed")?;
                emit(job_event(&job));
                continue;
            }
        };
        let mut plaintext = match decrypt_asset(&encrypted, &context.asset_key) {
            Ok(plaintext) => plaintext,
            Err(_) => {
                let job = finish_worker_item(&context, &claimed.item_id, "failed")?;
                emit(job_event(&job));
                continue;
            }
        };
        let write_result = std::fs::write(&image_path, &plaintext);
        plaintext.fill(0);
        write_result?;
        if cancellation.load(Ordering::Acquire) {
            return cancel_worker_job(&context, &emit);
        }

        let analysis = {
            let _engine_guard = engine_execution
                .lock()
                .map_err(|_| CaptureRecognitionWorkerError::Lock)?;
            engine.analyze(&image_path, claimed.staged_role)
        };
        let analysis = analysis.and_then(|mut analysis| {
            pairing_slots.resolve(&mut analysis)?;
            Ok(analysis)
        });
        if cancellation.load(Ordering::Acquire) {
            return cancel_worker_job(&context, &emit);
        }
        match analysis {
            Ok(analysis) if analysis.regions.is_empty() => {
                let job = finish_worker_item(&context, &claimed.item_id, "no_suggestion")?;
                emit(job_event(&job));
            }
            Ok(analysis) => {
                let result = {
                    let mut connection = context
                        .connection
                        .lock()
                        .map_err(|_| CaptureRecognitionWorkerError::Lock)?;
                    store_recognition_suggestion(
                        &mut connection,
                        StoreCaptureRecognitionSuggestion {
                            account_id: context.account_id.clone(),
                            profile_id: context.profile_id.clone(),
                            job_id: context.job_id.clone(),
                            item_id: claimed.item_id.clone(),
                            regions: analysis.regions,
                            confidence_basis_points: analysis.confidence_basis_points,
                            reason_codes: analysis.reason_codes,
                            now_utc_ms: current_utc_millis(),
                        },
                    )
                };
                match result {
                    Ok(job) => emit(job_event(&job)),
                    Err(CaptureRecognitionError::InvalidSuggestion) => {
                        let job = finish_worker_item(&context, &claimed.item_id, "failed")?;
                        emit(job_event(&job));
                    }
                    Err(CaptureRecognitionError::ItemNotFound)
                        if current_worker_job(&context)?.is_none() =>
                    {
                        return Ok(None);
                    }
                    Err(CaptureRecognitionError::JobNotFound) => return Ok(None),
                    Err(error) => return Err(error.into()),
                }
            }
            Err(
                error @ (RecognitionEngineError::ModelMissing
                | RecognitionEngineError::RuntimeUnavailable),
            ) => {
                let failure_code = match error {
                    RecognitionEngineError::ModelMissing => "model_missing",
                    RecognitionEngineError::RuntimeUnavailable => "runtime_unavailable",
                    RecognitionEngineError::InvalidResult | RecognitionEngineError::Failed => {
                        unreachable!("pattern contains only global availability failures")
                    }
                };
                let job = {
                    let mut connection = context
                        .connection
                        .lock()
                        .map_err(|_| CaptureRecognitionWorkerError::Lock)?;
                    fail_recognition_job(
                        &mut connection,
                        &context.account_id,
                        &context.profile_id,
                        &context.job_id,
                        failure_code,
                        current_utc_millis(),
                    )?
                };
                emit(job_event(&job));
                return Ok(Some(job));
            }
            Err(RecognitionEngineError::InvalidResult | RecognitionEngineError::Failed) => {
                let job = finish_worker_item(&context, &claimed.item_id, "failed")?;
                emit(job_event(&job));
            }
        }
    }
}

#[derive(Default)]
struct PairingSlotAllocator {
    occurrences: HashMap<u16, Vec<PairingOccurrence>>,
    next_slot: u32,
}

struct PairingOccurrence {
    slot: u32,
    has_question: bool,
    has_answer: bool,
}

impl PairingSlotAllocator {
    fn resolve(
        &mut self,
        analysis: &mut RecognitionAnalysis,
    ) -> Result<(), RecognitionEngineError> {
        if analysis.pairing_tokens.is_empty() {
            return Ok(());
        }
        if analysis.pairing_tokens.len() != analysis.regions.len()
            || analysis
                .regions
                .iter()
                .any(|region| region.group_slot.is_some())
        {
            return Err(RecognitionEngineError::InvalidResult);
        }
        let mut matched = false;
        for (region, token) in analysis
            .regions
            .iter_mut()
            .zip(analysis.pairing_tokens.iter().copied())
        {
            let Some(token) = token else {
                continue;
            };
            let (slot, completed_pair) = self.slot_for(token, region.role)?;
            region.group_slot = Some(slot);
            matched |= completed_pair;
        }
        if matched
            && !analysis
                .reason_codes
                .contains(&CaptureRecognitionReasonCode::MatchedQuestionAnswerAnchor)
        {
            analysis
                .reason_codes
                .push(CaptureRecognitionReasonCode::MatchedQuestionAnswerAnchor);
        }
        Ok(())
    }

    fn slot_for(
        &mut self,
        token: u16,
        role: CaptureRecognitionRole,
    ) -> Result<(u32, bool), RecognitionEngineError> {
        let occurrences = self.occurrences.entry(token).or_default();
        if let Some(occurrence) = occurrences.iter_mut().find(|occurrence| match role {
            CaptureRecognitionRole::Question => !occurrence.has_question,
            CaptureRecognitionRole::Answer => !occurrence.has_answer,
        }) {
            match role {
                CaptureRecognitionRole::Question => occurrence.has_question = true,
                CaptureRecognitionRole::Answer => occurrence.has_answer = true,
            }
            return Ok((
                occurrence.slot,
                occurrence.has_question && occurrence.has_answer,
            ));
        }
        if self.next_slot >= 150 {
            return Err(RecognitionEngineError::InvalidResult);
        }
        let slot = self.next_slot;
        self.next_slot += 1;
        occurrences.push(PairingOccurrence {
            slot,
            has_question: role == CaptureRecognitionRole::Question,
            has_answer: role == CaptureRecognitionRole::Answer,
        });
        Ok((slot, false))
    }
}

fn current_worker_job(
    context: &CaptureRecognitionWorkerContext,
) -> Result<Option<CaptureRecognitionJob>, CaptureRecognitionWorkerError> {
    let connection = context
        .connection
        .lock()
        .map_err(|_| CaptureRecognitionWorkerError::Lock)?;
    get_recognition_job_by_id(
        &connection,
        &context.account_id,
        &context.profile_id,
        &context.job_id,
    )
    .map_err(Into::into)
}

fn finish_worker_item(
    context: &CaptureRecognitionWorkerContext,
    item_id: &str,
    state: &str,
) -> Result<CaptureRecognitionJob, CaptureRecognitionWorkerError> {
    let mut connection = context
        .connection
        .lock()
        .map_err(|_| CaptureRecognitionWorkerError::Lock)?;
    finish_recognition_item_without_suggestion(
        &mut connection,
        &context.account_id,
        &context.profile_id,
        &context.job_id,
        item_id,
        state,
        current_utc_millis(),
    )
    .map_err(Into::into)
}

fn cancel_worker_job(
    context: &CaptureRecognitionWorkerContext,
    emit: &CaptureRecognitionEventSink,
) -> Result<Option<CaptureRecognitionJob>, CaptureRecognitionWorkerError> {
    let mut connection = context
        .connection
        .lock()
        .map_err(|_| CaptureRecognitionWorkerError::Lock)?;
    match cancel_recognition_job(
        &mut connection,
        &context.account_id,
        &context.profile_id,
        &context.job_id,
        current_utc_millis(),
    ) {
        Ok(job) => {
            emit(job_event(&job));
            Ok(Some(job))
        }
        Err(CaptureRecognitionError::JobNotFound) => Ok(None),
        Err(error) => Err(error.into()),
    }
}

fn job_event(job: &CaptureRecognitionJob) -> CaptureRecognitionChanged {
    CaptureRecognitionChanged {
        job_id: job.id.clone(),
        batch_id: job.batch_id.clone(),
        state: match job.state {
            crate::modules::capture_recognition::CaptureRecognitionJobState::Queued => "queued",
            crate::modules::capture_recognition::CaptureRecognitionJobState::Running => "running",
            crate::modules::capture_recognition::CaptureRecognitionJobState::Review => "review",
            crate::modules::capture_recognition::CaptureRecognitionJobState::Applied => "applied",
            crate::modules::capture_recognition::CaptureRecognitionJobState::Cancelled => {
                "cancelled"
            }
            crate::modules::capture_recognition::CaptureRecognitionJobState::Failed => "failed",
        }
        .to_owned(),
        processed_items: job.processed_items,
        total_items: job.total_items,
    }
}

struct PrivateJobDirectory(PathBuf);

impl Drop for PrivateJobDirectory {
    fn drop(&mut self) {
        let _ = std::fs::remove_dir_all(&self.0);
    }
}

fn current_utc_millis() -> i64 {
    std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .map(|duration| i64::try_from(duration.as_millis()).unwrap_or(i64::MAX))
        .unwrap_or_default()
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::modules::capture_inbox::NormalizedCropRect;

    fn analysis(role: CaptureRecognitionRole, tokens: &[u16]) -> RecognitionAnalysis {
        RecognitionAnalysis {
            regions: tokens
                .iter()
                .map(|_| CaptureRecognitionRegionProposal {
                    rect: NormalizedCropRect {
                        x: 0.0,
                        y: 0.0,
                        width: 1.0,
                        height: 1.0,
                    },
                    role,
                    group_slot: None,
                    confidence_basis_points: 9_100,
                })
                .collect(),
            confidence_basis_points: 9_100,
            reason_codes: vec![CaptureRecognitionReasonCode::ClearQuestionAnchor],
            pairing_tokens: tokens.iter().copied().map(Some).collect(),
        }
    }

    #[test]
    fn pairing_tokens_become_job_scoped_slots_and_matching_answers_reuse_them() {
        let mut allocator = PairingSlotAllocator::default();
        let mut questions = analysis(CaptureRecognitionRole::Question, &[7, 8]);
        let mut answers = analysis(CaptureRecognitionRole::Answer, &[7, 8]);

        allocator.resolve(&mut questions).unwrap();
        allocator.resolve(&mut answers).unwrap();

        assert_eq!(
            questions
                .regions
                .iter()
                .map(|region| region.group_slot)
                .collect::<Vec<_>>(),
            vec![Some(0), Some(1)]
        );
        assert_eq!(
            answers
                .regions
                .iter()
                .map(|region| region.group_slot)
                .collect::<Vec<_>>(),
            vec![Some(0), Some(1)]
        );
        assert!(
            answers
                .reason_codes
                .contains(&CaptureRecognitionReasonCode::MatchedQuestionAnswerAnchor)
        );
    }

    #[test]
    fn repeated_question_numbers_allocate_separate_occurrences_without_cross_pairing() {
        let mut allocator = PairingSlotAllocator::default();
        let mut first_questions = analysis(CaptureRecognitionRole::Question, &[1]);
        let mut second_questions = analysis(CaptureRecognitionRole::Question, &[1]);
        let mut first_answers = analysis(CaptureRecognitionRole::Answer, &[1]);
        let mut second_answers = analysis(CaptureRecognitionRole::Answer, &[1]);

        allocator.resolve(&mut first_questions).unwrap();
        allocator.resolve(&mut second_questions).unwrap();
        allocator.resolve(&mut first_answers).unwrap();
        allocator.resolve(&mut second_answers).unwrap();

        assert_eq!(first_questions.regions[0].group_slot, Some(0));
        assert_eq!(second_questions.regions[0].group_slot, Some(1));
        assert_eq!(first_answers.regions[0].group_slot, Some(0));
        assert_eq!(second_answers.regions[0].group_slot, Some(1));
    }

    #[test]
    fn pairing_contract_rejects_mixed_or_misaligned_engine_output() {
        let mut allocator = PairingSlotAllocator::default();
        let mut misaligned = analysis(CaptureRecognitionRole::Question, &[1]);
        misaligned.pairing_tokens.clear();
        misaligned.pairing_tokens.push(Some(1));
        misaligned.pairing_tokens.push(Some(2));
        assert!(matches!(
            allocator.resolve(&mut misaligned),
            Err(RecognitionEngineError::InvalidResult)
        ));

        let mut mixed = analysis(CaptureRecognitionRole::Question, &[1]);
        mixed.regions[0].group_slot = Some(44);
        assert!(matches!(
            allocator.resolve(&mut mixed),
            Err(RecognitionEngineError::InvalidResult)
        ));
    }

    #[test]
    fn product_manager_uses_the_built_in_visual_splitter_without_models() {
        let root = tempfile::tempdir().unwrap();
        let control_root = root.path().join("control");
        let private_temp_root = root.path().join("recognition-private-temp");
        let image_path = root.path().join("page.png");
        image::GrayImage::from_pixel(80, 80, image::Luma([255]))
            .save(&image_path)
            .unwrap();

        let resource_root = root.path().join("resources");
        let manager = CaptureRecognitionManager::for_product(
            &control_root,
            &resource_root,
            &private_temp_root,
        );

        assert!(manager.product_engine_configured());
        assert!(!control_root.exists());
        assert!(!private_temp_root.exists());
        let result = manager
            .engine
            .analyze(&image_path, CaptureRecognitionRole::Question)
            .unwrap();
        assert_eq!(result.regions.len(), 1);
        assert!(result.pairing_tokens.is_empty());
    }
}
