use std::{
    path::{Path, PathBuf},
    sync::atomic::{AtomicU64, Ordering},
};

use axum::{
    body::Body,
    http::{header, request::Builder, Request, StatusCode},
    response::IntoResponse,
};
use epaper_album_server::{
    db::{self, Store},
    error::AppError,
    routes::{self, AppState},
};
use http_body_util::BodyExt;
use serde_json::{json, Value};
use sqlx::{sqlite::SqlitePoolOptions, SqlitePool};
use tower::ServiceExt;

static TEST_ID: AtomicU64 = AtomicU64::new(1);

struct TestApp {
    app: axum::Router,
    pool: SqlitePool,
    data_dir: PathBuf,
    admin_token: String,
}

async fn test_app() -> TestApp {
    test_app_with_options(chrono::Utc::now() + chrono::Duration::hours(1)).await
}

async fn test_app_with_options(admin_token_expires_at: chrono::DateTime<chrono::Utc>) -> TestApp {
    let id = TEST_ID.fetch_add(1, Ordering::Relaxed);
    let data_dir = std::env::temp_dir().join(format!(
        "epaper-album-server-test-{}-{id}",
        std::process::id()
    ));
    std::fs::create_dir_all(data_dir.join("images").join("original")).expect("create original dir");
    std::fs::create_dir_all(data_dir.join("images").join("display")).expect("create display dir");
    std::fs::create_dir_all(data_dir.join("sprites")).expect("create sprites dir");

    let pool = SqlitePoolOptions::new()
        .max_connections(1)
        .connect("sqlite::memory:")
        .await
        .expect("connect in-memory sqlite");
    db::init_schema(&pool).await.expect("init schema");

    let admin_token = "test-admin-token".to_string();
    let state = AppState {
        store: Store::new(pool.clone()),
        secret_key: "test-secret".to_string(),
        admin_username: "admin".to_string(),
        admin_password: "password".to_string(),
        admin_token: admin_token.clone(),
        admin_token_expires_at,
        data_dir: data_dir.clone(),
        enqueue_processing: false,
    };

    TestApp {
        app: routes::router(state),
        pool,
        data_dir,
        admin_token,
    }
}

async fn request_json(app: axum::Router, request: Request<Body>) -> (StatusCode, Value) {
    let response = app.oneshot(request).await.expect("response");
    let status = response.status();
    let bytes = response
        .into_body()
        .collect()
        .await
        .expect("collect body")
        .to_bytes();
    let value = serde_json::from_slice(&bytes).expect("json response");
    (status, value)
}

fn user_request(uri: &str) -> Builder {
    Request::builder()
        .uri(uri)
        .header("secret-key", "test-secret")
}

fn admin_request(app: &TestApp, uri: &str) -> Builder {
    Request::builder()
        .uri(uri)
        .header(header::AUTHORIZATION, format!("Bearer {}", app.admin_token))
}

async fn login(app: &TestApp, username: &str, password: &str) -> (StatusCode, Value) {
    request_json(
        app.app.clone(),
        Request::builder()
            .method("POST")
            .uri("/api/login")
            .header(header::CONTENT_TYPE, "application/json")
            .body(Body::from(
                json!({ "username": username, "password": password }).to_string(),
            ))
            .expect("request"),
    )
    .await
}

async fn seed_image(pool: &SqlitePool, sha256: &str, status: &str, remark: &str) {
    sqlx::query("INSERT INTO images (sha256, status, remark) VALUES (?, ?, ?)")
        .bind(sha256)
        .bind(status)
        .bind(remark)
        .execute(pool)
        .await
        .expect("seed image");
}

async fn image_status(pool: &SqlitePool, sha256: &str) -> String {
    sqlx::query_scalar("SELECT status FROM images WHERE sha256 = ?")
        .bind(sha256)
        .fetch_one(pool)
        .await
        .expect("image status")
}

async fn image_remark(pool: &SqlitePool, sha256: &str) -> String {
    sqlx::query_scalar("SELECT remark FROM images WHERE sha256 = ?")
        .bind(sha256)
        .fetch_one(pool)
        .await
        .expect("image remark")
}

