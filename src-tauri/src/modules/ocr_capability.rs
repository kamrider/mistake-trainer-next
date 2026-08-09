use std::{fs, path::Path};

use futures_util::StreamExt as _;
use serde::{Deserialize, Serialize};
use sha2::{Digest as _, Sha256};
use specta::Type;
use thiserror::Error;
use tokio::io::AsyncWriteExt as _;
use uuid::Uuid;

const GIB: u64 = 1024 * 1024 * 1024;
const MIB: u64 = 1024 * 1024;
const COMPONENT_DIRECTORY: &str = "optional-components";
const STAGING_DIRECTORY: &str = ".staging";
const MANIFEST_FILE: &str = "component.json";
const MANIFEST_VERSION: u32 = 1;
pub const RECOGNITION_EVIDENCE_GATE_PASSED: bool = false;
pub const RECOGNITION_RUNTIME_AVAILABLE: bool = cfg!(all(windows, target_arch = "x86_64"));
const DETAIL_GATE: &str = "智能分题仍在真实题图验证中；顺序模板和手工整理可继续使用。";
const DETAIL_RUNTIME: &str = "本机识别运行时尚未随应用发布；顺序模板和手工整理可继续使用。";
const DETAIL_MODEL: &str = "智能分题需要已校验的 PP‑OCRv6 small 本地模型。";
const DETAIL_READY: &str = "智能分题可在本机运行；结果只会作为待确认建议。";
const DETAIL_VISUAL_SPLIT_READY: &str =
    "基础版面预切可直接使用，不需要下载模型；安装题号定位增强后，多题试卷会优先按连续题号切分。";
const DETAIL_ANCHOR_SPLIT_READY: &str =
    "本地题号定位增强已就绪；题号文字只在内存中用于划分区域，不保存 OCR 文本，结果仍需确认。";

const SMALL_FILES: &[OcrComponentFile] = &[
    OcrComponentFile {
        name: "PP-OCRv6_det_small.onnx",
        url: "https://www.modelscope.cn/models/RapidAI/RapidOCR/resolve/v3.9.2/onnx/PP-OCRv6/det/PP-OCRv6_det_small.onnx",
        byte_length: 9_929_594,
        sha256: "090f04abcd9d9a7498bc4ebf677e4cb9bdce1fe4197ddb7e529f1ef44e1ff94f",
    },
    OcrComponentFile {
        name: "PP-OCRv6_rec_small.onnx",
        url: "https://www.modelscope.cn/models/RapidAI/RapidOCR/resolve/v3.9.2/onnx/PP-OCRv6/rec/PP-OCRv6_rec_small.onnx",
        byte_length: 21_234_383,
        sha256: "6f327246b50388f3c176ae304bd95767ea6dc0c9ae92153ef8cbe210b3c14884",
    },
];

const MEDIUM_FILES: &[OcrComponentFile] = &[
    OcrComponentFile {
        name: "PP-OCRv6_det_medium.onnx",
        url: "https://www.modelscope.cn/models/RapidAI/RapidOCR/resolve/v3.9.2/onnx/PP-OCRv6/det/PP-OCRv6_det_medium.onnx",
        byte_length: 62_119_454,
        sha256: "92078b7355007ccfffcd4c8cd441a3afd4538904d06881b29a155e1e679907c2",
    },
    OcrComponentFile {
        name: "PP-OCRv6_rec_medium.onnx",
        url: "https://www.modelscope.cn/models/RapidAI/RapidOCR/resolve/v3.9.2/onnx/PP-OCRv6/rec/PP-OCRv6_rec_medium.onnx",
        byte_length: 76_629_984,
        sha256: "eef444829dbbe18d7fea59a3f6eb75647518d2b3a9568d27c92e42940204894b",
    },
];

#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize, Type)]
#[serde(rename_all = "snake_case")]
pub enum OcrHardwareTier {
    ManualOnly,
    Basic,
    Balanced,
    Performance,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize, Type)]
#[serde(rename_all = "snake_case")]
pub enum OcrComponentId {
    Ppocrv6Small,
    Ppocrv6Medium,
    OpencvPreprocess,
}

