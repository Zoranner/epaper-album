use axum::{
    body::Body,
    http::{header, StatusCode},
};
use http_body_util::BodyExt;
use serde_json::Value;
use tower::ServiceExt;

use super::common::*;

#[tokio::test]
async fn sprite_rejects_invalid_kind() {
    let app = test_app().await;

    let (status, value) = request_json(
        app.app.clone(),
        admin_request(&app, "/api/sprites?type=badge&text=caption")
            .body(Body::empty())
            .expect("request"),
    )
    .await;
    assert_eq!(status, StatusCode::BAD_REQUEST);
    assert_eq!(value["code"], 400);
}

#[tokio::test]
async fn sprite_meta_returns_sha256_and_sprite_download_uses_data_cache() {
    if !sprite_font_assets_available() {
        eprintln!("skip sprite success test: fixed font files are not installed");
        return;
    }

    let app = test_app().await;
    let sprite_uri = "/api/sprites?type=caption&text=Album%E6%99%9A%E9%A3%8E2026";

    let (status, value) = request_json(
        app.app.clone(),
        user_request(sprite_uri)
            .body(Body::empty())
            .expect("request"),
    )
    .await;

    assert_eq!(status, StatusCode::OK);
    let sha256 = value["data"]["sha256"].as_str().expect("sprite sha256");
    assert_eq!(sha256.len(), 64);

    let download_uri = format!("/sprites/{sha256}");
    let response = app
        .app
        .clone()
        .oneshot(
            user_request(&download_uri)
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
    assert!(image.pixels().any(|pixel| pixel.0 == [0, 255, 0, 255]));
    assert!(image.pixels().any(|pixel| pixel.0 == [0, 0, 0, 255]));
    assert!(image.pixels().any(|pixel| pixel.0 == [255, 255, 255, 255]));
    assert!(image.pixels().all(|pixel| {
        matches!(
            pixel.0,
            [0, 255, 0, 255] | [0, 0, 0, 255] | [255, 255, 255, 255]
        )
    }));

    let cache_path = app.data_dir.join("sprites").join(format!("{sha256}.bmp"));
    assert!(cache_path.exists());
    std::fs::write(cache_path, b"BMcached").expect("overwrite sprite cache");

    let response = app
        .app
        .clone()
        .oneshot(
            user_request(&download_uri)
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
async fn sprite_accepts_status_type() {
    if !sprite_font_assets_available() {
        eprintln!("skip sprite type test: fixed font files are not installed");
        return;
    }

    let app = test_app().await;

    let response = app
        .app
        .clone()
        .oneshot(
            user_request("/api/sprites?type=status&text=OFFLINE")
                .body(Body::empty())
                .expect("request"),
        )
        .await
        .expect("response");
    assert_eq!(response.status(), StatusCode::OK);
    let bytes = response
        .into_body()
        .collect()
        .await
        .expect("collect body")
        .to_bytes();
    let value: Value = serde_json::from_slice(&bytes).expect("json sprite metadata");
    let sha256 = value["data"]["sha256"].as_str().expect("sprite sha256");
    assert_eq!(sha256.len(), 64);
}

#[tokio::test]
async fn sprite_rejects_notice_type() {
    let app = test_app().await;

    let response = app
        .app
        .clone()
        .oneshot(
            user_request("/api/sprites?type=notice&text=OFFLINE")
                .body(Body::empty())
                .expect("request"),
        )
        .await
        .expect("response");

    assert_eq!(response.status(), StatusCode::BAD_REQUEST);
}

#[tokio::test]
async fn sprite_download_rejects_invalid_sha256() {
    let app = test_app().await;

    let (status, value) = request_json(
        app.app.clone(),
        user_request("/sprites/not-a-sha")
            .body(Body::empty())
            .expect("request"),
    )
    .await;

    assert_eq!(status, StatusCode::BAD_REQUEST);
    assert_eq!(value["code"], 400);
}

#[tokio::test]
async fn sprite_download_returns_404_for_missing_cache() {
    let app = test_app().await;
    let sha256 = valid_sha(31);

    let (status, value) = request_json(
        app.app.clone(),
        user_request(&format!("/sprites/{sha256}"))
            .body(Body::empty())
            .expect("request"),
    )
    .await;

    assert_eq!(status, StatusCode::NOT_FOUND);
    assert_eq!(value["code"], 404);
}
