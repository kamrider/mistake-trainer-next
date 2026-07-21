use std::{
    fmt,
    future::Future,
    time::{Duration, SystemTime, UNIX_EPOCH},
};

use base64::{Engine as _, engine::general_purpose::STANDARD as BASE64};
use futures_util::StreamExt;
use reqwest::{StatusCode, Url, redirect::Policy};
use serde::{Deserialize, Serialize};
use thiserror::Error;
use uuid::Uuid;

const MAX_AUTH_RESPONSE_BYTES: usize = 2 * 1024 * 1024;
const MAX_EMAIL_BYTES: usize = 320;
const MIN_PASSWORD_BYTES: usize = 8;
const MAX_PASSWORD_BYTES: usize = 1024;

#[derive(Clone)]
struct SecretString(String);

impl SecretString {
    fn expose(&self) -> &str {
        &self.0
    }
}

impl fmt::Debug for SecretString {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter.write_str("<redacted>")
    }
}

#[derive(Clone)]
pub struct SupabaseConfig {
    base_url: Url,
    storage_url: Url,
    publishable_key: SecretString,
    allow_insecure_loopback: bool,
}

impl fmt::Debug for SupabaseConfig {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter
            .debug_struct("SupabaseConfig")
            .field("base_url", &self.base_url)
            .field("storage_url", &self.storage_url)
            .field("publishable_key", &self.publishable_key)
            .finish()
    }
}

impl SupabaseConfig {
    pub fn from_build_environment() -> Result<Option<Self>, CloudError> {
        let url = option_env!("MISTAKE_TRAINER_SUPABASE_URL");
        let key = option_env!("MISTAKE_TRAINER_SUPABASE_PUBLISHABLE_KEY");
        match (url, key) {
            (None, None) => Ok(None),
            (Some(url), Some(key)) => Self::hosted(url, key).map(Some),
            _ => Err(CloudError::InvalidConfiguration),
        }
    }

    pub fn hosted(base_url: &str, publishable_key: &str) -> Result<Self, CloudError> {
        let base_url = parse_base_url(base_url)?;
        let host = base_url
            .host_str()
            .ok_or(CloudError::InvalidConfiguration)?;
        let labels = host.split('.').collect::<Vec<_>>();
        if base_url.scheme() != "https"
            || labels.len() != 3
            || labels[0].is_empty()
            || labels[1] != "supabase"
            || labels[2] != "co"
            || !base_url.username().is_empty()
            || base_url.password().is_some()
            || publishable_key.trim().is_empty()
            || publishable_key.len() > 4096
        {
            return Err(CloudError::InvalidConfiguration);
        }
        let storage_url = Url::parse(&format!("https://{}.storage.supabase.co/", labels[0]))
            .map_err(|_| CloudError::InvalidConfiguration)?;
        Ok(Self {
            base_url,
            storage_url,
            publishable_key: SecretString(publishable_key.to_owned()),
            allow_insecure_loopback: false,
        })
    }

    #[doc(hidden)]
    pub fn for_loopback_test(base_url: &str, publishable_key: &str) -> Result<Self, CloudError> {
        if !cfg!(debug_assertions) {
            return Err(CloudError::InvalidConfiguration);
        }
        let base_url = parse_base_url(base_url)?;
        let is_loopback = base_url.host_str().is_some_and(|host| {
            host == "localhost"
                || host
                    .parse::<std::net::IpAddr>()
                    .is_ok_and(|ip| ip.is_loopback())
        });
        if base_url.scheme() != "http" || !is_loopback || publishable_key.trim().is_empty() {
            return Err(CloudError::InvalidConfiguration);
        }
        Ok(Self {
            storage_url: base_url.clone(),
            base_url,
            publishable_key: SecretString(publishable_key.to_owned()),
            allow_insecure_loopback: true,
        })
    }

    pub fn storage_url(&self) -> &Url {
        &self.storage_url
    }
}

fn parse_base_url(value: &str) -> Result<Url, CloudError> {
    let url = Url::parse(value).map_err(|_| CloudError::InvalidConfiguration)?;
    if url.query().is_some()
        || url.fragment().is_some()
        || url.path() != "/"
        || !url.username().is_empty()
        || url.password().is_some()
    {
        return Err(CloudError::InvalidConfiguration);
    }
    Ok(url)
}