impl OcrComponentId {
    pub const fn key(self) -> &'static str {
        match self {
            Self::Ppocrv6Small => "ppocrv6-small",
            Self::Ppocrv6Medium => "ppocrv6-medium",
            Self::OpencvPreprocess => "opencv-preprocess",
        }
    }
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Type)]
#[serde(rename_all = "snake_case")]
pub enum OcrComponentState {
    NotInstalled,
    Installed,
    Corrupt,
    Unavailable,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Type)]
#[serde(rename_all = "snake_case")]
pub enum OcrRecognitionFeatureState {
    EvidenceGatePending,
    RuntimeMissing,
    ModelMissing,
    Ready,
}

#[derive(Clone, Debug, PartialEq, Serialize, Type)]
#[serde(rename_all = "camelCase")]
pub struct OcrRecognitionFeatureStatus {
    pub state: OcrRecognitionFeatureState,
    pub required_component_id: OcrComponentId,
    pub detail: String,
}

#[derive(Clone, Debug, PartialEq, Serialize, Type)]
#[serde(rename_all = "camelCase")]
pub struct OcrHardwareAssessment {
    pub tier: OcrHardwareTier,
    pub logical_processor_count: u32,
    pub total_memory_mb: f64,
    pub available_component_storage_mb: f64,
    pub avx2_supported: bool,
    pub estimated_suitable: bool,
    pub recommended_component_id: Option<OcrComponentId>,
    pub summary: String,
}

#[derive(Clone, Debug, PartialEq, Serialize, Type)]
#[serde(rename_all = "camelCase")]
pub struct OcrComponentStatus {
    pub id: OcrComponentId,
    pub display_name: String,
    pub description: String,
    pub state: OcrComponentState,
    pub download_bytes: f64,
    pub installed_bytes: f64,
    pub recommended: bool,
    pub install_allowed: bool,
    pub status_detail: String,
    pub source_label: String,
    pub license_label: String,
}

#[derive(Clone, Debug, PartialEq, Serialize, Type)]
#[serde(rename_all = "camelCase")]
pub struct OcrCapabilityStatus {
    pub assessment: OcrHardwareAssessment,
    pub components: Vec<OcrComponentStatus>,
    pub recognition_feature: OcrRecognitionFeatureStatus,
    pub automatic_recognition_enabled: bool,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct HardwareFacts {
    pub logical_processors: u32,
    pub total_memory_bytes: u64,
    pub available_component_bytes: u64,
    pub avx2: bool,
    pub architecture: &'static str,
}

#[derive(Default)]
pub struct OcrCapabilityManager {
    mutation: tokio::sync::Mutex<()>,
}

impl OcrCapabilityManager {
    pub async fn lock_mutation(&self) -> tokio::sync::MutexGuard<'_, ()> {
        self.mutation.lock().await
    }
}

#[derive(Clone, Copy)]
struct OcrComponentFile {
    name: &'static str,
    url: &'static str,
    byte_length: u64,
    sha256: &'static str,
}

#[derive(Clone, Copy)]
enum ComponentAvailability {
    BuiltIn,
    Downloadable,
}

#[derive(Clone, Copy)]
struct OcrComponentDescriptor {
    id: OcrComponentId,
    display_name: &'static str,
    description: &'static str,
    version: &'static str,
    source_label: &'static str,
    license_label: &'static str,
    availability: ComponentAvailability,
    files: &'static [OcrComponentFile],
}

const COMPONENTS: &[OcrComponentDescriptor] = &[
    OcrComponentDescriptor {
        id: OcrComponentId::Ppocrv6Small,
        display_name: "本地题号定位增强",
        description: "约 31 MB，专门定位连续题号，是多题试卷切分的推荐模型；不做内容理解。",
        version: "rapidocr-3.9.2-ppocrv6-small",
        source_label: "ModelScope · RapidAI/RapidOCR 3.9.2",
        license_label: "PaddleOCR · Apache-2.0",
        availability: ComponentAvailability::Downloadable,
        files: SMALL_FILES,
    },
    OcrComponentDescriptor {
        id: OcrComponentId::Ppocrv6Medium,
        display_name: "PP‑OCRv6 medium（实验）",
        description: "约 139 MB，面向未来文字转写；真实切题样本上更慢且不比 small 稳，当前不用于切图。",
        version: "rapidocr-3.9.2-ppocrv6-medium",
        source_label: "ModelScope · RapidAI/RapidOCR 3.9.2",
        license_label: "PaddleOCR · Apache-2.0",
        availability: ComponentAvailability::Downloadable,
        files: MEDIUM_FILES,
    },
    OcrComponentDescriptor {
        id: OcrComponentId::OpencvPreprocess,
        display_name: "本地视觉切图",
        description: "按前景密度、分栏和留白拆分版面；不读取文字，也不推断题目内容。",
        version: "1.0.0",
        source_label: "Mistake Trainer Next 内置模块",
        license_label: "随应用发布",
        availability: ComponentAvailability::BuiltIn,
        files: &[],
    },
];

