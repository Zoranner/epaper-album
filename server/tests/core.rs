use axum::{
    body::Body,
    http::{header, Request, StatusCode},
    response::IntoResponse,
};
use epaper_album_server::{
    db::{self, NewImage, NewPlanEntry, Store},
    error::AppError,
    models::{PlanEntry, PlanResponse},
    routes::{self, AppState},
};
use http_body_util::BodyExt;
use sqlx::sqlite::SqlitePoolOptions;
use tower::ServiceExt;

async fn test_store() -> Store {
    let pool = SqlitePoolOptions::new()
        .max_connections(1)
        .connect("sqlite::memory:")
        .await
        .expect("connect in-memory sqlite");
    db::init_schema(&pool).await.expect("init schema");
    Store::new(pool)
}

#[test]
fn plan_response_json_matches_device_contract() {
    let response = PlanResponse {
        version: "2026-06-06-001".to_string(),
        plans: vec![PlanEntry {
            start: "2026-06-06".to_string(),
            end: "2026-06-06".to_string(),
            caption: "晚风和海".to_string(),
            images: vec![
                "7f83b1657ff1fc53b92dc18148a1d65dfc2d4b1fa3d677284addd200126d9069".to_string(),
            ],
        }],
    };

    let value = serde_json::to_value(response).expect("serialize plan response");

    assert_eq!(
        value,
        serde_json::json!({
            "version": "2026-06-06-001",
            "plans": [
                {
                    "start": "2026-06-06",
                    "end": "2026-06-06",
                    "caption": "晚风和海",
                    "images": [
                        "7f83b1657ff1fc53b92dc18148a1d65dfc2d4b1fa3d677284addd200126d9069"
                    ]
                }
            ]
        })
    );
    assert!(value["plans"][0].get("url").is_none());
    assert!(value["plans"][0].get("base_url").is_none());
}

#[tokio::test]
async fn store_persists_and_loads_plan_response_ordered_by_start() {
    let store = test_store().await;

    store
        .replace_plan_entries(
            "2026-06-06-001",
            &[
                NewPlanEntry {
                    start: "2026-06-07".to_string(),
                    end: "2026-06-07".to_string(),
                    caption: "第二天".to_string(),
                    images: vec!["hash-b".to_string()],
                },
                NewPlanEntry {
                    start: "2026-06-06".to_string(),
                    end: "2026-06-06".to_string(),
                    caption: "第一天".to_string(),
                    images: vec!["hash-a".to_string(), "hash-b".to_string()],
                },
            ],
        )
        .await
        .expect("replace plan entries");

    let response = store
        .load_plan_response()
        .await
        .expect("load plan response")
        .expect("plan response exists");

    assert_eq!(response.version, "2026-06-06-001");
    assert_eq!(response.plans.len(), 2);
    assert_eq!(response.plans[0].start, "2026-06-06");
    assert_eq!(response.plans[0].images, vec!["hash-a", "hash-b"]);
    assert_eq!(response.plans[1].start, "2026-06-07");
}

#[tokio::test]
async fn replacing_plan_entries_removes_stale_plan_rows() {
    let store = test_store().await;

    store
        .replace_plan_entries(
            "old",
            &[NewPlanEntry {
                start: "2026-06-06".to_string(),
                end: "2026-06-06".to_string(),
                caption: "旧计划".to_string(),
                images: vec!["old-hash".to_string()],
            }],
        )
        .await
        .expect("insert old plan");
    store
        .replace_plan_entries(
            "new",
            &[NewPlanEntry {
                start: "2026-06-07".to_string(),
                end: "2026-06-07".to_string(),
                caption: "新计划".to_string(),
                images: vec!["new-hash".to_string()],
            }],
        )
        .await
        .expect("replace with new plan");

    let response = store
        .load_plan_response()
        .await
        .expect("load plan response")
        .expect("plan response exists");

    assert_eq!(response.version, "new");
    assert_eq!(response.plans.len(), 1);
    assert_eq!(response.plans[0].caption, "新计划");
    assert_eq!(response.plans[0].images, vec!["new-hash"]);
}