pub struct SupabaseClient {
    config: SupabaseConfig,
    http: reqwest::Client,
}

impl fmt::Debug for SupabaseClient {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter
            .debug_struct("SupabaseClient")
            .field("config", &self.config)
            .field("http", &"<configured transport>")
            .finish()
    }
}

impl SupabaseClient {
    pub fn new(config: SupabaseConfig) -> Result<Self, CloudError> {
        let http = reqwest::Client::builder()
            .redirect(Policy::none())
            .connect_timeout(Duration::from_secs(10))
            .timeout(Duration::from_secs(30))
            .https_only(!config.allow_insecure_loopback)
            .user_agent("Mistake-Trainer-Next/0.1")
            .build()
            .map_err(|_| CloudError::TransportConfiguration)?;
        Ok(Self { config, http })
    }

    async fn auth_request<T: Serialize + ?Sized>(
        &self,
        method: reqwest::Method,
        path: &str,
        body: &T,
        access_token: Option<&str>,
    ) -> Result<AuthReply, CloudError> {
        let url = self
            .config
            .base_url
            .join(path)
            .map_err(|_| CloudError::InvalidConfiguration)?;
        let mut request = self
            .http
            .request(method, url)
            .header("apikey", self.config.publishable_key.expose())
            .header("accept", "application/json")
            .json(body);
        if let Some(token) = access_token {
            request = request.bearer_auth(token);
        }
        let response = request.send().await.map_err(map_request_error)?;
        if !response.status().is_success() {
            return Err(map_status(response.status()));
        }
        let bytes = read_capped(response, MAX_AUTH_RESPONSE_BYTES).await?;
        let wire: AuthWire =
            serde_json::from_slice(&bytes).map_err(|_| CloudError::InvalidResponse)?;
        AuthReply::from_wire(wire)
    }

    fn validate_credentials(email: &str, password: &str) -> Result<(), CloudError> {
        if email.trim() != email
            || email.is_empty()
            || email.len() > MAX_EMAIL_BYTES
            || !email.contains('@')
            || password.len() < MIN_PASSWORD_BYTES
            || password.len() > MAX_PASSWORD_BYTES
        {
            return Err(CloudError::InvalidCredentialsInput);
        }
        Ok(())
    }
}

pub trait AuthTransport: Sync {
    fn sign_up<'a>(
        &'a self,
        email: &'a str,
        password: &'a str,
    ) -> impl Future<Output = Result<AuthReply, CloudError>> + Send + 'a;
    fn sign_in<'a>(
        &'a self,
        email: &'a str,
        password: &'a str,
    ) -> impl Future<Output = Result<AuthReply, CloudError>> + Send + 'a;
    fn refresh<'a>(
        &'a self,
        refresh_token: &'a str,
    ) -> impl Future<Output = Result<AuthReply, CloudError>> + Send + 'a;
    fn revoke<'a>(
        &'a self,
        access_token: &'a str,
    ) -> impl Future<Output = Result<(), CloudError>> + Send + 'a;
}

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
    pub operation_id: String,
    pub entity_type: String,
    pub entity_id: String,
    pub change_seq: i64,
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

impl AuthTransport for SupabaseClient {
    fn sign_up<'a>(
        &'a self,
        email: &'a str,
        password: &'a str,
    ) -> impl Future<Output = Result<AuthReply, CloudError>> + Send + 'a {
        async move {
            Self::validate_credentials(email, password)?;
            self.auth_request(
                reqwest::Method::POST,
                "/auth/v1/signup",
                &PasswordBody { email, password },
                None,
            )
            .await
        }
    }

    fn sign_in<'a>(
        &'a self,
        email: &'a str,
        password: &'a str,
    ) -> impl Future<Output = Result<AuthReply, CloudError>> + Send + 'a {
        async move {
            Self::validate_credentials(email, password)?;
            self.auth_request(
                reqwest::Method::POST,
                "/auth/v1/token?grant_type=password",
                &PasswordBody { email, password },
                None,
            )
            .await
        }
    }

    fn refresh<'a>(
        &'a self,
        refresh_token: &'a str,
    ) -> impl Future<Output = Result<AuthReply, CloudError>> + Send + 'a {
        async move {
            if refresh_token.is_empty() || refresh_token.len() > 8192 {
                return Err(CloudError::InvalidResponse);
            }
            self.auth_request(
                reqwest::Method::POST,
                "/auth/v1/token?grant_type=refresh_token",
                &RefreshBody { refresh_token },
                None,
            )
            .await
        }
    }

    fn revoke<'a>(
        &'a self,
        access_token: &'a str,
    ) -> impl Future<Output = Result<(), CloudError>> + Send + 'a {
        async move {
            if access_token.is_empty() || access_token.len() > 16 * 1024 {
                return Err(CloudError::InvalidResponse);
            }
            let url = self
                .config
                .base_url
                .join("/auth/v1/logout")
                .map_err(|_| CloudError::InvalidConfiguration)?;
            let response = self
                .http
                .post(url)
                .header("apikey", self.config.publishable_key.expose())
                .bearer_auth(access_token)
                .send()
                .await
                .map_err(map_request_error)?;
            if response.status().is_success() || response.status() == StatusCode::UNAUTHORIZED {
                Ok(())
            } else {
                Err(map_status(response.status()))
            }
        }
    }
}

