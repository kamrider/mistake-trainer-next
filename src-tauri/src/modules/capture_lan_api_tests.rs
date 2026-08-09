use std::{
    io::Cursor,
    net::{Ipv4Addr, SocketAddr, TcpListener, TcpStream},
    sync::{Arc, Mutex},
    thread,
    time::Duration,
};

use axum::http::{Method, Request};
use base64::{Engine as _, engine::general_purpose::STANDARD};
use image::{DynamicImage, ImageBuffer, ImageFormat, Rgba};
use tempfile::TempDir;
use tokio::sync::{Semaphore, watch};
use tower::ServiceExt as _;

use crate::{
    infrastructure::database::{open_encrypted_database, run_migrations},
    modules::{
        capture_inbox::{
            CaptureBatchState, CreateCaptureBatch, create_capture_batch, get_capture_batch_detail,
        },
        profiles::{CreateProfile, create_profile},
    },
};

use super::super::{
    CaptureLanAddress, CaptureLanContext, CaptureLanError, IDLE_TIMEOUT_MS, ServerState,
    SessionActivity, WeakCaptureLanSessionRegistry, is_private_lan, run_server, select_address,
};
use super::*;

const TEST_TOKEN: &str = "test-capture-token";

struct TestServer {
    _directory: TempDir,
    state: Arc<ServerState>,
    router: Router,
}

impl TestServer {
    fn new() -> Self {
        let directory = tempfile::tempdir().expect("temporary directory");
        let mut connection = open_encrypted_database(
            &directory.path().join("capture-lan.db"),
            "capture-lan-test-key",
        )
        .expect("open database");
        run_migrations(&mut connection).expect("migrate database");
        let profile = create_profile(
            &mut connection,
            CreateProfile {
                account_id: "account-1".to_owned(),
                name: "student".to_owned(),
                now_utc_ms: 1,
            },
        )
        .expect("create profile");
        let batch = create_capture_batch(
            &mut connection,
            CreateCaptureBatch {
                account_id: "account-1".to_owned(),
                profile_id: profile.id.clone(),
                subject: "math".to_owned(),
                state: CaptureBatchState::Collecting,
                now_utc_ms: 2,
            },
        )
        .expect("create batch");
        let (shutdown, _) = watch::channel(false);
        let now = current_utc_millis();
        let state = Arc::new(ServerState {
            session_id: "session-1".to_owned(),
            context: CaptureLanContext {
                connection: Arc::new(Mutex::new(connection)),
                blob_root: directory.path().join("assets"),
                asset_key: [91; 32],
                account_id: "account-1".to_owned(),
                profile_id: profile.id,
                batch_id: batch.id,
                notifier: Arc::new(|_| {}),
            },
            public_origin: "http://127.0.0.1:3210".to_owned(),
            expected_host: "127.0.0.1:3210".to_owned(),
            token_hash: Sha256::digest(TEST_TOKEN.as_bytes()).into(),
            sequence_base: 0,
            started_at_utc_ms: now,
            activity: Mutex::new(SessionActivity {
                last_activity_utc_ms: now,
                received_item_count: 0,
                received_bytes: 0,
                next_source_sequence: 0,
            }),
            upload_slots: Arc::new(Semaphore::new(2)),
            shutdown,
        });
        let router = build_router(Arc::clone(&state));
        Self {
            _directory: directory,
            state,
            router,
        }
    }

    fn request(&self, method: Method, uri: &str, body: Body) -> Request<Body> {
        Request::builder()
            .method(method)
            .uri(uri)
            .header(header::HOST, "127.0.0.1:3210")
            .header(header::ORIGIN, "http://127.0.0.1:3210")
            .header(header::AUTHORIZATION, format!("Bearer {TEST_TOKEN}"))
            .body(body)
            .expect("request")
    }
}

