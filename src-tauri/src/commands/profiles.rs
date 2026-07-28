use serde::{Deserialize, Serialize};
use specta::Type;
use std::path::Path;
use tauri::State;
use uuid::Uuid;

use crate::{
    application::result::AppResult,
    domain::profile::ProfileName,
    infrastructure::{assets::remove_asset_blob, runtime::LibraryRuntime},
    modules::{
        capture_lan::CaptureLanManager,
        profiles::{
            CreateProfile, DeleteProfile, LearnerProfile, ProfileUseCaseError, RenameProfile,
            create_profile, delete_profile, list_profiles, rename_profile,
        },
    },
};

#[derive(Clone, Debug, Serialize, Type)]
#[serde(rename_all = "camelCase")]
pub struct ProfileSummary {
    pub id: String,
    pub name: String,
    pub created_at_utc_ms: f64,
    pub updated_at_utc_ms: f64,
    pub revision: i32,
}

#[derive(Clone, Debug, Serialize, Type)]
#[serde(rename_all = "camelCase")]
pub struct ProfileOverview {
    pub active_profile_id: String,
    pub profiles: Vec<ProfileSummary>,
}

#[derive(Clone, Debug, Deserialize, Type)]
#[serde(rename_all = "camelCase")]
pub struct ProfileNameInput {
    pub name: String,
}

#[derive(Clone, Debug, Deserialize, Type)]
#[serde(rename_all = "camelCase")]
pub struct ProfileRenameInput {
    pub profile_id: String,
    pub name: String,
}

#[derive(Clone, Debug, Deserialize, Type)]
#[serde(rename_all = "camelCase")]
pub struct ProfileDeleteInput {
    pub profile_id: String,
    pub confirmation_name: String,
}

pub fn profile_list_for(runtime: &LibraryRuntime) -> AppResult<ProfileOverview> {
    overview_for(runtime)
}

pub fn profile_create_for(
    runtime: &LibraryRuntime,
    manager: &CaptureLanManager,
    input: ProfileNameInput,
    now_utc_ms: i64,
) -> AppResult<ProfileOverview> {
    let name = match ProfileName::parse(&input.name) {
        Ok(name) => name.as_str().to_owned(),
        Err(error) => return profile_error(&ProfileUseCaseError::InvalidName(error)),
    };
    let _transition = runtime.lock_profile_transition();
    {
        let connection = match runtime.connection.lock() {
            Ok(connection) => connection,
            Err(_) => return profile_error_code("library_lock_poisoned", None),
        };
        match list_profiles(&connection, runtime.account_id()) {
            Ok(profiles) if profiles.iter().any(|profile| profile.name == name) => {
                return profile_error(&ProfileUseCaseError::DuplicateName);
            }
            Ok(_) => {}
            Err(error) => return profile_error(&error),
        }
    }
    if let Err(error) = manager.stop() {
        return profile_lan_error(&error);
    }
    let profile = {
        let mut connection = match runtime.connection.lock() {
            Ok(connection) => connection,
            Err(_) => return profile_error_code("library_lock_poisoned", None),
        };
        match create_profile(
            &mut connection,
            CreateProfile {
                account_id: runtime.account_id().to_owned(),
                name,
                now_utc_ms,
            },
        ) {
            Ok(profile) => profile,
            Err(error) => return profile_error(&error),
        }
    };
    if let Err(error) = runtime.activate_profile(&profile.id, now_utc_ms) {
        return profile_error(&error);
    }
    overview_for(runtime)
}

pub fn profile_rename_for(
    runtime: &LibraryRuntime,
    input: ProfileRenameInput,
    now_utc_ms: i64,
) -> AppResult<ProfileOverview> {
    let profile = {
        let mut connection = match runtime.connection.lock() {
            Ok(connection) => connection,
            Err(_) => return profile_error_code("library_lock_poisoned", None),
        };
        match rename_profile(
            &mut connection,
            RenameProfile {
                account_id: runtime.account_id().to_owned(),
                profile_id: input.profile_id,
                name: input.name,
                now_utc_ms,
            },
        ) {
            Ok(profile) => profile,
            Err(error) => return profile_error(&error),
        }
    };
    runtime.refresh_active_profile(&profile);
    overview_for(runtime)
}