#[tokio::test]
async fn store_persists_image_metadata_and_blob() {
    let store = test_store().await;

    store
        .upsert_image(NewImage {
            sha256: "7f83b1657ff1fc53b92dc18148a1d65dfc2d4b1fa3d677284addd200126d9069",
            content_type: "image/bmp",
            bytes: &[1, 2, 3, 4],
        })
        .await
        .expect("upsert image");

    let image = store
        .get_image("7f83b1657ff1fc53b92dc18148a1d65dfc2d4b1fa3d677284addd200126d9069")
        .await
        .expect("get image")
        .expect("image exists");

    assert_eq!(
        image.sha256,
        "7f83b1657ff1fc53b92dc18148a1d65dfc2d4b1fa3d677284addd200126d9069"
    );
    assert_eq!(image.content_type, "image/bmp");
    assert_eq!(image.bytes, vec![1, 2, 3, 4]);
}

#[tokio::test]
async fn app_error_serializes_http_error_body() {
    let response = AppError::Unauthorized.into_response();
    assert_eq!(response.status(), StatusCode::UNAUTHORIZED);

    let bytes = response
        .into_body()
        .collect()
        .await
        .expect("collect body")
        .to_bytes();
    let value: serde_json::Value = serde_json::from_slice(&bytes).expect("json body");

    assert_eq!(value, serde_json::json!({ "error": "Invalid secret-key" }));
}

#[tokio::test]
async fn manifest_route_requires_secret_key_and_returns_device_contract() {
    let store = test_store().await;
    store
        .replace_plan_entries(
            "2026-06-06-001",
            &[NewPlanEntry {
                start: "2026-06-06".to_string(),
                end: "2026-06-06".to_string(),
                caption: "晚风和海".to_string(),
                images: vec![
                    "7f83b1657ff1fc53b92dc18148a1d65dfc2d4b1fa3d677284addd200126d9069".to_string(),
                ],
            }],
        )
        .await
        .expect("seed plan");
    let app = routes::router(AppState {
        store,
        secret_key: "test-secret".to_string(),
    });

    let unauthorized = app
        .clone()
        .oneshot(
            Request::builder()
                .uri("/api/manifest")
                .body(Body::empty())
                .expect("request"),
        )
        .await
        .expect("unauthorized response");
    assert_eq!(unauthorized.status(), StatusCode::UNAUTHORIZED);

    let authorized = app
        .oneshot(
            Request::builder()
                .uri("/api/manifest")
                .header("secret-key", "test-secret")
                .body(Body::empty())
                .expect("request"),
        )
        .await
        .expect("authorized response");
    assert_eq!(authorized.status(), StatusCode::OK);

    let bytes = authorized
        .into_body()
        .collect()
        .await
        .expect("collect body")
        .to_bytes();
    let value: serde_json::Value = serde_json::from_slice(&bytes).expect("json body");

    assert_eq!(value["version"], "2026-06-06-001");
    assert_eq!(value["plans"][0]["caption"], "晚风和海");
    assert!(value["plans"][0].get("url").is_none());
}

#[tokio::test]
async fn image_route_requires_secret_key_and_returns_blob() {
    let store = test_store().await;
    let sha256 = "7f83b1657ff1fc53b92dc18148a1d65dfc2d4b1fa3d677284addd200126d9069";
    store
        .upsert_image(NewImage {
            sha256,
            content_type: "image/bmp",
            bytes: &[1, 2, 3, 4],
        })
        .await
        .expect("seed image");
    let app = routes::router(AppState {
        store,
        secret_key: "test-secret".to_string(),
    });

    let unauthorized = app
        .clone()
        .oneshot(
            Request::builder()
                .uri(format!("/images/{sha256}"))
                .body(Body::empty())
                .expect("request"),
        )
        .await
        .expect("unauthorized response");
    assert_eq!(unauthorized.status(), StatusCode::UNAUTHORIZED);

    let authorized = app
        .oneshot(
            Request::builder()
                .uri(format!("/images/{sha256}"))
                .header("secret-key", "test-secret")
                .body(Body::empty())
                .expect("request"),
        )
        .await
        .expect("authorized response");
    assert_eq!(authorized.status(), StatusCode::OK);
    assert_eq!(
        authorized.headers().get(header::CONTENT_TYPE),
        Some(&"image/bmp".parse().expect("content type"))
    );

    let bytes = authorized
        .into_body()
        .collect()
        .await
        .expect("collect body")
        .to_bytes();
    assert_eq!(bytes.as_ref(), &[1, 2, 3, 4]);
}