fn png(seed: u8) -> Vec<u8> {
    let image = DynamicImage::ImageRgba8(ImageBuffer::from_pixel(
        4,
        3,
        Rgba([seed, seed.wrapping_add(1), seed.wrapping_add(2), 255]),
    ));
    let mut output = Cursor::new(Vec::new());
    image
        .write_to(&mut output, ImageFormat::Png)
        .expect("encode png");
    output.into_inner()
}

fn split_color_png() -> Vec<u8> {
    let image = ImageBuffer::from_fn(8, 4, |x, _| {
        if x < 4 {
            Rgba([220, 30, 20, 255])
        } else {
            Rgba([20, 40, 220, 255])
        }
    });
    let mut output = Cursor::new(Vec::new());
    DynamicImage::ImageRgba8(image)
        .write_to(&mut output, ImageFormat::Png)
        .expect("encode split color png");
    output.into_inner()
}

#[test]
fn private_network_filter_accepts_only_rfc1918_addresses() {
    assert!(is_private_lan(Ipv4Addr::new(10, 0, 0, 1)));
    assert!(is_private_lan(Ipv4Addr::new(172, 16, 0, 1)));
    assert!(is_private_lan(Ipv4Addr::new(172, 31, 255, 254)));
    assert!(is_private_lan(Ipv4Addr::new(192, 168, 1, 2)));
    assert!(!is_private_lan(Ipv4Addr::LOCALHOST));
    assert!(!is_private_lan(Ipv4Addr::new(169, 254, 1, 1)));
    assert!(!is_private_lan(Ipv4Addr::new(8, 8, 8, 8)));
}

#[test]
fn multiple_interfaces_require_an_explicit_selection() {
    let addresses = vec![
        CaptureLanAddress {
            label: "wifi".to_owned(),
            address: "192.168.1.2".to_owned(),
        },
        CaptureLanAddress {
            label: "hotspot".to_owned(),
            address: "192.168.137.1".to_owned(),
        },
    ];
    assert!(matches!(
        select_address(&addresses, None),
        Err(CaptureLanError::AddressRequired)
    ));
    assert_eq!(
        select_address(&addresses, Some("192.168.137.1"))
            .unwrap()
            .label,
        "hotspot"
    );
    assert!(matches!(
        select_address(&addresses, Some("192.168.9.9")),
        Err(CaptureLanError::InvalidAddress)
    ));
}

#[test]
fn token_comparison_checks_every_byte() {
    let left = [7_u8; 32];
    let mut different = left;
    different[31] = 8;
    assert!(constant_time_eq(&left, &left));
    assert!(!constant_time_eq(&left, &different));
}

#[test]
fn api_rejects_an_invalid_token() {
    let server = TestServer::new();
    let runtime = tokio::runtime::Builder::new_current_thread()
        .enable_all()
        .build()
        .expect("runtime");
    let request = Request::builder()
        .uri("/api/v1/session")
        .header(header::HOST, "127.0.0.1:3210")
        .header(header::AUTHORIZATION, "Bearer wrong-token")
        .body(Body::empty())
        .expect("request");
    let response = runtime
        .block_on(server.router.oneshot(request))
        .expect("response");
    assert_eq!(response.status(), StatusCode::UNAUTHORIZED);
}