pub fn profile_select_for(
    runtime: &LibraryRuntime,
    manager: &CaptureLanManager,
    profile_id: String,
    now_utc_ms: i64,
) -> AppResult<ProfileOverview> {
    let _transition = runtime.lock_profile_transition();
    if runtime.active_profile().id == profile_id {
        return overview_for(runtime);
    }
    {
        let connection = match runtime.connection.lock() {
            Ok(connection) => connection,
            Err(_) => return profile_error_code("library_lock_poisoned", None),
        };
        match list_profiles(&connection, runtime.account_id()) {
            Ok(profiles) if profiles.iter().any(|profile| profile.id == profile_id) => {}
            Ok(_) => return profile_error(&ProfileUseCaseError::NotFound),
            Err(error) => return profile_error(&error),
        }
    }
    if let Err(error) = manager.stop() {
        return profile_lan_error(&error);
    }
    if let Err(error) = runtime.activate_profile(&profile_id, now_utc_ms) {
        return profile_error(&error);
    }
    overview_for(runtime)
}

pub fn profile_delete_for(
    runtime: &LibraryRuntime,
    manager: &CaptureLanManager,
    input: ProfileDeleteInput,
    now_utc_ms: i64,
) -> AppResult<ProfileOverview> {
    let _transition = runtime.lock_profile_transition();
    {
        let connection = match runtime.connection.lock() {
            Ok(connection) => connection,
            Err(_) => return profile_error_code("library_lock_poisoned", None),
        };
        let profiles = match list_profiles(&connection, runtime.account_id()) {
            Ok(profiles) => profiles,
            Err(error) => return profile_error(&error),
        };
        if !profiles
            .iter()
            .any(|profile| profile.id == input.profile_id)
        {
            return profile_error(&ProfileUseCaseError::NotFound);
        }
        if profiles.len() <= 1 {
            return profile_error(&ProfileUseCaseError::LastProfile);
        }
    }
    if let Err(error) = manager.stop() {
        return profile_lan_error(&error);
    }
    let receipt = {
        let mut connection = match runtime.connection.lock() {
            Ok(connection) => connection,
            Err(_) => return profile_error_code("library_lock_poisoned", None),
        };
        match delete_profile(
            &mut connection,
            DeleteProfile {
                account_id: runtime.account_id().to_owned(),
                profile_id: input.profile_id,
                confirmation_name: input.confirmation_name,
                now_utc_ms,
            },
        ) {
            Ok(receipt) => receipt,
            Err(error) => return profile_error(&error),
        }
    };
    runtime.replace_active_profile(&receipt.active_profile);
    for orphan in &receipt.orphan_assets {
        remove_orphan_blob(&runtime.blob_root, &orphan.encrypted_path);
    }
    overview_for(runtime)
}

#[tauri::command]
#[specta::specta]
pub fn profile_list(state: State<'_, LibraryRuntime>) -> AppResult<ProfileOverview> {
    profile_list_for(&state)
}

#[tauri::command]
#[specta::specta]
pub fn profile_create(
    state: State<'_, LibraryRuntime>,
    manager: State<'_, CaptureLanManager>,
    input: ProfileNameInput,
) -> AppResult<ProfileOverview> {
    profile_create_for(&state, &manager, input, current_utc_millis())
}

#[tauri::command]
#[specta::specta]
pub fn profile_rename(
    state: State<'_, LibraryRuntime>,
    input: ProfileRenameInput,
) -> AppResult<ProfileOverview> {
    profile_rename_for(&state, input, current_utc_millis())
}

#[tauri::command]
#[specta::specta]
pub fn profile_select(
    state: State<'_, LibraryRuntime>,
    manager: State<'_, CaptureLanManager>,
    profile_id: String,
) -> AppResult<ProfileOverview> {
    profile_select_for(&state, &manager, profile_id, current_utc_millis())
}

#[tauri::command]
#[specta::specta]
pub fn profile_delete(
    state: State<'_, LibraryRuntime>,
    manager: State<'_, CaptureLanManager>,
    input: ProfileDeleteInput,
) -> AppResult<ProfileOverview> {
    profile_delete_for(&state, &manager, input, current_utc_millis())
}

