use std::future::Future;

use serde::Deserialize;
use thiserror::Error;

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct RemoteObjectMetadata {
    pub byte_length: i64,
    pub media_type: String,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum ObjectUploadResult {
    Created,
    AlreadyExists,
}

#[derive(Clone, Debug, PartialEq, Eq, Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct PushAcknowledgement {
    #[serde(alias = "operation_id")]
    pub operation_id: String,
    #[serde(alias = "entity_type")]
    pub entity_type: String,
    #[serde(alias = "entity_id")]
    pub entity_id: String,
    #[serde(alias = "change_seq")]
    pub change_seq: i64,
}

#[derive(Clone, Debug, PartialEq, Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct RemotePullChange {
    #[serde(alias = "change_seq")]
    pub change_seq: i64,
    #[serde(alias = "entity_type")]
    pub entity_type: String,
    #[serde(alias = "entity_id")]
    pub entity_id: String,
    pub operation: String,
    pub payload: serde_json::Value,
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct DownloadedRemoteAsset {
    pub bytes: Vec<u8>,
    pub media_type: String,
}

pub trait CloudPushTransport: Sync {
    fn object_metadata<'a>(
        &'a self,
        access_token: &'a str,
        storage_object: &'a str,
    ) -> impl Future<Output = Result<Option<RemoteObjectMetadata>, CloudError>> + Send + 'a;
    fn upload_small_object<'a>(
        &'a self,
        access_token: &'a str,
        storage_object: &'a str,
        media_type: &'a str,
        bytes: &'a [u8],
    ) -> impl Future<Output = Result<ObjectUploadResult, CloudError>> + Send + 'a;
    fn create_resumable_upload<'a>(
        &'a self,
        access_token: &'a str,
        storage_object: &'a str,
        media_type: &'a str,
        byte_length: i64,
    ) -> impl Future<Output = Result<String, CloudError>> + Send + 'a;
    fn resumable_offset<'a>(
        &'a self,
        access_token: &'a str,
        upload_url: &'a str,
    ) -> impl Future<Output = Result<Option<i64>, CloudError>> + Send + 'a;
    fn upload_resumable_chunk<'a>(
        &'a self,
        access_token: &'a str,
        upload_url: &'a str,
        offset: i64,
        bytes: &'a [u8],
    ) -> impl Future<Output = Result<Option<i64>, CloudError>> + Send + 'a;
    fn push_operations<'a>(
        &'a self,
        access_token: &'a str,
        operations: &'a serde_json::Value,
    ) -> impl Future<Output = Result<Vec<PushAcknowledgement>, CloudError>> + Send + 'a;
}

pub trait CloudPullTransport: Sync {
    fn pull_changes<'a>(
        &'a self,
        access_token: &'a str,
        after: i64,
        limit: usize,
    ) -> impl Future<Output = Result<Vec<RemotePullChange>, CloudError>> + Send + 'a;
    fn download_object<'a>(
        &'a self,
        access_token: &'a str,
        storage_object: &'a str,
    ) -> impl Future<Output = Result<DownloadedRemoteAsset, CloudError>> + Send + 'a;
}

#[derive(Debug, Error, Clone, Copy, PartialEq, Eq)]
pub enum CloudError {
    #[error("cloud configuration is invalid")]
    InvalidConfiguration,
    #[error("cloud transport could not be configured")]
    TransportConfiguration,
    #[error("email or password input is invalid")]
    InvalidCredentialsInput,
    #[error("the account credentials were rejected")]
    AuthenticationRejected,
    #[error("email verification is required")]
    EmailVerificationRequired,
    #[error("the cloud response was invalid")]
    InvalidResponse,
    #[error("the cloud response exceeded its size limit")]
    ResponseTooLarge,
    #[error("the cloud request timed out")]
    Timeout,
    #[error("the cloud service could not be reached")]
    Network,
    #[error("the cloud service rate limit was reached")]
    RateLimited,
    #[error("the cloud service is temporarily unavailable")]
    ServiceUnavailable,
    #[error("secure credential storage failed")]
    SecretStore,
    #[error("this library is already bound to another account")]
    LibraryBoundToAnotherAccount,
}

impl CloudError {
    pub const fn retryable(self) -> bool {
        matches!(
            self,
            Self::Timeout | Self::Network | Self::RateLimited | Self::ServiceUnavailable
        )
    }
}

#[cfg(test)]
mod tests {
    use super::CloudError;

    #[test]
    fn only_transient_transport_failures_are_retryable() {
        for error in [
            CloudError::Timeout,
            CloudError::Network,
            CloudError::RateLimited,
            CloudError::ServiceUnavailable,
        ] {
            assert!(error.retryable(), "{error:?} should remain retryable");
        }
        for error in [
            CloudError::InvalidConfiguration,
            CloudError::TransportConfiguration,
            CloudError::InvalidCredentialsInput,
            CloudError::AuthenticationRejected,
            CloudError::EmailVerificationRequired,
            CloudError::InvalidResponse,
            CloudError::ResponseTooLarge,
            CloudError::SecretStore,
            CloudError::LibraryBoundToAnotherAccount,
        ] {
            assert!(!error.retryable(), "{error:?} must fail closed");
        }
    }
}