impl CloudPushTransport for SupabaseClient {
    fn object_metadata<'a>(
        &'a self,
        access_token: &'a str,
        storage_object: &'a str,
    ) -> impl Future<Output = Result<Option<RemoteObjectMetadata>, CloudError>> + Send + 'a {
        async move {
            validate_access_token(access_token)?;
            let url = self.object_url(storage_object)?;
            let response = self
                .http
                .head(url)
                .header("apikey", self.config.publishable_key.expose())
                .bearer_auth(access_token)
                .send()
                .await
                .map_err(map_request_error)?;
            if response.status() == StatusCode::NOT_FOUND {
                return Ok(None);
            }
            if !response.status().is_success() {
                return Err(map_status(response.status()));
            }
            let byte_length = response
                .headers()
                .get(reqwest::header::CONTENT_LENGTH)
                .and_then(|value| value.to_str().ok())
                .and_then(|value| value.parse::<i64>().ok())
                .filter(|value| *value >= 0)
                .ok_or(CloudError::InvalidResponse)?;
            let media_type = response
                .headers()
                .get(reqwest::header::CONTENT_TYPE)
                .and_then(|value| value.to_str().ok())
                .and_then(|value| value.split(';').next())
                .map(str::trim)
                .filter(|value| !value.is_empty())
                .ok_or(CloudError::InvalidResponse)?
                .to_owned();
            Ok(Some(RemoteObjectMetadata {
                byte_length,
                media_type,
            }))
        }
    }

    fn upload_small_object<'a>(
        &'a self,
        access_token: &'a str,
        storage_object: &'a str,
        media_type: &'a str,
        bytes: &'a [u8],
    ) -> impl Future<Output = Result<ObjectUploadResult, CloudError>> + Send + 'a {
        async move {
            validate_access_token(access_token)?;
            let url = self.object_url(storage_object)?;
            let response = self
                .http
                .post(url)
                .header("apikey", self.config.publishable_key.expose())
                .header("x-upsert", "false")
                .header(reqwest::header::CONTENT_TYPE, media_type)
                .bearer_auth(access_token)
                .body(bytes.to_vec())
                .send()
                .await
                .map_err(map_request_error)?;
            if response.status().is_success() {
                Ok(ObjectUploadResult::Created)
            } else if matches!(
                response.status(),
                StatusCode::BAD_REQUEST | StatusCode::CONFLICT
            ) {
                // Storage reports an immutable-path collision as 400 today and
                // some compatible deployments use 409. The caller must still
                // re-read and match the remote metadata before accepting it.
                Ok(ObjectUploadResult::AlreadyExists)
            } else {
                Err(map_status(response.status()))
            }
        }
    }

    fn create_resumable_upload<'a>(
        &'a self,
        access_token: &'a str,
        storage_object: &'a str,
        media_type: &'a str,
        byte_length: i64,
    ) -> impl Future<Output = Result<String, CloudError>> + Send + 'a {
        async move {
            validate_access_token(access_token)?;
            validate_storage_object(storage_object)?;
            if byte_length <= 0 {
                return Err(CloudError::InvalidResponse);
            }
            let url = self
                .config
                .storage_url
                .join("/storage/v1/upload/resumable")
                .map_err(|_| CloudError::InvalidConfiguration)?;
            let metadata = format!(
                "bucketName {},objectName {},contentType {}",
                BASE64.encode("mistake-assets"),
                BASE64.encode(storage_object),
                BASE64.encode(media_type)
            );
            let response = self
                .http
                .post(url)
                .header("apikey", self.config.publishable_key.expose())
                .header("tus-resumable", "1.0.0")
                .header("upload-length", byte_length)
                .header("upload-metadata", metadata)
                .header("x-upsert", "false")
                .bearer_auth(access_token)
                .send()
                .await
                .map_err(map_request_error)?;
            if !response.status().is_success() {
                return Err(map_status(response.status()));
            }
            let location = response
                .headers()
                .get(reqwest::header::LOCATION)
                .and_then(|value| value.to_str().ok())
                .ok_or(CloudError::InvalidResponse)?;
            let upload_url = self
                .config
                .storage_url
                .join(location)
                .map_err(|_| CloudError::InvalidResponse)?;
            self.validate_resumable_url(&upload_url)?;
            Ok(upload_url.to_string())
        }
    }

    fn resumable_offset<'a>(
        &'a self,
        access_token: &'a str,
        upload_url: &'a str,
    ) -> impl Future<Output = Result<Option<i64>, CloudError>> + Send + 'a {
        async move {
            validate_access_token(access_token)?;
            let url = Url::parse(upload_url).map_err(|_| CloudError::InvalidResponse)?;
            self.validate_resumable_url(&url)?;
            let response = self
                .http
                .head(url)
                .header("apikey", self.config.publishable_key.expose())
                .header("tus-resumable", "1.0.0")
                .bearer_auth(access_token)
                .send()
                .await
                .map_err(map_request_error)?;
            if matches!(response.status(), StatusCode::NOT_FOUND | StatusCode::GONE) {
                return Ok(None);
            }
            if !response.status().is_success() {
                return Err(map_status(response.status()));
            }
            parse_upload_offset(&response).map(Some)
        }
    }

    fn upload_resumable_chunk<'a>(
        &'a self,
        access_token: &'a str,
        upload_url: &'a str,
        offset: i64,
        bytes: &'a [u8],
    ) -> impl Future<Output = Result<Option<i64>, CloudError>> + Send + 'a {
        async move {
            validate_access_token(access_token)?;
            if offset < 0 || bytes.is_empty() {
                return Err(CloudError::InvalidResponse);
            }
            let url = Url::parse(upload_url).map_err(|_| CloudError::InvalidResponse)?;
            self.validate_resumable_url(&url)?;
            let response = self
                .http
                .patch(url)
                .header("apikey", self.config.publishable_key.expose())
                .header("tus-resumable", "1.0.0")
                .header("upload-offset", offset)
                .header(
                    reqwest::header::CONTENT_TYPE,
                    "application/offset+octet-stream",
                )
                .bearer_auth(access_token)
                .body(bytes.to_vec())
                .send()
                .await
                .map_err(map_request_error)?;
            if matches!(response.status(), StatusCode::NOT_FOUND | StatusCode::GONE) {
                return Ok(None);
            }
            if !response.status().is_success() {
                return Err(map_status(response.status()));
            }
            parse_upload_offset(&response).map(Some)
        }
    }

    fn push_operations<'a>(
        &'a self,
        access_token: &'a str,
        operations: &'a serde_json::Value,
    ) -> impl Future<Output = Result<Vec<PushAcknowledgement>, CloudError>> + Send + 'a {
        async move {
            validate_access_token(access_token)?;
            let operation_count = operations.as_array().map(Vec::len).unwrap_or_default();
            if !(1..=100).contains(&operation_count) {
                return Err(CloudError::InvalidResponse);
            }
            let url = self
                .config
                .base_url
                .join("/rest/v1/rpc/push_sync_batch")
                .map_err(|_| CloudError::InvalidConfiguration)?;
            let response = self
                .http
                .post(url)
                .header("apikey", self.config.publishable_key.expose())
                .header("accept", "application/json")
                .bearer_auth(access_token)
                .json(&serde_json::json!({ "p_operations": operations }))
                .send()
                .await
                .map_err(map_request_error)?;
            if !response.status().is_success() {
                return Err(map_status(response.status()));
            }
            let bytes = read_capped(response, MAX_AUTH_RESPONSE_BYTES).await?;
            serde_json::from_slice(&bytes).map_err(|_| CloudError::InvalidResponse)
        }
    }
}