#[test]
fn api_rejects_wrong_origin_expired_sessions_and_forged_media_types() {
    let server = TestServer::new();
    let runtime = tokio::runtime::Builder::new_current_thread()
        .enable_all()
        .build()
        .expect("runtime");
    let wrong_origin = Request::builder()
        .uri("/api/v1/session")
        .header(header::HOST, "127.0.0.1:3210")
        .header(header::ORIGIN, "http://192.168.1.9:3210")
        .header(header::AUTHORIZATION, format!("Bearer {TEST_TOKEN}"))
        .body(Body::empty())
        .expect("request");
    let response = runtime
        .block_on(server.router.clone().oneshot(wrong_origin))
        .expect("response");
    assert_eq!(response.status(), StatusCode::UNAUTHORIZED);

    let mut forged = server.request(
        Method::PUT,
        &format!("/api/v1/uploads/{}", Uuid::now_v7()),
        Body::from(png(9)),
    );
    forged.headers_mut().insert(
        header::CONTENT_TYPE,
        "application/octet-stream".parse().expect("header"),
    );
    forged
        .headers_mut()
        .insert("x-source-sequence", "0".parse().expect("header"));
    let response = runtime
        .block_on(server.router.clone().oneshot(forged))
        .expect("response");
    assert_eq!(response.status(), StatusCode::UNSUPPORTED_MEDIA_TYPE);

    server
        .state
        .activity
        .lock()
        .expect("activity")
        .last_activity_utc_ms = current_utc_millis() - IDLE_TIMEOUT_MS - 1;
    let expired = server.request(Method::GET, "/api/v1/session", Body::empty());
    let response = runtime
        .block_on(server.router.clone().oneshot(expired))
        .expect("response");
    assert_eq!(response.status(), StatusCode::GONE);

    let connection = server.state.context.connection.lock().expect("connection");
    let item_count: i64 = connection
        .query_row(
            "SELECT COUNT(*) FROM capture_items WHERE batch_id = ?1",
            [&server.state.context.batch_id],
            |row| row.get(0),
        )
        .expect("item count");
    assert_eq!(item_count, 0);
}

#[test]
fn mobile_page_hardens_headers_and_keeps_heic_decoder_lazy() {
    let server = TestServer::new();
    let runtime = tokio::runtime::Builder::new_current_thread()
        .enable_all()
        .build()
        .expect("runtime");
    let page = runtime
        .block_on(
            server.router.clone().oneshot(
                Request::builder()
                    .uri("/mobile/")
                    .body(Body::empty())
                    .expect("page request"),
            ),
        )
        .expect("page response");
    assert_eq!(page.status(), StatusCode::OK);
    assert_eq!(
        page.headers()
            .get("x-content-type-options")
            .and_then(|value| value.to_str().ok()),
        Some("nosniff")
    );
    assert!(page.headers().contains_key("content-security-policy"));
    assert!(!MOBILE_PAGE.contains("<script src=\"/mobile/vendor/heic2any.js\""));
    assert!(MOBILE_PAGE.contains("const createClientId="));
    assert!(MOBILE_PAGE.contains("crypto.getRandomValues"));
    assert!(MOBILE_PAGE.contains("Array.from(input.files||[])"));
    assert!(MOBILE_PAGE.contains("restoreRemoteItems"));
    assert!(MOBILE_PAGE.contains("pumpRemotePreviews"));
    assert!(MOBILE_PAGE.contains("/items/${encodeURIComponent(item.serverId)}/preview"));
    assert!(MOBILE_PAGE.contains("grid-template-columns:76px minmax(0,1fr) auto"));
    assert!(MOBILE_PAGE.contains(".item>div { min-width:0"));
    assert!(MOBILE_PAGE.contains("overflow-wrap:anywhere"));
    assert!(MOBILE_PAGE.contains("overflow-x:hidden"));
    assert!(MOBILE_PAGE.contains("已选中，正在处理图片"));
    assert!(MOBILE_PAGE.contains("item.status==='preparing'||item.status==='pending'"));
    assert!(MOBILE_PAGE.contains("const workerCount=Math.min(2,pendingFiles.length)"));
    assert!(MOBILE_PAGE.contains("state.cameraLoop"));
    assert!(MOBILE_PAGE.contains("const canCapture="));
    assert!(MOBILE_PAGE.contains("state.session.state==='collecting'"));
    assert!(!MOBILE_PAGE.contains("if(finish.disabled)return"));
    assert!(!MOBILE_PAGE.contains("state.cameraLoop&&!finish.disabled"));
    assert!(MOBILE_PAGE.contains("快速拍一张"));
    assert!(MOBILE_PAGE.contains("继续拍一张"));
    assert!(MOBILE_PAGE.contains("MicroMessenger"));
    assert!(MOBILE_PAGE.contains("navigator.userActivation"));
    assert!(!MOBILE_PAGE.contains("开始连续拍照"));
    assert!(!MOBILE_PAGE.contains("正在打开下一张"));
    assert!(MOBILE_PAGE.contains("cameraInput.click()"));
    assert!(MOBILE_PAGE.contains("id=\"nextCamera\""));
    assert!(MOBILE_PAGE.contains("微信会拦截网页自动重开相机"));
    assert!(MOBILE_PAGE.contains("收起快拍"));
    assert!(MOBILE_PAGE.contains("id=\"reviewCapture\""));
    assert!(MOBILE_PAGE.contains("role=\"dialog\""));
    assert!(MOBILE_PAGE.contains("aria-modal=\"true\""));
    assert!(MOBILE_PAGE.contains("const openCropEditor="));
    assert!(MOBILE_PAGE.contains("const applyMobileCrop="));
    assert!(MOBILE_PAGE.contains("const restoreMobileCrop="));
    assert!(MOBILE_PAGE.contains("setPointerCapture"));
    assert!(MOBILE_PAGE.contains("/crop/revert"));
    assert!(MOBILE_PAGE.contains("rotationDegrees"));
    assert!(!MOBILE_PAGE.contains("body:croppedBlob"));
    assert!(MOBILE_PAGE.contains("animation:item-enter"));
    assert!(MOBILE_PAGE.contains("prefers-reduced-motion:reduce"));

    let decoder = runtime
        .block_on(
            server.router.oneshot(
                Request::builder()
                    .uri("/mobile/vendor/heic2any.js")
                    .body(Body::empty())
                    .expect("decoder request"),
            ),
        )
        .expect("decoder response");
    assert_eq!(decoder.status(), StatusCode::OK);
    assert_eq!(
        decoder
            .headers()
            .get(header::CONTENT_TYPE)
            .and_then(|value| value.to_str().ok()),
        Some("application/javascript; charset=utf-8")
    );
}

