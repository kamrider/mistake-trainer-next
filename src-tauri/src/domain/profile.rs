use serde::{Deserialize, Serialize};
use thiserror::Error;

const MAX_PROFILE_NAME_CHARS: usize = 40;

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(transparent)]
pub struct ProfileName(String);

#[derive(Clone, Debug, Error, PartialEq, Eq)]
pub enum ProfileNameError {
    #[error("学习档案名称不能为空")]
    Empty,
    #[error("学习档案名称不能超过 {MAX_PROFILE_NAME_CHARS} 个字符")]
    TooLong,
    #[error("学习档案名称包含不安全字符")]
    UnsafeCharacters,
}

impl ProfileName {
    pub fn parse(value: &str) -> Result<Self, ProfileNameError> {
        let trimmed = value.trim();
        if trimmed.is_empty() {
            return Err(ProfileNameError::Empty);
        }
        if trimmed.chars().count() > MAX_PROFILE_NAME_CHARS {
            return Err(ProfileNameError::TooLong);
        }
        if trimmed == "."
            || trimmed == ".."
            || trimmed.contains(['/', '\\'])
            || trimmed.chars().any(char::is_control)
        {
            return Err(ProfileNameError::UnsafeCharacters);
        }
        Ok(Self(trimmed.to_owned()))
    }

    pub fn as_str(&self) -> &str {
        &self.0
    }
}