impl SupabaseClient {
    fn object_url(&self, storage_object: &str) -> Result<Url, CloudError> {
        validate_storage_object(storage_object)?;
        self.config
            .base_url
            .join(&format!(
                "/storage/v1/object/mistake-assets/{storage_object}"
            ))
            .map_err(|_| CloudError::InvalidConfiguration)
    }

    fn validate_resumable_url(&self, candidate: &Url) -> Result<(), CloudError> {
        let expected = &self.config.storage_url;
        if candidate.scheme() != expected.scheme()
            || candidate.host_str() != expected.host_str()
            || candidate.port_or_known_default() != expected.port_or_known_default()
            || !candidate.username().is_empty()
            || candidate.password().is_some()
            || candidate.fragment().is_some()
            || !candidate.path().starts_with("/storage/v1/upload/resumable")
        {
            return Err(CloudError::InvalidResponse);
        }
        Ok(())
    }
}

fn validate_access_token(access_token: &str) -> Result<(), CloudError> {
    if access_token.is_empty() || access_token.len() > 16 * 1024 {
        Err(CloudError::InvalidResponse)
    } else {
        Ok(())
    }
}

fn validate_storage_object(storage_object: &str) -> Result<(), CloudError> {
    let mut parts = storage_object.split('/');
    let account = parts.next().unwrap_or_default();
    let hash = parts.next().unwrap_or_default();
    if parts.next().is_some()
        || Uuid::parse_str(account).is_err()
        || hash.len() != 64
        || !hash
            .bytes()
            .all(|byte| byte.is_ascii_digit() || (b'a'..=b'f').contains(&byte))
    {
        return Err(CloudError::InvalidResponse);
    }
    Ok(())
}