#[test]
fn authenticated_api_responses_are_never_cacheable() {
    let server = TestServer::new();
    let runtime = tokio::runtime::Builder::new_current_thread()
        .enable_all()
        .build()
        .expect("runtime");

    let success = runtime
        .block_on(server.router.clone().oneshot(server.request(
            Method::GET,
            "/api/v1/session",
            Body::empty(),
        )))
        .expect("session response");
    assert_eq!(
        success
            .headers()
            .get(header::CACHE_CONTROL)
            .and_then(|value| value.to_str().ok()),
        Some("no-store")
    );

    let unauthorized = runtime
        .block_on(
            server.router.oneshot(
                Request::builder()
                    .uri("/api/v1/session")
                    .header(header::HOST, "127.0.0.1:3210")
                    .header(header::AUTHORIZATION, "Bearer wrong-token")
                    .body(Body::empty())
                    .expect("unauthorized request"),
            ),
        )
        .expect("unauthorized response");
    assert_eq!(unauthorized.status(), StatusCode::UNAUTHORIZED);
    assert_eq!(
        unauthorized
            .headers()
            .get(header::CACHE_CONTROL)
            .and_then(|value| value.to_str().ok()),
        Some("no-store")
    );
}

#[test]
fn duplicate_upload_is_idempotent_and_finish_organizes_batch() {
    let server = TestServer::new();
    let runtime = tokio::runtime::Builder::new_current_thread()
        .enable_all()
        .build()
        .expect("runtime");
    let upload_id = Uuid::now_v7();
    for _ in 0..2 {
        let mut request = server.request(
            Method::PUT,
            &format!("/api/v1/uploads/{upload_id}"),
            Body::from(png(17)),
        );
        request
            .headers_mut()
            .insert("x-source-sequence", "0".parse().expect("header"));
        request
            .headers_mut()
            .insert(header::CONTENT_TYPE, "image/png".parse().expect("header"));
        request
            .headers_mut()
            .insert("x-source-name", "photo.png".parse().expect("header"));
        let response = runtime
            .block_on(server.router.clone().oneshot(request))
            .expect("upload response");
        assert_eq!(response.status(), StatusCode::OK);
    }

    let finish = server.request(Method::POST, "/api/v1/session/finish", Body::empty());
    let response = runtime
        .block_on(server.router.clone().oneshot(finish))
        .expect("finish response");
    assert_eq!(response.status(), StatusCode::NO_CONTENT);

    let connection = server.state.context.connection.lock().expect("connection");
    let detail = get_capture_batch_detail(
        &connection,
        &server.state.context.account_id,
        &server.state.context.profile_id,
        &server.state.context.batch_id,
    )
    .expect("batch detail");
    assert_eq!(detail.items.len(), 1);
    assert_eq!(detail.batch.state, CaptureBatchState::Organizing);
    assert_eq!(server.state.activity_snapshot().received_item_count, 1);
}

