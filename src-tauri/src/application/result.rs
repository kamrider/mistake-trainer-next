use serde::Serialize;
use specta::Type;

#[derive(Clone, Debug, Serialize, Type)]
#[serde(rename_all = "camelCase")]
pub struct AppError {
    pub code: String,
    pub user_message: String,
    pub retryable: bool,
    pub diagnostic_id: String,
}

#[derive(Clone, Debug, Serialize, Type)]
#[serde(untagged)]
pub enum AppResult<T> {
    Success { ok: bool, data: T },
    Failure { ok: bool, error: AppError },
}

impl<T> AppResult<T> {
    pub fn success(data: T) -> Self {
        Self::Success { ok: true, data }
    }

    pub fn failure(
        code: impl Into<String>,
        user_message: impl Into<String>,
        retryable: bool,
        diagnostic_id: impl Into<String>,
    ) -> Self {
        Self::Failure {
            ok: false,
            error: AppError {
                code: code.into(),
                user_message: user_message.into(),
                retryable,
                diagnostic_id: diagnostic_id.into(),
            },
        }
    }
}
