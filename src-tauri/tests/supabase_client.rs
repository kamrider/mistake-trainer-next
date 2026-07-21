use std::{
    collections::HashMap,
    sync::{Arc, Mutex},
};

use axum::{
    Router,
    body::{Body, to_bytes},
    extract::State,
    http::{Method, Request, StatusCode},
    response::Response,
    routing::any,
};
use mistake_trainer_next_lib::infrastructure::supabase::{
    AuthTransport, CloudError, CloudPushTransport, ObjectUploadResult, SupabaseClient,
    SupabaseConfig,
};
use tokio::sync::oneshot;

#[derive(Clone, Debug)]
struct CapturedRequest {
    path_and_query: String,
    api_key: String,
    authorization: Option<String>,
    body: String,
}

#[derive(Clone, Debug)]
struct CapturedCloudRequest {
    method: Method,
    path: String,
    headers: HashMap<String, String>,
    body: Vec<u8>,
}

#[test]
fn hosted_configuration_accepts_only_the_expected_supabase_origin() {
    let config = SupabaseConfig::hosted(
        "https://abcdefghijklmnopqrst.supabase.co/",
        "sb_publishable_secret-value",
    )
    .unwrap();

    assert_eq!(
        config.storage_url().as_str(),
        "https://abcdefghijklmnopqrst.storage.supabase.co/"
    );
    assert!(!format!("{config:?}").contains("sb_publishable_secret-value"));

    for invalid_url in [
        "http://abcdefghijklmnopqrst.supabase.co/",
        "https://supabase.co/",
        "https://abcdefghijklmnopqrst.supabase.co/rest/v1/",
        "https://abcdefghijklmnopqrst.supabase.co/?redirect=evil",
        "https://abcdefghijklmnopqrst.supabase.co.evil.test/",
        "https://user:password@abcdefghijklmnopqrst.supabase.co/",
    ] {
        assert!(matches!(
            SupabaseConfig::hosted(invalid_url, "publishable"),
            Err(CloudError::InvalidConfiguration)
        ));
    }
}

#[test]
fn password_sign_in_uses_the_exact_auth_endpoint_without_exposing_secrets() {
    tokio::runtime::Runtime::new().unwrap().block_on(async {
        let (request_tx, request_rx) = oneshot::channel();
        let request_tx = Arc::new(Mutex::new(Some(request_tx)));
        let app = Router::new()
            .fallback(any(capture_auth_request))
            .with_state(request_tx);
        let listener = tokio::net::TcpListener::bind("127.0.0.1:0").await.unwrap();
        let address = listener.local_addr().unwrap();
        tokio::spawn(async move { axum::serve(listener, app).await.unwrap() });

        let config = SupabaseConfig::for_loopback_test(
            &format!("http://{address}"),
            "sb_publishable_private-test-value",
        )
        .unwrap();
        let client = SupabaseClient::new(config).unwrap();
        let reply = client
            .sign_in("student@example.test", "correct horse battery staple")
            .await
            .unwrap();
        let captured = request_rx.await.unwrap();

        assert_eq!(
            captured.path_and_query,
            "/auth/v1/token?grant_type=password"
        );
        assert_eq!(captured.api_key, "sb_publishable_private-test-value");
        assert_eq!(captured.authorization, None);
        assert!(captured.body.contains("student@example.test"));
        assert!(captured.body.contains("correct horse battery staple"));
        assert_eq!(reply.user_id(), "33333333-3333-4333-8333-333333333333");

        let debug = format!("{client:?} {reply:?}");
        for secret in [
            "sb_publishable_private-test-value",
            "access-secret",
            "refresh-secret",
            "correct horse battery staple",
        ] {
            assert!(!debug.contains(secret));
        }
    });
}

#[test]
fn oversized_auth_responses_are_rejected_before_deserialization() {
    tokio::runtime::Runtime::new().unwrap().block_on(async {
        let app = Router::new().fallback(any(|| async {
            Response::builder()
                .status(StatusCode::OK)
                .header("content-type", "application/json")
                .body(Body::from(vec![b'x'; 2 * 1024 * 1024 + 1]))
                .unwrap()
        }));
        let listener = tokio::net::TcpListener::bind("127.0.0.1:0").await.unwrap();
        let address = listener.local_addr().unwrap();
        tokio::spawn(async move { axum::serve(listener, app).await.unwrap() });
        let client = SupabaseClient::new(
            SupabaseConfig::for_loopback_test(&format!("http://{address}"), "publishable").unwrap(),
        )
        .unwrap();

        assert!(matches!(
            client.sign_up("student@example.test", "password-123").await,
            Err(CloudError::ResponseTooLarge)
        ));
    });
}