#[derive(Debug, Error)]
pub enum OcrCapabilityError {
    #[error("the optional component is unavailable")]
    Unavailable,
    #[error("the hardware assessment does not allow this component")]
    UnsupportedHardware,
    #[error("the optional component response failed")]
    Request(#[from] reqwest::Error),
    #[error("the optional component response status failed")]
    HttpStatus,
    #[error("the optional component exceeded its exact byte contract")]
    InvalidLength,
    #[error("the optional component failed integrity verification")]
    Integrity,
    #[error("the optional component manifest is invalid")]
    InvalidManifest,
    #[error("the optional component storage failed")]
    Io(#[from] std::io::Error),
    #[error("the optional component manifest could not be serialized")]
    Serialize(#[from] serde_json::Error),
}

impl OcrCapabilityError {
    pub const fn code(&self) -> &'static str {
        match self {
            Self::Unavailable => "unavailable",
            Self::UnsupportedHardware => "unsupported_hardware",
            Self::Request(_) => "request",
            Self::HttpStatus => "http_status",
            Self::InvalidLength => "invalid_length",
            Self::Integrity => "integrity",
            Self::InvalidManifest => "invalid_manifest",
            Self::Io(_) => "io",
            Self::Serialize(_) => "serialize",
        }
    }
}

#[derive(Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
struct InstalledManifest {
    schema_version: u32,
    component_id: OcrComponentId,
    version: String,
}

pub fn assess_hardware(facts: HardwareFacts) -> OcrHardwareAssessment {
    let compatible_architecture = facts.architecture == "x86_64";
    let performance = compatible_architecture
        && facts.avx2
        && facts.logical_processors >= 8
        && facts.total_memory_bytes >= 16 * GIB
        && facts.available_component_bytes >= 4 * GIB;
    let balanced = compatible_architecture
        && facts.avx2
        && facts.logical_processors >= 4
        && facts.total_memory_bytes >= 8 * GIB
        && facts.available_component_bytes >= 2 * GIB;
    let basic = compatible_architecture
        && facts.logical_processors >= 2
        && facts.total_memory_bytes >= 4 * GIB
        && facts.available_component_bytes >= GIB;

    let (tier, recommended_component_id, summary) = if performance {
        (
            OcrHardwareTier::Performance,
            Some(OcrComponentId::Ppocrv6Small),
            "本机预检通过，预计适合运行 small，并可按需尝试 medium。".to_owned(),
        )
    } else if balanced {
        (
            OcrHardwareTier::Balanced,
            Some(OcrComponentId::Ppocrv6Small),
            "本机预检通过，推荐使用 PP‑OCRv6 small。".to_owned(),
        )
    } else if basic {
        (
            OcrHardwareTier::Basic,
            None,
            "本机可继续使用现有手工流程，但暂不建议下载本地 OCR 模型。".to_owned(),
        )
    } else {
        (
            OcrHardwareTier::ManualOnly,
            None,
            "本机未通过本地 OCR 预检，现有拍照、整理和题库功能不受影响。".to_owned(),
        )
    };

    OcrHardwareAssessment {
        tier,
        logical_processor_count: facts.logical_processors,
        total_memory_mb: facts.total_memory_bytes as f64 / MIB as f64,
        available_component_storage_mb: facts.available_component_bytes as f64 / MIB as f64,
        avx2_supported: facts.avx2,
        estimated_suitable: matches!(
            tier,
            OcrHardwareTier::Balanced | OcrHardwareTier::Performance
        ),
        recommended_component_id,
        summary,
    }
}

pub fn capability_status(control_root: &Path) -> Result<OcrCapabilityStatus, OcrCapabilityError> {
    fs::create_dir_all(control_root)?;
    cleanup_staging(control_root)?;
    let assessment = assess_hardware(probe_hardware(control_root));
    let components = COMPONENTS
        .iter()
        .map(|descriptor| component_status(control_root, descriptor, &assessment))
        .collect::<Result<Vec<_>, _>>()?;
    let enhanced_ready = recognition_runtime_enabled()
        && components.iter().any(|component| {
            component.id == OcrComponentId::Ppocrv6Small
                && component.state == OcrComponentState::Installed
        });
    Ok(OcrCapabilityStatus {
        assessment,
        components,
        recognition_feature: if enhanced_ready {
            OcrRecognitionFeatureStatus {
                state: OcrRecognitionFeatureState::Ready,
                required_component_id: OcrComponentId::Ppocrv6Small,
                detail: DETAIL_ANCHOR_SPLIT_READY.to_owned(),
            }
        } else {
            visual_split_feature_status()
        },
        automatic_recognition_enabled: enhanced_ready,
    })
}

pub fn visual_split_feature_status() -> OcrRecognitionFeatureStatus {
    OcrRecognitionFeatureStatus {
        state: OcrRecognitionFeatureState::Ready,
        required_component_id: OcrComponentId::OpencvPreprocess,
        detail: DETAIL_VISUAL_SPLIT_READY.to_owned(),
    }
}

pub const fn recognition_runtime_enabled() -> bool {
    RECOGNITION_RUNTIME_AVAILABLE && cfg!(feature = "local-ocr-runtime")
}

pub fn recognition_feature_status(
    evidence_gate_passed: bool,
    runtime_available: bool,
    small_component_state: OcrComponentState,
) -> OcrRecognitionFeatureStatus {
    let (state, detail) = if !evidence_gate_passed {
        (OcrRecognitionFeatureState::EvidenceGatePending, DETAIL_GATE)
    } else if !runtime_available {
        (OcrRecognitionFeatureState::RuntimeMissing, DETAIL_RUNTIME)
    } else if small_component_state != OcrComponentState::Installed {
        (OcrRecognitionFeatureState::ModelMissing, DETAIL_MODEL)
    } else {
        (OcrRecognitionFeatureState::Ready, DETAIL_READY)
    };
    OcrRecognitionFeatureStatus {
        state,
        required_component_id: OcrComponentId::Ppocrv6Small,
        detail: detail.to_owned(),
    }
}

fn cleanup_staging(control_root: &Path) -> Result<(), OcrCapabilityError> {
    let staging = control_root
        .join(COMPONENT_DIRECTORY)
        .join(STAGING_DIRECTORY);
    match fs::remove_dir_all(&staging) {
        Ok(()) => {}
        Err(error) if error.kind() == std::io::ErrorKind::NotFound => {}
        Err(error) => return Err(error.into()),
    }
    Ok(())
}

pub async fn install_component(
    control_root: &Path,
    component_id: OcrComponentId,
    client: &reqwest::Client,
) -> Result<OcrComponentStatus, OcrCapabilityError> {
    let descriptor = descriptor(component_id);
    if !matches!(descriptor.availability, ComponentAvailability::Downloadable) {
        return Err(OcrCapabilityError::Unavailable);
    }
    let assessment = assess_hardware(probe_hardware(control_root));
    if !install_allowed(component_id, assessment.tier) {
        return Err(OcrCapabilityError::UnsupportedHardware);
    }
    let existing = component_status(control_root, descriptor, &assessment)?;
    if existing.state == OcrComponentState::Installed {
        return Ok(existing);
    }

    let components_root = control_root.join(COMPONENT_DIRECTORY);
    let staging_parent = components_root.join(STAGING_DIRECTORY);
    fs::create_dir_all(&staging_parent)?;
    let staging = staging_parent.join(format!("{}-{}", descriptor.id.key(), Uuid::now_v7()));
    fs::create_dir(&staging)?;

    let download_result = async {
        for file in descriptor.files {
            download_file(client, file, &staging.join(file.name)).await?;
        }
        let manifest = InstalledManifest {
            schema_version: MANIFEST_VERSION,
            component_id,
            version: descriptor.version.to_owned(),
        };
        let manifest_bytes = serde_json::to_vec_pretty(&manifest)?;
        let manifest_path = staging.join(MANIFEST_FILE);
        let mut manifest_file = tokio::fs::OpenOptions::new()
            .write(true)
            .create_new(true)
            .open(&manifest_path)
            .await?;
        manifest_file.write_all(&manifest_bytes).await?;
        manifest_file.sync_all().await?;
        drop(manifest_file);
        verify_component_directory(&staging, descriptor)?;
        promote_component(&components_root, &staging, descriptor.id)
    }
    .await;

    if download_result.is_err() {
        let _ = fs::remove_dir_all(&staging);
    }
    download_result?;
    component_status(control_root, descriptor, &assessment)
}

pub fn remove_component(
    control_root: &Path,
    component_id: OcrComponentId,
) -> Result<OcrComponentStatus, OcrCapabilityError> {
    let descriptor = descriptor(component_id);
    let target = control_root
        .join(COMPONENT_DIRECTORY)
        .join(descriptor.id.key());
    match fs::remove_dir_all(&target) {
        Ok(()) => {}
        Err(error) if error.kind() == std::io::ErrorKind::NotFound => {}
        Err(error) => return Err(error.into()),
    }
    let assessment = assess_hardware(probe_hardware(control_root));
    component_status(control_root, descriptor, &assessment)
}

fn component_status(
    control_root: &Path,
    descriptor: &OcrComponentDescriptor,
    assessment: &OcrHardwareAssessment,
) -> Result<OcrComponentStatus, OcrCapabilityError> {
    let (state, installed_bytes, status_detail) = match descriptor.availability {
        ComponentAvailability::BuiltIn => (
            OcrComponentState::Installed,
            0,
            "已随应用安装，可离线使用，不需要模型。".to_owned(),
        ),
        ComponentAvailability::Downloadable => {
            let directory = control_root
                .join(COMPONENT_DIRECTORY)
                .join(descriptor.id.key());
            if !directory.exists() {
                (
                    OcrComponentState::NotInstalled,
                    0,
                    "尚未下载；不会影响现有功能。".to_owned(),
                )
            } else {
                match verify_component_directory(&directory, descriptor) {
                    Ok(bytes) => (
                        OcrComponentState::Installed,
                        bytes,
                        if descriptor.id == OcrComponentId::Ppocrv6Small
                            && recognition_runtime_enabled()
                        {
                            "模型文件已通过完整性校验，题号定位增强可离线使用。".to_owned()
                        } else {
                            "模型文件已通过完整性校验；当前不会用于自动切图。".to_owned()
                        },
                    ),
                    Err(_) => (
                        OcrComponentState::Corrupt,
                        0,
                        "本地文件未通过校验，可重新下载；现有功能不受影响。".to_owned(),
                    ),
                }
            }
        }
    };
    Ok(OcrComponentStatus {
        id: descriptor.id,
        display_name: descriptor.display_name.to_owned(),
        description: descriptor.description.to_owned(),
        state,
        download_bytes: descriptor
            .files
            .iter()
            .map(|file| file.byte_length)
            .sum::<u64>() as f64,
        installed_bytes: installed_bytes as f64,
        recommended: assessment.recommended_component_id == Some(descriptor.id),
        install_allowed: matches!(descriptor.availability, ComponentAvailability::Downloadable)
            && install_allowed(descriptor.id, assessment.tier),
        status_detail,
        source_label: descriptor.source_label.to_owned(),
        license_label: descriptor.license_label.to_owned(),
    })
}

fn install_allowed(component_id: OcrComponentId, tier: OcrHardwareTier) -> bool {
    match component_id {
        OcrComponentId::Ppocrv6Small => matches!(
            tier,
            OcrHardwareTier::Balanced | OcrHardwareTier::Performance
        ),
        OcrComponentId::Ppocrv6Medium => tier == OcrHardwareTier::Performance,
        OcrComponentId::OpencvPreprocess => false,
    }
}

fn descriptor(component_id: OcrComponentId) -> &'static OcrComponentDescriptor {
    COMPONENTS
        .iter()
        .find(|descriptor| descriptor.id == component_id)
        .expect("every component ID must have an immutable descriptor")
}

async fn download_file(
    client: &reqwest::Client,
    descriptor: &OcrComponentFile,
    destination: &Path,
) -> Result<(), OcrCapabilityError> {
    let response = client.get(descriptor.url).send().await?;
    if !response.status().is_success() || response.url().scheme() != "https" {
        return Err(OcrCapabilityError::HttpStatus);
    }
    if response
        .content_length()
        .is_some_and(|length| length != descriptor.byte_length)
    {
        return Err(OcrCapabilityError::InvalidLength);
    }
    let mut output = tokio::fs::OpenOptions::new()
        .write(true)
        .create_new(true)
        .open(destination)
        .await?;
    let mut received = 0_u64;
    let mut hash = Sha256::new();
    let mut stream = response.bytes_stream();
    while let Some(chunk) = stream.next().await {
        let chunk = chunk?;
        received = received
            .checked_add(u64::try_from(chunk.len()).unwrap_or(u64::MAX))
            .ok_or(OcrCapabilityError::InvalidLength)?;
        if received > descriptor.byte_length {
            return Err(OcrCapabilityError::InvalidLength);
        }
        hash.update(&chunk);
        output.write_all(&chunk).await?;
    }
    output.sync_all().await?;
    drop(output);
    if received != descriptor.byte_length {
        return Err(OcrCapabilityError::InvalidLength);
    }
    if format!("{:x}", hash.finalize()) != descriptor.sha256 {
        return Err(OcrCapabilityError::Integrity);
    }
    Ok(())
}

fn verify_component_directory(
    directory: &Path,
    descriptor: &OcrComponentDescriptor,
) -> Result<u64, OcrCapabilityError> {
    let manifest: InstalledManifest =
        serde_json::from_slice(&fs::read(directory.join(MANIFEST_FILE))?)?;
    if manifest.schema_version != MANIFEST_VERSION
        || manifest.component_id != descriptor.id
        || manifest.version != descriptor.version
    {
        return Err(OcrCapabilityError::InvalidManifest);
    }
    let mut installed_bytes = 0_u64;
    for expected in descriptor.files {
        let path = directory.join(expected.name);
        let metadata = fs::metadata(&path)?;
        if !metadata.is_file() || metadata.len() != expected.byte_length {
            return Err(OcrCapabilityError::InvalidLength);
        }
        let digest = Sha256::digest(fs::read(&path)?);
        if format!("{digest:x}") != expected.sha256 {
            return Err(OcrCapabilityError::Integrity);
        }
        installed_bytes = installed_bytes.saturating_add(metadata.len());
    }
    Ok(installed_bytes)
}

fn promote_component(
    components_root: &Path,
    staging: &Path,
    component_id: OcrComponentId,
) -> Result<(), OcrCapabilityError> {
    let target = components_root.join(component_id.key());
    let previous = components_root.join(format!(".previous-{}", Uuid::now_v7()));
    let had_previous = target.exists();
    if had_previous {
        fs::rename(&target, &previous)?;
    }
    if let Err(error) = fs::rename(staging, &target) {
        if had_previous {
            let _ = fs::rename(&previous, &target);
        }
        return Err(error.into());
    }
    if had_previous {
        let _ = fs::remove_dir_all(previous);
    }
    Ok(())
}

fn probe_hardware(control_root: &Path) -> HardwareFacts {
    HardwareFacts {
        logical_processors: std::thread::available_parallelism()
            .map(|count| u32::try_from(count.get()).unwrap_or(u32::MAX))
            .unwrap_or(1),
        total_memory_bytes: total_memory_bytes(),
        available_component_bytes: available_storage_bytes(control_root),
        avx2: avx2_supported(),
        architecture: std::env::consts::ARCH,
    }
}

#[cfg(target_arch = "x86_64")]
fn avx2_supported() -> bool {
    std::arch::is_x86_feature_detected!("avx2")
}

#[cfg(not(target_arch = "x86_64"))]
const fn avx2_supported() -> bool {
    false
}

#[cfg(windows)]
fn total_memory_bytes() -> u64 {
    use windows::Win32::System::SystemInformation::{GlobalMemoryStatusEx, MEMORYSTATUSEX};

    let mut status = MEMORYSTATUSEX {
        dwLength: u32::try_from(std::mem::size_of::<MEMORYSTATUSEX>()).unwrap_or_default(),
        ..Default::default()
    };
    unsafe { GlobalMemoryStatusEx(&mut status) }
        .map(|_| status.ullTotalPhys)
        .unwrap_or_default()
}

#[cfg(not(windows))]
const fn total_memory_bytes() -> u64 {
    0
}

#[cfg(windows)]
fn available_storage_bytes(control_root: &Path) -> u64 {
    use std::os::windows::ffi::OsStrExt as _;
    use windows::{Win32::Storage::FileSystem::GetDiskFreeSpaceExW, core::PCWSTR};

    let probe = control_root
        .canonicalize()
        .unwrap_or_else(|_| control_root.to_path_buf());
    let wide = probe
        .as_os_str()
        .encode_wide()
        .chain(std::iter::once(0))
        .collect::<Vec<_>>();
    let mut available = 0_u64;
    unsafe { GetDiskFreeSpaceExW(PCWSTR(wide.as_ptr()), Some(&mut available), None, None) }
        .map(|_| available)
        .unwrap_or_default()
}

#[cfg(not(windows))]
const fn available_storage_bytes(_control_root: &Path) -> u64 {
    0
}

pub fn download_client() -> Result<reqwest::Client, OcrCapabilityError> {
    Ok(reqwest::Client::builder()
        .user_agent(concat!("Mistake-Trainer-Next/", env!("CARGO_PKG_VERSION")))
        .connect_timeout(std::time::Duration::from_secs(15))
        .timeout(std::time::Duration::from_secs(600))
        .redirect(reqwest::redirect::Policy::limited(5))
        .build()?)
}

#[cfg(test)]
mod tests {
    use super::*;
    use tempfile::tempdir;

