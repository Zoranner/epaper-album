use std::{
    path::{Path, PathBuf},
    sync::atomic::{AtomicU64, Ordering},
    sync::Arc,
};

use axum::{
    body::Body,
    http::{header, request::Builder, Request, StatusCode},
};
use epaper_album_server::{
    db::{self, Store},
    routes,
    state::{AdminSession, AppState},
};
use http_body_util::BodyExt;
use serde_json::{json, Value};
use sqlx::{sqlite::SqlitePoolOptions, SqlitePool};
use tokio::sync::Mutex;
use tower::ServiceExt;

static TEST_ID: AtomicU64 = AtomicU64::new(1);

pub struct TestApp {
    pub app: axum::Router,
    pub pool: SqlitePool,
    pub data_dir: PathBuf,
    pub admin_token: String,
}

pub async fn test_app() -> TestApp {
    test_app_with_options(chrono::Utc::now() + chrono::Duration::hours(1)).await
}

pub async fn test_app_with_options(
    admin_token_expires_at: chrono::DateTime<chrono::Utc>,
) -> TestApp {
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
        admin_session: Arc::new(Mutex::new(AdminSession::new(
            admin_token.clone(),
            admin_token_expires_at,
        ))),
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

pub async fn request_json(app: axum::Router, request: Request<Body>) -> (StatusCode, Value) {
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

pub fn user_request(uri: &str) -> Builder {
    Request::builder()
        .uri(uri)
        .header("secret-key", "test-secret")
}

pub fn admin_request(app: &TestApp, uri: &str) -> Builder {
    Request::builder()
        .uri(uri)
        .header(header::AUTHORIZATION, format!("Bearer {}", app.admin_token))
}

pub async fn login(app: &TestApp, username: &str, password: &str) -> (StatusCode, Value) {
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

pub async fn seed_image(pool: &SqlitePool, sha256: &str, status: &str, remark: &str) {
    let has_created_at = sqlx::query_scalar::<_, i64>(
        "SELECT COUNT(*) FROM pragma_table_info('images') WHERE name = 'created_at'",
    )
    .fetch_one(pool)
    .await
    .expect("inspect images columns")
        > 0;

    if has_created_at {
        sqlx::query(
            "INSERT INTO images (sha256, status, remark, created_at, updated_at) VALUES (?, ?, ?, ?, ?)",
        )
            .bind(sha256)
            .bind(status)
            .bind(remark)
            .bind("2026-06-20T00:00:00+00:00")
            .bind("2026-06-20T00:00:00+00:00")
            .execute(pool)
            .await
            .expect("seed image");
    } else {
        sqlx::query("INSERT INTO images (sha256, status, remark) VALUES (?, ?, ?)")
            .bind(sha256)
            .bind(status)
            .bind(remark)
            .execute(pool)
            .await
            .expect("seed image");
    }
}

pub async fn seed_image_tags(pool: &SqlitePool, sha256: &str, tags: &[&str]) {
    for tag in tags {
        sqlx::query("INSERT INTO image_tags (image, tag) VALUES (?, ?)")
            .bind(sha256)
            .bind(tag)
            .execute(pool)
            .await
            .expect("seed image tag");
    }
}

pub async fn image_status(pool: &SqlitePool, sha256: &str) -> String {
    sqlx::query_scalar("SELECT status FROM images WHERE sha256 = ?")
        .bind(sha256)
        .fetch_one(pool)
        .await
        .expect("image status")
}

pub async fn image_remark(pool: &SqlitePool, sha256: &str) -> String {
    sqlx::query_scalar("SELECT remark FROM images WHERE sha256 = ?")
        .bind(sha256)
        .fetch_one(pool)
        .await
        .expect("image remark")
}

pub async fn image_exists(pool: &SqlitePool, sha256: &str) -> bool {
    sqlx::query_scalar::<_, i64>("SELECT 1 FROM images WHERE sha256 = ?")
        .bind(sha256)
        .fetch_optional(pool)
        .await
        .expect("image exists")
        .is_some()
}

pub async fn insert_plan(pool: &SqlitePool, date: &str, caption: &str, image: &str) {
    sqlx::query("INSERT INTO plans (date, caption, image) VALUES (?, ?, ?)")
        .bind(date)
        .bind(caption)
        .bind(image)
        .execute(pool)
        .await
        .expect("insert plan");
}

pub fn valid_sha(byte: u8) -> String {
    format!("{byte:064x}")
}

pub fn tiny_png() -> Vec<u8> {
    let mut bytes = Vec::new();
    image::DynamicImage::ImageRgb8(image::RgbImage::from_pixel(1, 1, image::Rgb([0, 0, 0])))
        .write_to(
            &mut std::io::Cursor::new(&mut bytes),
            image::ImageFormat::Png,
        )
        .expect("encode tiny png");
    bytes
}

pub fn multipart_body(boundary: &str, image: &[u8], remark: Option<&str>) -> Vec<u8> {
    multipart_body_with_tags(boundary, image, remark, None)
}

pub fn multipart_body_with_tags(
    boundary: &str,
    image: &[u8],
    remark: Option<&str>,
    tags: Option<&[&str]>,
) -> Vec<u8> {
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

    if let Some(tags) = tags {
        body.extend_from_slice(format!("--{boundary}\r\n").as_bytes());
        body.extend_from_slice(b"Content-Disposition: form-data; name=\"tags\"\r\n\r\n");
        body.extend_from_slice(serde_json::to_string(tags).expect("tags json").as_bytes());
        body.extend_from_slice(b"\r\n");
    }

    body.extend_from_slice(format!("--{boundary}--\r\n").as_bytes());
    body
}

pub fn write_display_file(data_dir: &Path, sha256: &str, bytes: &[u8]) {
    std::fs::write(
        data_dir
            .join("images")
            .join("display")
            .join(format!("{sha256}.bmp")),
        bytes,
    )
    .expect("write display file");
}

pub fn sprite_font_assets_available() -> bool {
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