async fn insert_plan(pool: &SqlitePool, date: &str, caption: &str, image_sha256: &str) {
    sqlx::query("INSERT INTO plans (date, caption, image_sha256) VALUES (?, ?, ?)")
        .bind(date)
        .bind(caption)
        .bind(image_sha256)
        .execute(pool)
        .await
        .expect("insert plan");
}

fn valid_sha(byte: u8) -> String {
    format!("{byte:064x}")
}

fn multipart_body(boundary: &str, image: &[u8], remark: Option<&str>) -> Vec<u8> {
    let mut body = Vec::new();
    body.extend_from_slice(format!("--{boundary}\r\n").as_bytes());
    body.extend_from_slice(
        b"Content-Disposition: form-data; name=\"image\"; filename=\"image.bin\"\r\n",
    );
    body.extend_from_slice(b"Content-Type: application/octet-stream\r\n\r\n");
    body.extend_from_slice(image);
    body.extend_from_slice(b"\r\n");

    if let Some(remark) = remark {
        body.extend_from_slice(format!("--{boundary}\r\n").as_bytes());
        body.extend_from_slice(b"Content-Disposition: form-data; name=\"remark\"\r\n\r\n");
        body.extend_from_slice(remark.as_bytes());
        body.extend_from_slice(b"\r\n");
    }

    body.extend_from_slice(format!("--{boundary}--\r\n").as_bytes());
    body
}

fn write_display_file(data_dir: &Path, sha256: &str, bytes: &[u8]) {
    std::fs::write(data_dir.join("images").join("display").join(sha256), bytes)
        .expect("write display file");
}

fn sprite_font_assets_available() -> bool {
    let Ok(config) = std::fs::read_to_string("assets/fonts.toml") else {
        return false;
    };
    let Ok(value) = config.parse::<toml::Value>() else {
        return false;
    };
    value
        .get("files")
        .and_then(toml::Value::as_array)
        .is_some_and(|files| {
            !files.is_empty()
                && files.iter().all(|file| {
                    file.as_str()
                        .map(|file_name| Path::new("assets/fonts").join(file_name).exists())
                        .unwrap_or(false)
                })
        })
}

#[tokio::test]
async fn app_error_serializes_unified_error_body() {
    let response = AppError::Unauthorized.into_response();
    assert_eq!(response.status(), StatusCode::UNAUTHORIZED);

    let bytes = response
        .into_body()
        .collect()
        .await
        .expect("collect body")
        .to_bytes();
    let value: Value = serde_json::from_slice(&bytes).expect("json body");

    assert_eq!(
        value,
        json!({
            "code": 401,
            "message": "Unauthorized",
            "data": null
        })
    );
}

#[tokio::test]
async fn login_returns_jwt_token_expiration_and_rejects_bad_credentials() {
    let app = test_app().await;

    let (status, value) = login(&app, "admin", "password").await;
    assert_eq!(status, StatusCode::OK);
    assert_eq!(value["code"], 0);
    assert_eq!(value["message"], "ok");
    assert_eq!(value["data"]["jwtToken"], app.admin_token);
    let expires_at = value["data"]["expiresAt"]
        .as_str()
        .expect("expiresAt string");
    assert!(
        chrono::DateTime::parse_from_rfc3339(expires_at).expect("expiresAt rfc3339")
            > chrono::Utc::now()
    );

    let (status, value) = login(&app, "admin", "bad").await;
    assert_eq!(status, StatusCode::UNAUTHORIZED);
    assert_eq!(value["code"], 401);
    assert_eq!(value["data"], Value::Null);
}

#[tokio::test]
async fn expired_admin_token_is_rejected() {
    let app = test_app_with_options(chrono::Utc::now() - chrono::Duration::seconds(1)).await;

    let (status, value) = request_json(
        app.app.clone(),
        admin_request(&app, "/api/images")
            .body(Body::empty())
            .expect("request"),
    )
    .await;
    assert_eq!(status, StatusCode::UNAUTHORIZED);
    assert_eq!(value["code"], 401);
}