#[test]
fn session_rehydrates_uploaded_items_and_serves_a_preview() {
    let server = TestServer::new();
    let runtime = tokio::runtime::Builder::new_current_thread()
        .enable_all()
        .build()
        .expect("runtime");
    let upload_id = Uuid::now_v7();
    let mut upload = server.request(
        Method::PUT,
        &format!("/api/v1/uploads/{upload_id}"),
        Body::from(png(23)),
    );
    upload
        .headers_mut()
        .insert("x-source-sequence", "0".parse().expect("header"));
    upload
        .headers_mut()
        .insert(header::CONTENT_TYPE, "image/png".parse().expect("header"));
    upload
        .headers_mut()
        .insert("x-source-name", "photo.png".parse().expect("header"));
    let upload_response = runtime
        .block_on(server.router.clone().oneshot(upload))
        .expect("upload response");
    assert_eq!(upload_response.status(), StatusCode::OK);
    let upload_body = runtime
        .block_on(axum::body::to_bytes(upload_response.into_body(), 1_000_000))
        .expect("upload body");
    let upload_json: serde_json::Value = serde_json::from_slice(&upload_body).expect("json");
    let item_id = upload_json["itemId"].as_str().expect("item id").to_owned();

    let session_response = runtime
        .block_on(server.router.clone().oneshot(server.request(
            Method::GET,
            "/api/v1/session",
            Body::empty(),
        )))
        .expect("session response");
    let session_body = runtime
        .block_on(axum::body::to_bytes(
            session_response.into_body(),
            1_000_000,
        ))
        .expect("session body");
    let session_json: serde_json::Value = serde_json::from_slice(&session_body).expect("json");
    assert_eq!(session_json["items"][0]["itemId"], item_id);
    assert_eq!(session_json["items"][0]["sourceName"], "photo.png");

    let preview_response = runtime
        .block_on(server.router.clone().oneshot(server.request(
            Method::GET,
            &format!("/api/v1/items/{item_id}/preview"),
            Body::empty(),
        )))
        .expect("preview response");
    assert_eq!(preview_response.status(), StatusCode::OK);
    let preview_body = runtime
        .block_on(axum::body::to_bytes(
            preview_response.into_body(),
            2_000_000,
        ))
        .expect("preview body");
    let preview_json: serde_json::Value = serde_json::from_slice(&preview_body).expect("json");
    assert!(
        preview_json["dataUrl"]
            .as_str()
            .is_some_and(|value| value.starts_with("data:image/png;base64,"))
    );
}