#[test]
fn storage_tus_and_rpc_requests_follow_the_supabase_wire_contract() {
    tokio::runtime::Runtime::new().unwrap().block_on(async {
        let captured = Arc::new(Mutex::new(Vec::<CapturedCloudRequest>::new()));
        let app = Router::new()
            .fallback(any(capture_cloud_request))
            .with_state(captured.clone());
        let listener = tokio::net::TcpListener::bind("127.0.0.1:0").await.unwrap();
        let address = listener.local_addr().unwrap();
        tokio::spawn(async move { axum::serve(listener, app).await.unwrap() });
        let client = SupabaseClient::new(
            SupabaseConfig::for_loopback_test(&format!("http://{address}"), "publishable").unwrap(),
        )
        .unwrap();
        let storage_object = format!("33333333-3333-4333-8333-333333333333/{}", "a".repeat(64));

        assert_eq!(
            client
                .upload_small_object("access", &storage_object, "image/jpeg", b"small")
                .await
                .unwrap(),
            ObjectUploadResult::Created
        );
        let upload_url = client
            .create_resumable_upload("access", &storage_object, "image/jpeg", 9)
            .await
            .unwrap();
        assert_eq!(
            client
                .resumable_offset("access", &upload_url)
                .await
                .unwrap(),
            Some(6)
        );
        assert_eq!(
            client
                .upload_resumable_chunk("access", &upload_url, 6, b"end")
                .await
                .unwrap(),
            Some(9)
        );
        let acknowledgements = client
            .push_operations(
                "access",
                &serde_json::json!([{
                    "operationId": "0191365e-2f2f-7b89-b3b0-777777777777",
                    "entityType": "asset",
                    "entityId": "0191365e-2f2f-7b89-b3b0-888888888888",
                    "operation": "upsert",
                    "payload": {"id": "0191365e-2f2f-7b89-b3b0-888888888888"}
                }]),
            )
            .await
            .unwrap();
        assert_eq!(acknowledgements.len(), 1);

        let captured = captured.lock().unwrap();
        assert_eq!(captured.len(), 5);
        let small = &captured[0];
        assert_eq!(small.method, Method::POST);
        assert_eq!(
            small.path,
            format!("/storage/v1/object/mistake-assets/{storage_object}")
        );
        assert_eq!(small.headers["x-upsert"], "false");
        assert_eq!(small.headers["content-type"], "image/jpeg");
        assert_eq!(small.headers["authorization"], "Bearer access");
        assert_eq!(small.body, b"small");

        let create = &captured[1];
        assert_eq!(create.path, "/storage/v1/upload/resumable");
        assert_eq!(create.headers["tus-resumable"], "1.0.0");
        assert_eq!(create.headers["upload-length"], "9");
        assert_eq!(create.headers["x-upsert"], "false");
        assert!(create.headers["upload-metadata"].contains("bucketName bWlzdGFrZS1hc3NldHM="));

        let head = &captured[2];
        assert_eq!(head.method, Method::HEAD);
        assert_eq!(head.headers["tus-resumable"], "1.0.0");
        let patch = &captured[3];
        assert_eq!(patch.method, Method::PATCH);
        assert_eq!(patch.headers["upload-offset"], "6");
        assert_eq!(
            patch.headers["content-type"],
            "application/offset+octet-stream"
        );
        assert_eq!(patch.body, b"end");

        let rpc = &captured[4];
        assert_eq!(rpc.path, "/rest/v1/rpc/push_sync_batch");
        let rpc_body: serde_json::Value = serde_json::from_slice(&rpc.body).unwrap();
        assert_eq!(rpc_body["p_operations"].as_array().unwrap().len(), 1);
    });
}