fn parse_upload_offset(response: &reqwest::Response) -> Result<i64, CloudError> {
    response
        .headers()
        .get("upload-offset")
        .and_then(|value| value.to_str().ok())
        .and_then(|value| value.parse::<i64>().ok())
        .filter(|value| *value >= 0)
        .ok_or(CloudError::InvalidResponse)
}

#[derive(Serialize)]
struct PasswordBody<'a> {
    email: &'a str,
    password: &'a str,
}

#[derive(Serialize)]
struct RefreshBody<'a> {
    refresh_token: &'a str,
}

#[derive(Deserialize)]
struct AuthWire {
    #[serde(default)]
    access_token: Option<String>,
    #[serde(default)]
    refresh_token: Option<String>,
    #[serde(default)]
    expires_in: Option<i64>,
    #[serde(default)]
    expires_at: Option<i64>,
    user: AuthUserWire,
}

#[derive(Deserialize)]
struct AuthUserWire {
    id: String,
    email: Option<String>,
    #[serde(default)]
    email_confirmed_at: Option<String>,
}

pub struct AuthReply {
    user_id: String,
    email: String,
    access_token: Option<SecretString>,
    refresh_token: Option<SecretString>,
    expires_at_utc_ms: Option<i64>,
    email_verified: bool,
}

impl fmt::Debug for AuthReply {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter
            .debug_struct("AuthReply")
            .field("user_id", &"<redacted>")
            .field("email", &redact_email(&self.email))
            .field("access_token", &self.access_token)
            .field("refresh_token", &self.refresh_token)
            .field("expires_at_utc_ms", &self.expires_at_utc_ms)
            .field("email_verified", &self.email_verified)
            .finish()
    }
}

impl AuthReply {
    fn from_wire(wire: AuthWire) -> Result<Self, CloudError> {
        Uuid::parse_str(&wire.user.id).map_err(|_| CloudError::InvalidResponse)?;
        let email = wire.user.email.ok_or(CloudError::InvalidResponse)?;
        if email.len() > MAX_EMAIL_BYTES {
            return Err(CloudError::InvalidResponse);
        }
        let has_access = wire
            .access_token
            .as_ref()
            .is_some_and(|value| !value.is_empty());
        let has_refresh = wire
            .refresh_token
            .as_ref()
            .is_some_and(|value| !value.is_empty());
        if has_access != has_refresh {
            return Err(CloudError::InvalidResponse);
        }
        let expires_at_utc_ms = if has_access {
            let seconds = wire.expires_at.unwrap_or_else(|| {
                now_utc_seconds().saturating_add(wire.expires_in.unwrap_or(3600).clamp(1, 86_400))
            });
            Some(seconds.saturating_mul(1000))
        } else {
            None
        };
        Ok(Self {
            user_id: wire.user.id,
            email,
            access_token: wire.access_token.map(SecretString),
            refresh_token: wire.refresh_token.map(SecretString),
            expires_at_utc_ms,
            email_verified: wire.user.email_confirmed_at.is_some() || has_access,
        })
    }

