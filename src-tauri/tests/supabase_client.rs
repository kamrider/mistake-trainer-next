use std::sync::{Arc, Mutex};

use axum::{
    Router,
    body::{Body, to_bytes},
    extract::State,
    http::{Request, StatusCode},
    response::Response,
    routing::any,
};
use mistake_trainer_next_lib::infrastructure::supabase::{
    AuthTransport, CloudError, SupabaseClient, SupabaseConfig,
};
use tokio::sync::oneshot;

#[derive(Clone, Debug)]
struct CapturedRequest {
    path_and_query: String,
    api_key: String,
    authorization: Option<String>,
    body: String,
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