    #[test]
    fn four_cores_eight_gib_avx2_recommends_small() {
        let assessment = assess_hardware(HardwareFacts {
            logical_processors: 4,
            total_memory_bytes: 8 * GIB,
            available_component_bytes: 3 * GIB,
            avx2: true,
            architecture: "x86_64",
        });

        assert_eq!(assessment.tier, OcrHardwareTier::Balanced);
        assert_eq!(
            assessment.recommended_component_id,
            Some(OcrComponentId::Ppocrv6Small)
        );
        assert!(assessment.estimated_suitable);
    }

    #[test]
    fn medium_requires_eight_cores_and_sixteen_gib() {
        let assessment = assess_hardware(HardwareFacts {
            logical_processors: 8,
            total_memory_bytes: 16 * GIB,
            available_component_bytes: 5 * GIB,
            avx2: true,
            architecture: "x86_64",
        });

        assert_eq!(assessment.tier, OcrHardwareTier::Performance);
        assert!(install_allowed(
            OcrComponentId::Ppocrv6Medium,
            assessment.tier
        ));
    }

    #[test]
    fn low_memory_machine_keeps_every_download_disabled() {
        let assessment = assess_hardware(HardwareFacts {
            logical_processors: 8,
            total_memory_bytes: 3 * GIB,
            available_component_bytes: 5 * GIB,
            avx2: true,
            architecture: "x86_64",
        });

        assert_eq!(assessment.tier, OcrHardwareTier::ManualOnly);
        assert!(!assessment.estimated_suitable);
        assert_eq!(assessment.recommended_component_id, None);
    }