#[tokio::test]
async fn plans_days_parameter_is_lenient_and_clamped() {
    let app = test_app().await;
    let today = chrono::Local::now().date_naive();
    let previous_old = (today - chrono::Duration::days(5))
        .format("%Y-%m-%d")
        .to_string();
    let previous_recent = (today - chrono::Duration::days(1))
        .format("%Y-%m-%d")
        .to_string();
    let day0 = today.format("%Y-%m-%d").to_string();
    let day2 = (today + chrono::Duration::days(2))
        .format("%Y-%m-%d")
        .to_string();
    let day6 = (today + chrono::Duration::days(6))
        .format("%Y-%m-%d")
        .to_string();
    let day7 = (today + chrono::Duration::days(7))
        .format("%Y-%m-%d")
        .to_string();

    let ready_sha = valid_sha(20);
    seed_image(&app.pool, &ready_sha, "ready", "ready").await;
    insert_plan(&app.pool, &previous_old, "old", &ready_sha).await;
    insert_plan(&app.pool, &previous_recent, "recent", &ready_sha).await;
    insert_plan(&app.pool, &day0, "today", &ready_sha).await;
    insert_plan(&app.pool, &day2, "day2", &ready_sha).await;
    insert_plan(&app.pool, &day6, "day6", &ready_sha).await;
    insert_plan(&app.pool, &day7, "day7", &ready_sha).await;

    let (_, default_value) = request_json(
        app.app.clone(),
        user_request("/api/plans?days=abc")
            .body(Body::empty())
            .expect("request"),
    )
    .await;
    let default_plans = default_value["data"].as_array().expect("plans");
    assert_eq!(default_plans.len(), 3);
    assert_eq!(default_plans[0]["date"], previous_recent);
    assert_eq!(default_plans[0]["caption"], "recent");

    let (_, min_value) = request_json(
        app.app.clone(),
        user_request("/api/plans?days=0")
            .body(Body::empty())
            .expect("request"),
    )
    .await;
    let min_plans = min_value["data"].as_array().expect("plans");
    assert_eq!(min_plans.len(), 2);
    assert_eq!(min_plans[0]["date"], previous_recent);
    assert_eq!(min_plans[1]["date"], day0);

    let (_, max_value) = request_json(
        app.app.clone(),
        user_request("/api/plans?days=99")
            .body(Body::empty())
            .expect("request"),
    )
    .await;
    let dates = max_value["data"]
        .as_array()
        .expect("plans")
        .iter()
        .map(|item| item["date"].as_str().expect("date"))
        .collect::<Vec<_>>();
    assert_eq!(dates, vec![previous_recent, day0, day2, day6]);
}

#[tokio::test]
async fn plan_create_uses_date_key_and_rejects_unknown_sha256() {
    let app = test_app().await;
    let sha_a = valid_sha(10);
    let sha_unknown = valid_sha(12);
    seed_image(&app.pool, &sha_a, "ready", "A").await;

    let body = json!({
        "date": "2026-06-06",
        "caption": "创建计划",
        "image_sha256": sha_a
    });
    let (status, value) = request_json(
        app.app.clone(),
        admin_request(&app, "/api/plans")
            .method("POST")
            .header(header::CONTENT_TYPE, "application/json")
            .body(Body::from(body.to_string()))
            .expect("request"),
    )
    .await;
    assert_eq!(status, StatusCode::CREATED);
    assert_eq!(value["data"]["date"], "2026-06-06");
    assert_eq!(value["data"]["caption"], "创建计划");
    assert_eq!(value["data"]["image_sha256"], valid_sha(10));

    let bad_body = json!({
        "date": "2026-06-07",
        "caption": "未知图片",
        "image_sha256": sha_unknown
    });
    let (status, value) = request_json(
        app.app.clone(),
        admin_request(&app, "/api/plans")
            .method("POST")
            .header(header::CONTENT_TYPE, "application/json")
            .body(Body::from(bad_body.to_string()))
            .expect("request"),
    )
    .await;
    assert_eq!(status, StatusCode::BAD_REQUEST);
    assert_eq!(value["code"], 400);
}