#[test]
fn mobile_crop_is_non_destructive_and_reversible() {
    let server = TestServer::new();
    let runtime = tokio::runtime::Builder::new_current_thread()
        .enable_all()
        .build()
        .expect("runtime");
    let upload_id = Uuid::now_v7();
    let mut upload = server.request(
        Method::PUT,
        &format!("/api/v1/uploads/{upload_id}"),
        Body::from(split_color_png()),
    );
    upload
        .headers_mut()
        .insert("x-source-sequence", "0".parse().expect("header"));
    upload
        .headers_mut()
        .insert(header::CONTENT_TYPE, "image/png".parse().expect("header"));
    upload
        .headers_mut()
        .insert("x-source-name", "worksheet.png".parse().expect("header"));
    let upload_response = runtime
        .block_on(server.router.clone().oneshot(upload))
        .expect("upload response");
    let upload_body = runtime
        .block_on(axum::body::to_bytes(upload_response.into_body(), 1_000_000))
        .expect("upload body");
    let upload_json: serde_json::Value = serde_json::from_slice(&upload_body).expect("json");
    let source_item_id = upload_json["itemId"]
        .as_str()
        .expect("source item id")
        .to_owned();
    let revision_before_invalid = {
        let connection = server.state.context.connection.lock().expect("connection");
        get_capture_batch_detail(
            &connection,
            &server.state.context.account_id,
            &server.state.context.profile_id,
            &server.state.context.batch_id,
        )
        .expect("detail")
        .batch
        .revision
    };

    let invalid_body = serde_json::json!({
        "rect": { "x": 0.9, "y": 0.0, "width": 0.5, "height": 1.0 },
        "rotationDegrees": 0
    });
    let mut invalid = server.request(
        Method::POST,
        &format!("/api/v1/items/{source_item_id}/crop"),
        Body::from(invalid_body.to_string()),
    );
    invalid.headers_mut().insert(
        header::CONTENT_TYPE,
        "application/json".parse().expect("header"),
    );
    let invalid_response = runtime
        .block_on(server.router.clone().oneshot(invalid))
        .expect("invalid crop response");
    assert_eq!(invalid_response.status(), StatusCode::BAD_REQUEST);
    {
        let connection = server.state.context.connection.lock().expect("connection");
        let detail = get_capture_batch_detail(
            &connection,
            &server.state.context.account_id,
            &server.state.context.profile_id,
            &server.state.context.batch_id,
        )
        .expect("detail after invalid crop");
        assert_eq!(detail.batch.revision, revision_before_invalid);
        assert_eq!(detail.items[0].id, source_item_id);
    }

    let crop_body = serde_json::json!({
        "rect": { "x": 0.0, "y": 0.0, "width": 0.5, "height": 1.0 },
        "rotationDegrees": 0
    });
    let mut crop = server.request(
        Method::POST,
        &format!("/api/v1/items/{source_item_id}/crop"),
        Body::from(crop_body.to_string()),
    );
    crop.headers_mut().insert(
        header::CONTENT_TYPE,
        "application/json".parse().expect("header"),
    );
    let crop_response = runtime
        .block_on(server.router.clone().oneshot(crop))
        .expect("crop response");
    assert_eq!(crop_response.status(), StatusCode::OK);
    let crop_body = runtime
        .block_on(axum::body::to_bytes(crop_response.into_body(), 1_000_000))
        .expect("crop response body");
    let crop_json: serde_json::Value = serde_json::from_slice(&crop_body).expect("json");
    let derived_item_id = crop_json["itemId"]
        .as_str()
        .expect("derived item id")
        .to_owned();
    let derivation_id = crop_json["cropDerivationId"]
        .as_str()
        .expect("crop derivation id")
        .to_owned();
    assert_ne!(derived_item_id, source_item_id);
    assert_eq!(crop_json["sourceItemId"], source_item_id);

    let session_response = runtime
        .block_on(server.router.clone().oneshot(server.request(
            Method::GET,
            "/api/v1/session",
            Body::empty(),
        )))
        .expect("session response");
    let session_body = runtime
        .block_on(axum::body::to_bytes(
            session_response.into_body(),
            1_000_000,
        ))
        .expect("session body");
    let session_json: serde_json::Value = serde_json::from_slice(&session_body).expect("json");
    assert_eq!(session_json["items"][0]["itemId"], derived_item_id);
    assert_eq!(session_json["items"][0]["cropDerivationId"], derivation_id);

    let preview_response = runtime
        .block_on(server.router.clone().oneshot(server.request(
            Method::GET,
            &format!("/api/v1/items/{derived_item_id}/preview"),
            Body::empty(),
        )))
        .expect("preview response");
    let preview_body = runtime
        .block_on(axum::body::to_bytes(
            preview_response.into_body(),
            2_000_000,
        ))
        .expect("preview body");
    let preview_json: serde_json::Value = serde_json::from_slice(&preview_body).expect("json");
    let preview_bytes = STANDARD
        .decode(
            preview_json["dataUrl"]
                .as_str()
                .expect("preview data url")
                .split_once(',')
                .expect("data url comma")
                .1,
        )
        .expect("preview base64");
    assert_eq!(
        image::load_from_memory(&preview_bytes)
            .expect("preview image")
            .to_rgba8()
            .get_pixel(0, 0)
            .0,
        [220, 30, 20, 255]
    );
    {
        let connection = server.state.context.connection.lock().expect("connection");
        let counts: (i64, i64) = connection
            .query_row(
                "SELECT (SELECT COUNT(*) FROM capture_items WHERE batch_id = ?1),
                            (SELECT COUNT(*) FROM asset_derivations WHERE batch_id = ?1)",
                [&server.state.context.batch_id],
                |row| Ok((row.get(0)?, row.get(1)?)),
            )
            .expect("crop counts");
        assert_eq!(counts, (2, 1), "source remains encrypted beside derivative");
    }

    let restore = server.request(
        Method::POST,
        &format!("/api/v1/items/{derived_item_id}/crop/revert"),
        Body::empty(),
    );
    let restore_response = runtime
        .block_on(server.router.clone().oneshot(restore))
        .expect("restore response");
    assert_eq!(restore_response.status(), StatusCode::NO_CONTENT);
    let connection = server.state.context.connection.lock().expect("connection");
    let detail = get_capture_batch_detail(
        &connection,
        &server.state.context.account_id,
        &server.state.context.profile_id,
        &server.state.context.batch_id,
    )
    .expect("restored detail");
    assert_eq!(detail.batch.state, CaptureBatchState::Collecting);
    assert_eq!(detail.items.len(), 1);
    assert_eq!(detail.items[0].id, source_item_id);
    let derivation_count: i64 = connection
        .query_row("SELECT COUNT(*) FROM asset_derivations", [], |row| {
            row.get(0)
        })
        .expect("derivation count");
    assert_eq!(derivation_count, 0);
}