    #[test]
    fn visual_splitter_is_built_in_while_optional_ocr_model_stays_downloadable() {
        let root = tempdir().unwrap();
        let assessment = assess_hardware(HardwareFacts {
            logical_processors: 8,
            total_memory_bytes: 16 * GIB,
            available_component_bytes: 5 * GIB,
            avx2: true,
            architecture: "x86_64",
        });
        let small = component_status(
            root.path(),
            descriptor(OcrComponentId::Ppocrv6Small),
            &assessment,
        )
        .unwrap();
        let opencv = component_status(
            root.path(),
            descriptor(OcrComponentId::OpencvPreprocess),
            &assessment,
        )
        .unwrap();

        assert_eq!(small.state, OcrComponentState::NotInstalled);
        assert!(small.install_allowed);
        assert_eq!(opencv.state, OcrComponentState::Installed);
        assert!(!opencv.install_allowed);
        assert_eq!(opencv.installed_bytes, opencv.download_bytes);
    }

    #[test]
    fn corrupt_component_never_looks_installed() {
        let root = tempdir().unwrap();
        let directory = root
            .path()
            .join(COMPONENT_DIRECTORY)
            .join(OcrComponentId::Ppocrv6Small.key());
        fs::create_dir_all(&directory).unwrap();
        fs::write(directory.join(MANIFEST_FILE), b"{}").unwrap();
        let assessment = assess_hardware(HardwareFacts {
            logical_processors: 4,
            total_memory_bytes: 8 * GIB,
            available_component_bytes: 3 * GIB,
            avx2: true,
            architecture: "x86_64",
        });

        let status = component_status(
            root.path(),
            descriptor(OcrComponentId::Ppocrv6Small),
            &assessment,
        )
        .unwrap();

        assert_eq!(status.state, OcrComponentState::Corrupt);
        assert_eq!(status.installed_bytes, 0.0);
    }