#[tokio::test]
async fn plan_create_rejects_invalid_date() {
    let app = test_app().await;
    let sha = valid_sha(13);
    seed_image(&app.pool, &sha, "ready", "A").await;

    let body = json!({
        "date": "2026-06-99",
        "caption": "无效日期",
        "image_sha256": sha
    });
    let (status, value) = request_json(
        app.app.clone(),
        admin_request(&app, "/api/plans")
            .method("POST")
            .header(header::CONTENT_TYPE, "application/json")
            .body(Body::from(body.to_string()))
            .expect("request"),
    )
    .await;

    assert_eq!(status, StatusCode::BAD_REQUEST);
    assert_eq!(value["code"], 400);
}

#[tokio::test]
async fn init_schema_replaces_incompatible_legacy_tables() {
    let pool = SqlitePoolOptions::new()
        .max_connections(1)
        .connect("sqlite::memory:")
        .await
        .expect("connect in-memory sqlite");

    sqlx::query("CREATE TABLE images (sha256 TEXT PRIMARY KEY, content_type TEXT NOT NULL, bytes BLOB NOT NULL)")
        .execute(&pool)
        .await
        .expect("create legacy images");
    sqlx::query("CREATE TABLE plans (caption TEXT NOT NULL)")
        .execute(&pool)
        .await
        .expect("create legacy plans");

    db::init_schema(&pool).await.expect("init schema");

    sqlx::query("INSERT INTO images (sha256, status, remark) VALUES (?, 'pending', '')")
        .bind(valid_sha(14))
        .execute(&pool)
        .await
        .expect("insert current image");
    sqlx::query("INSERT INTO plans (date, caption, image_sha256) VALUES (?, ?, ?)")
        .bind("2026-06-06")
        .bind("current")
        .bind(valid_sha(14))
        .execute(&pool)
        .await
        .expect("insert current plan");
}

#[tokio::test]
async fn plan_update_by_date_and_returns_404_for_missing_plan() {
    let app = test_app().await;
    let sha_a = valid_sha(15);
    let sha_b = valid_sha(16);
    seed_image(&app.pool, &sha_a, "ready", "A").await;
    seed_image(&app.pool, &sha_b, "ready", "B").await;
    insert_plan(&app.pool, "2026-06-06", "旧计划", &sha_a).await;

    let body = json!({
        "date": "2026-06-07",
        "caption": "更新计划",
        "image_sha256": sha_b
    });
    let (status, value) = request_json(
        app.app.clone(),
        admin_request(&app, "/api/plans/2026-06-06")
            .method("PUT")
            .header(header::CONTENT_TYPE, "application/json")
            .body(Body::from(body.to_string()))
            .expect("request"),
    )
    .await;
    assert_eq!(status, StatusCode::OK);
    assert_eq!(value["data"]["date"], "2026-06-07");
    assert_eq!(value["data"]["caption"], "更新计划");
    assert_eq!(value["data"]["image_sha256"], valid_sha(16));

    let missing_body = json!({
        "date": "2026-06-08",
        "caption": "缺失计划",
        "image_sha256": sha_a
    });
    let (status, value) = request_json(
        app.app.clone(),
        admin_request(&app, "/api/plans/2026-06-09")
            .method("PUT")
            .header(header::CONTENT_TYPE, "application/json")
            .body(Body::from(missing_body.to_string()))
            .expect("request"),
    )
    .await;
    assert_eq!(status, StatusCode::NOT_FOUND);
    assert_eq!(value["code"], 404);
}

#[tokio::test]
async fn plans_return_ready_dates_for_users_and_all_dates_for_admins() {
    let app = test_app().await;
    let ready_sha = valid_sha(1);
    let failed_sha = valid_sha(2);
    seed_image(&app.pool, &ready_sha, "ready", "可用").await;
    seed_image(&app.pool, &failed_sha, "failed", "失败").await;
    let today = chrono::Local::now().date_naive();
    let day0 = today.format("%Y-%m-%d").to_string();
    let day1 = (today + chrono::Duration::days(1))
        .format("%Y-%m-%d")
        .to_string();
    insert_plan(&app.pool, &day0, "可用计划", &ready_sha).await;
    insert_plan(&app.pool, &day1, "失败计划", &failed_sha).await;

    let (_, user_value) = request_json(
        app.app.clone(),
        user_request("/api/plans")
            .body(Body::empty())
            .expect("request"),
    )
    .await;
    assert_eq!(
        user_value["data"],
        json!([{ "date": day0, "caption": "可用计划", "image_sha256": ready_sha }])
    );

    let (_, admin_value) = request_json(
        app.app.clone(),
        admin_request(&app, "/api/plans")
            .body(Body::empty())
            .expect("request"),
    )
    .await;
    assert_eq!(admin_value["data"][0]["caption"], "可用计划");
    assert_eq!(admin_value["data"][0]["image_sha256"], valid_sha(1));
    assert_eq!(admin_value["data"][1]["caption"], "失败计划");
    assert_eq!(admin_value["data"][1]["image_sha256"], valid_sha(2));
}