#[test]
fn shutdown_signal_closes_the_listening_port() {
    let server = TestServer::new();
    let listener = TcpListener::bind((Ipv4Addr::LOCALHOST, 0)).expect("bind listener");
    listener.set_nonblocking(true).expect("nonblocking");
    let address = listener.local_addr().expect("listener address");
    let shutdown_receiver = server.state.shutdown.subscribe();
    let weak_sessions = WeakCaptureLanSessionRegistry::default();
    let state = Arc::clone(&server.state);
    let shutdown = state.shutdown.clone();
    let thread = thread::spawn(move || {
        tokio::runtime::Builder::new_current_thread()
            .enable_all()
            .build()
            .expect("runtime")
            .block_on(run_server(
                listener,
                state,
                shutdown_receiver,
                weak_sessions,
                "session-1".to_owned(),
            ));
    });

    wait_until_port_is_open(address);
    shutdown.send(true).expect("send shutdown");
    thread.join().expect("join server");
    assert!(TcpStream::connect_timeout(&address, Duration::from_millis(100)).is_err());
}

fn wait_until_port_is_open(address: SocketAddr) {
    for _ in 0..50 {
        if TcpStream::connect_timeout(&address, Duration::from_millis(50)).is_ok() {
            return;
        }
        thread::sleep(Duration::from_millis(10));
    }
    panic!("capture server did not start");
}