    #[test]
    fn removal_is_idempotent_and_scoped_to_one_component() {
        let root = tempdir().unwrap();
        let component = root
            .path()
            .join(COMPONENT_DIRECTORY)
            .join(OcrComponentId::Ppocrv6Small.key());
        let sibling = root.path().join("library.db");
        fs::create_dir_all(&component).unwrap();
        fs::write(component.join("partial"), b"broken").unwrap();
        fs::write(&sibling, b"preserve").unwrap();

        let first = remove_component(root.path(), OcrComponentId::Ppocrv6Small).unwrap();
        let second = remove_component(root.path(), OcrComponentId::Ppocrv6Small).unwrap();

        assert_eq!(first.state, OcrComponentState::NotInstalled);
        assert_eq!(second.state, OcrComponentState::NotInstalled);
        assert_eq!(fs::read(sibling).unwrap(), b"preserve");
    }

    #[test]
    fn status_cleanup_removes_only_interrupted_component_staging() {
        let root = tempdir().unwrap();
        let staging = root
            .path()
            .join(COMPONENT_DIRECTORY)
            .join(STAGING_DIRECTORY)
            .join("interrupted");
        let sibling = root.path().join("library.db");
        fs::create_dir_all(&staging).unwrap();
        fs::write(staging.join("partial.onnx"), b"partial").unwrap();
        fs::write(&sibling, b"preserve").unwrap();

        cleanup_staging(root.path()).unwrap();

        assert!(!staging.exists());
        assert_eq!(fs::read(sibling).unwrap(), b"preserve");
    }