#[tokio::test]
async fn image_list_requires_admin_and_remark_update_returns_updated_image() {
    let app = test_app().await;
    let sha = valid_sha(17);
    seed_image(&app.pool, &sha, "ready", "旧备注").await;

    let (status, value) = request_json(
        app.app.clone(),
        user_request("/api/images")
            .body(Body::empty())
            .expect("request"),
    )
    .await;
    assert_eq!(status, StatusCode::UNAUTHORIZED);
    assert_eq!(value["code"], 401);

    let body = json!({ "remark": "新备注" });
    let (status, value) = request_json(
        app.app.clone(),
        admin_request(&app, &format!("/api/images/{sha}"))
            .method("PUT")
            .header(header::CONTENT_TYPE, "application/json")
            .body(Body::from(body.to_string()))
            .expect("request"),
    )
    .await;
    assert_eq!(status, StatusCode::OK);
    assert_eq!(value["data"]["remark"], "新备注");
    assert_eq!(image_remark(&app.pool, &sha).await, "新备注");
}

#[tokio::test]
async fn upload_deduplicates_same_image_and_requeues_failed_image() {
    let app = test_app().await;
    let image_bytes = b"same-image-content";
    let expected_sha = "4d61acb7dcd2fe36cf64353d5422105675e6e7eeac7ed960f45d3ca79e358f45";
    let boundary = "X-BOUNDARY";

    let (status, value) = request_json(
        app.app.clone(),
        admin_request(&app, "/api/images")
            .method("POST")
            .header(
                header::CONTENT_TYPE,
                format!("multipart/form-data; boundary={boundary}"),
            )
            .body(Body::from(multipart_body(
                boundary,
                image_bytes,
                Some("第一次"),
            )))
            .expect("request"),
    )
    .await;
    assert_eq!(status, StatusCode::CREATED);
    assert_eq!(value["data"]["sha256"], expected_sha);
    assert_eq!(value["data"]["status"], "pending");
    assert_eq!(image_remark(&app.pool, expected_sha).await, "第一次");
    assert!(app
        .data_dir
        .join("images")
        .join("original")
        .join(expected_sha)
        .exists());

    sqlx::query("UPDATE images SET status = 'failed' WHERE sha256 = ?")
        .bind(expected_sha)
        .execute(&app.pool)
        .await
        .expect("mark failed");

    let (status, value) = request_json(
        app.app.clone(),
        admin_request(&app, "/api/images")
            .method("POST")
            .header(
                header::CONTENT_TYPE,
                format!("multipart/form-data; boundary={boundary}"),
            )
            .body(Body::from(multipart_body(
                boundary,
                image_bytes,
                Some("重试"),
            )))
            .expect("request"),
    )
    .await;
    assert_eq!(status, StatusCode::CREATED);
    assert_eq!(value["data"]["sha256"], expected_sha);
    assert_eq!(value["data"]["status"], "pending");
    assert_eq!(image_status(&app.pool, expected_sha).await, "pending");
    assert_eq!(image_remark(&app.pool, expected_sha).await, "重试");
}