    pub fn verified_session(
        user_id: &str,
        email: &str,
        access_token: &str,
        refresh_token: &str,
        expires_at_utc_ms: i64,
    ) -> Self {
        Self {
            user_id: user_id.to_owned(),
            email: email.to_owned(),
            access_token: Some(SecretString(access_token.to_owned())),
            refresh_token: Some(SecretString(refresh_token.to_owned())),
            expires_at_utc_ms: Some(expires_at_utc_ms),
            email_verified: true,
        }
    }

    pub fn verification_required(user_id: &str, email: &str) -> Self {
        Self {
            user_id: user_id.to_owned(),
            email: email.to_owned(),
            access_token: None,
            refresh_token: None,
            expires_at_utc_ms: None,
            email_verified: false,
        }
    }

    pub fn user_id(&self) -> &str {
        &self.user_id
    }

    pub fn requires_email_verification(&self) -> bool {
        !self.email_verified || self.access_token.is_none()
    }

    pub(crate) fn into_session_parts(
        self,
    ) -> Result<(String, String, String, String, i64), CloudError> {
        if !self.email_verified {
            return Err(CloudError::EmailVerificationRequired);
        }
        Uuid::parse_str(&self.user_id).map_err(|_| CloudError::InvalidResponse)?;
        if self.email.is_empty() || self.email.len() > MAX_EMAIL_BYTES {
            return Err(CloudError::InvalidResponse);
        }
        let access_token = self.access_token.ok_or(CloudError::InvalidResponse)?.0;
        let refresh_token = self.refresh_token.ok_or(CloudError::InvalidResponse)?.0;
        if access_token.is_empty() || refresh_token.is_empty() {
            return Err(CloudError::InvalidResponse);
        }
        Ok((
            self.user_id,
            self.email,
            access_token,
            refresh_token,
            self.expires_at_utc_ms.ok_or(CloudError::InvalidResponse)?,
        ))
    }
}

async fn read_capped(response: reqwest::Response, limit: usize) -> Result<Vec<u8>, CloudError> {
    if response
        .content_length()
        .is_some_and(|length| length > limit as u64)
    {
        return Err(CloudError::ResponseTooLarge);
    }
    let mut output = Vec::new();
    let mut stream = response.bytes_stream();
    while let Some(chunk) = stream.next().await {
        let chunk = chunk.map_err(map_request_error)?;
        if output.len().saturating_add(chunk.len()) > limit {
            return Err(CloudError::ResponseTooLarge);
        }
        output.extend_from_slice(&chunk);
    }
    Ok(output)
}

fn map_request_error(error: reqwest::Error) -> CloudError {
    if error.is_timeout() {
        CloudError::Timeout
    } else {
        CloudError::Network
    }
}

fn map_status(status: StatusCode) -> CloudError {
    match status {
        StatusCode::BAD_REQUEST | StatusCode::UNAUTHORIZED | StatusCode::FORBIDDEN => {
            CloudError::AuthenticationRejected
        }
        StatusCode::TOO_MANY_REQUESTS => CloudError::RateLimited,
        status if status.is_server_error() => CloudError::ServiceUnavailable,
        _ => CloudError::InvalidResponse,
    }
}

fn now_utc_seconds() -> i64 {
    SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .unwrap_or_default()
        .as_secs()
        .try_into()
        .unwrap_or(i64::MAX)
}

pub(crate) fn redact_email(email: &str) -> String {
    let Some((local, domain)) = email.split_once('@') else {
        return "***".to_owned();
    };
    let mut characters = local.chars();
    let first = characters.next().unwrap_or('*');
    let last = characters.last().unwrap_or(first);
    format!("{first}***{last}@{domain}")
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