#[test]
fn storage_collision_requires_caller_side_metadata_revalidation() {
    tokio::runtime::Runtime::new().unwrap().block_on(async {
        let app = Router::new().fallback(any(|| async {
            Response::builder()
                .status(StatusCode::BAD_REQUEST)
                .header("content-type", "application/json")
                .body(Body::from(r#"{"error":"ResourceAlreadyExists"}"#))
                .unwrap()
        }));
        let listener = tokio::net::TcpListener::bind("127.0.0.1:0").await.unwrap();
        let address = listener.local_addr().unwrap();
        tokio::spawn(async move { axum::serve(listener, app).await.unwrap() });
        let client = SupabaseClient::new(
            SupabaseConfig::for_loopback_test(&format!("http://{address}"), "publishable").unwrap(),
        )
        .unwrap();
        let storage_object = format!("33333333-3333-4333-8333-333333333333/{}", "b".repeat(64));

        assert_eq!(
            client
                .upload_small_object("access", &storage_object, "image/png", b"duplicate")
                .await
                .unwrap(),
            ObjectUploadResult::AlreadyExists
        );
    });
}

async fn capture_auth_request(
    State(sender): State<Arc<Mutex<Option<oneshot::Sender<CapturedRequest>>>>>,
    request: Request<Body>,
) -> Response<Body> {
    let path_and_query = request
        .uri()
        .path_and_query()
        .map(ToString::to_string)
        .unwrap_or_default();
    let api_key = request
        .headers()
        .get("apikey")
        .and_then(|value| value.to_str().ok())
        .unwrap_or_default()
        .to_owned();
    let authorization = request
        .headers()
        .get("authorization")
        .and_then(|value| value.to_str().ok())
        .map(ToOwned::to_owned);
    let body = to_bytes(request.into_body(), 16 * 1024).await.unwrap();
    sender
        .lock()
        .unwrap()
        .take()
        .unwrap()
        .send(CapturedRequest {
            path_and_query,
            api_key,
            authorization,
            body: String::from_utf8(body.to_vec()).unwrap(),
        })
        .unwrap();
    Response::builder()
        .status(StatusCode::OK)
        .header("content-type", "application/json")
        .body(Body::from(
            r#"{"access_token":"access-secret","refresh_token":"refresh-secret","expires_in":3600,"user":{"id":"33333333-3333-4333-8333-333333333333","email":"student@example.test","email_confirmed_at":"2026-07-21T00:00:00Z"}}"#,
        ))
        .unwrap()
}

async fn capture_cloud_request(
    State(captured): State<Arc<Mutex<Vec<CapturedCloudRequest>>>>,
    request: Request<Body>,
) -> Response<Body> {
    let method = request.method().clone();
    let path = request.uri().path().to_owned();
    let headers = request
        .headers()
        .iter()
        .filter_map(|(name, value)| {
            value
                .to_str()
                .ok()
                .map(|value| (name.as_str().to_owned(), value.to_owned()))
        })
        .collect();
    let body = to_bytes(request.into_body(), 8 * 1024 * 1024)
        .await
        .unwrap()
        .to_vec();
    captured.lock().unwrap().push(CapturedCloudRequest {
        method: method.clone(),
        path: path.clone(),
        headers,
        body,
    });

    if method == Method::POST && path.starts_with("/storage/v1/object/") {
        return Response::builder()
            .status(StatusCode::OK)
            .body(Body::empty())
            .unwrap();
    }
    if method == Method::POST && path == "/storage/v1/upload/resumable" {
        return Response::builder()
            .status(StatusCode::CREATED)
            .header("location", "/storage/v1/upload/resumable/upload-id")
            .body(Body::empty())
            .unwrap();
    }
    if method == Method::HEAD && path == "/storage/v1/upload/resumable/upload-id" {
        return Response::builder()
            .status(StatusCode::OK)
            .header("upload-offset", "6")
            .body(Body::empty())
            .unwrap();
    }
    if method == Method::PATCH && path == "/storage/v1/upload/resumable/upload-id" {
        return Response::builder()
            .status(StatusCode::NO_CONTENT)
            .header("upload-offset", "9")
            .body(Body::empty())
            .unwrap();
    }
    if method == Method::POST && path == "/rest/v1/rpc/push_sync_batch" {
        return Response::builder()
            .status(StatusCode::OK)
            .header("content-type", "application/json")
            .body(Body::from(
                r#"[{"operationId":"0191365e-2f2f-7b89-b3b0-777777777777","entityType":"asset","entityId":"0191365e-2f2f-7b89-b3b0-888888888888","changeSeq":1}]"#,
            ))
            .unwrap();
    }
    Response::builder()
        .status(StatusCode::NOT_FOUND)
        .body(Body::empty())
        .unwrap()
}