    #[test]
    fn downloadable_catalog_is_https_versioned_and_size_bounded() {
        for descriptor in COMPONENTS.iter().filter(|descriptor| {
            matches!(descriptor.availability, ComponentAvailability::Downloadable)
        }) {
            assert!(!descriptor.files.is_empty());
            assert!(descriptor.version.contains("rapidocr-3.9.2"));
            for file in descriptor.files {
                assert!(file.url.starts_with("https://"));
                assert!(file.url.contains("/resolve/v3.9.2/"));
                assert!(file.name.ends_with(".onnx"));
                assert!(!file.name.contains(['/', '\\']));
                assert!(file.byte_length > MIB);
                assert!(file.byte_length < 100 * MIB);
                assert_eq!(file.sha256.len(), 64);
            }
        }
    }

    #[test]
    fn runtime_gate_also_requires_the_runtime_feature_to_be_compiled() {
        assert_eq!(
            recognition_runtime_enabled(),
            RECOGNITION_RUNTIME_AVAILABLE && cfg!(feature = "local-ocr-runtime")
        );
    }

    #[cfg(windows)]
    #[test]
    fn windows_probe_reports_real_memory_disk_and_processor_facts() {
        let root = tempdir().unwrap();
        let facts = probe_hardware(root.path());

        assert!(facts.logical_processors >= 1);
        assert!(facts.total_memory_bytes >= GIB);
        assert!(facts.available_component_bytes >= MIB);
        assert_eq!(facts.architecture, std::env::consts::ARCH);
    }
}