#[tokio::test]
async fn download_returns_ready_bmp_and_404_does_not_change_status() {
    let app = test_app().await;
    let ready_sha = valid_sha(3);
    let missing_file_sha = valid_sha(4);
    seed_image(&app.pool, &ready_sha, "ready", "ready").await;
    seed_image(&app.pool, &missing_file_sha, "ready", "missing").await;
    write_display_file(&app.data_dir, &ready_sha, b"BMready");

    let response = app
        .app
        .clone()
        .oneshot(
            user_request(&format!("/images/{ready_sha}"))
                .body(Body::empty())
                .expect("request"),
        )
        .await
        .expect("response");
    assert_eq!(response.status(), StatusCode::OK);
    assert_eq!(
        response.headers().get(header::CONTENT_TYPE),
        Some(&"image/bmp".parse().expect("content type"))
    );
    let bytes = response
        .into_body()
        .collect()
        .await
        .expect("collect body")
        .to_bytes();
    assert_eq!(bytes.as_ref(), b"BMready");

    let (status, value) = request_json(
        app.app.clone(),
        user_request(&format!("/images/{missing_file_sha}"))
            .body(Body::empty())
            .expect("request"),
    )
    .await;
    assert_eq!(status, StatusCode::NOT_FOUND);
    assert_eq!(value["code"], 404);
    assert_eq!(image_status(&app.pool, &missing_file_sha).await, "ready");
}

#[tokio::test]
async fn sprite_rejects_invalid_kind() {
    let app = test_app().await;

    let (status, value) = request_json(
        app.app.clone(),
        admin_request(&app, "/api/sprite?type=badge&text=caption")
            .body(Body::empty())
            .expect("request"),
    )
    .await;
    assert_eq!(status, StatusCode::BAD_REQUEST);
    assert_eq!(value["code"], 400);
}

#[tokio::test]
async fn sprite_returns_bmp_and_uses_data_cache() {
    if !sprite_font_assets_available() {
        eprintln!("skip sprite success test: fixed font files are not installed");
        return;
    }

    let app = test_app().await;
    let sprite_uri = "/api/sprite?type=caption&text=Album%E6%99%9A%E9%A3%8E2026";

    let response = app
        .app
        .clone()
        .oneshot(
            user_request(sprite_uri)
                .body(Body::empty())
                .expect("request"),
        )
        .await
        .expect("response");

    assert_eq!(response.status(), StatusCode::OK);
    assert_eq!(
        response.headers().get(header::CONTENT_TYPE),
        Some(&"image/bmp".parse().expect("content type"))
    );
    let bytes = response
        .into_body()
        .collect()
        .await
        .expect("collect body")
        .to_bytes();
    assert!(bytes.starts_with(b"BM"));

    let image = image::load_from_memory(&bytes)
        .expect("decode text image bmp")
        .to_rgba8();
    assert!(image
        .pixels()
        .all(|pixel| { matches!(pixel.0, [0, 0, 0, 255] | [255, 255, 255, 255]) }));

    let cache_files = std::fs::read_dir(app.data_dir.join("sprites"))
        .expect("read sprite cache dir")
        .collect::<Result<Vec<_>, _>>()
        .expect("sprite cache entries");
    assert_eq!(cache_files.len(), 1);
    assert_eq!(
        cache_files[0]
            .path()
            .extension()
            .and_then(|value| value.to_str()),
        Some("bmp")
    );
    std::fs::write(cache_files[0].path(), b"BMcached").expect("overwrite sprite cache");

    let response = app
        .app
        .clone()
        .oneshot(
            user_request(sprite_uri)
                .body(Body::empty())
                .expect("request"),
        )
        .await
        .expect("response");
    assert_eq!(response.status(), StatusCode::OK);
    let cached_bytes = response
        .into_body()
        .collect()
        .await
        .expect("collect cached body")
        .to_bytes();
    assert_eq!(cached_bytes.as_ref(), b"BMcached");
}

#[tokio::test]
async fn sprite_accepts_notice_and_status_types() {
    if !sprite_font_assets_available() {
        eprintln!("skip sprite type test: fixed font files are not installed");
        return;
    }

    let app = test_app().await;

    for kind in ["notice", "status"] {
        let response = app
            .app
            .clone()
            .oneshot(
                user_request(&format!("/api/sprite?type={kind}&text=OFFLINE"))
                    .body(Body::empty())
                    .expect("request"),
            )
            .await
            .expect("response");
        assert_eq!(response.status(), StatusCode::OK);
        assert_eq!(
            response.headers().get(header::CONTENT_TYPE),
            Some(&"image/bmp".parse().expect("content type"))
        );
    }
}