fn overview_for(runtime: &LibraryRuntime) -> AppResult<ProfileOverview> {
    let active = runtime.active_profile();
    let connection = match runtime.connection.lock() {
        Ok(connection) => connection,
        Err(_) => return profile_error_code("library_lock_poisoned", None),
    };
    match list_profiles(&connection, runtime.account_id()) {
        Ok(profiles) => AppResult::success(ProfileOverview {
            active_profile_id: active.id,
            profiles: profiles.into_iter().map(ProfileSummary::from).collect(),
        }),
        Err(error) => profile_error(&error),
    }
}

impl From<LearnerProfile> for ProfileSummary {
    fn from(profile: LearnerProfile) -> Self {
        Self {
            id: profile.id,
            name: profile.name,
            created_at_utc_ms: profile.created_at_utc_ms as f64,
            updated_at_utc_ms: profile.updated_at_utc_ms as f64,
            revision: i32::try_from(profile.revision).unwrap_or(i32::MAX),
        }
    }
}

fn profile_error<T>(error: &ProfileUseCaseError) -> AppResult<T> {
    let code = match error {
        ProfileUseCaseError::InvalidName(_) => "profile_name_invalid",
        ProfileUseCaseError::DuplicateName => "profile_name_duplicate",
        ProfileUseCaseError::NotFound => "profile_not_found",
        ProfileUseCaseError::LastProfile => "profile_last_cannot_delete",
        ProfileUseCaseError::ConfirmationMismatch => "profile_delete_confirmation_mismatch",
        ProfileUseCaseError::ConflictPending => "profile_conflict_pending",
        ProfileUseCaseError::Database(_) | ProfileUseCaseError::Serialization(_) => {
            "profile_operation_failed"
        }
    };
    profile_error_code(code, Some(error))
}

fn profile_error_code<T>(code: &str, error: Option<&ProfileUseCaseError>) -> AppResult<T> {
    let (message, retryable) = match code {
        "profile_name_invalid" => (
            "档案名称不能为空、不能超过 40 个字，也不能含有路径符号。",
            false,
        ),
        "profile_name_duplicate" => ("已经有同名学习档案，请换一个名称。", false),
        "profile_not_found" => ("这个学习档案已不存在，请刷新后重新选择。", false),
        "profile_last_cannot_delete" => (
            "至少需要保留一个学习档案；请先新建另一个档案再删除。",
            false,
        ),
        "profile_delete_confirmation_mismatch" => {
            ("输入的档案名称不一致；没有删除任何资料。", false)
        }
        "profile_conflict_pending" => (
            "这个学习档案有尚未处理的同步冲突，请先到“设置 → 同步冲突”完成选择。",
            false,
        ),
        "library_lock_poisoned" => ("本地题库暂时不可用，请重新打开应用后重试。", true),
        _ => (
            "学习档案没有完成这次操作，原有数据保持不变，请稍后重试。",
            true,
        ),
    };
    let diagnostic_id = Uuid::now_v7().to_string();
    if let Some(error) = error {
        eprintln!("profile error [{diagnostic_id}] {code}: {error}");
    }
    AppResult::failure(code, message, retryable, diagnostic_id)
}

fn profile_lan_error<T>(error: &crate::modules::capture_lan::CaptureLanError) -> AppResult<T> {
    let diagnostic_id = Uuid::now_v7().to_string();
    eprintln!("profile error [{diagnostic_id}] profile_capture_stop_failed: {error}");
    AppResult::failure(
        "profile_capture_stop_failed",
        "手机采集会话没有安全停止，暂未切换档案；请停止采集后重试。",
        true,
        diagnostic_id,
    )
}

fn current_utc_millis() -> i64 {
    std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .map(|duration| i64::try_from(duration.as_millis()).unwrap_or(i64::MAX))
        .unwrap_or_default()
}

fn remove_orphan_blob(blob_root: &Path, encrypted_path: &str) {
    match remove_asset_blob(blob_root, encrypted_path) {
        Ok(_) => {}
        Err(error) => eprintln!(
            "profile orphan cleanup [{}] could not remove an unreferenced blob: {error}",
            Uuid::now_v7()
        ),
    }
}
