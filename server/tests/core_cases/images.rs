use std::sync::Arc;

use axum::{
    body::Body,
    http::{header, StatusCode},
};
use http_body_util::BodyExt;
use inkframe_server::{
    db::Store,
    routes,
    state::{AdminSession, AppState},
};
use serde_json::{json, Value};
use sha2::Digest;
use tokio::sync::Mutex;
use tower::ServiceExt;

use super::common::*;

#[tokio::test]
async fn image_list_requires_admin_and_remark_update_returns_updated_image() {
    let app = test_app().await;
    let sha = valid_sha(17);
    seed_image(&app.pool, &sha, "ready", "旧备注").await;
    seed_image_tags(&app.pool, &sha, &["旧标签"]).await;

    let (status, value) = request_json(
        app.app.clone(),
        user_request("/api/images")
            .body(Body::empty())
            .expect("request"),
    )
    .await;
    assert_eq!(status, StatusCode::UNAUTHORIZED);
    assert_eq!(value["code"], 401);

    let body = json!({ "remark": "新备注", "tags": ["家庭", "旅行", "家庭"] });
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
    assert_eq!(value["data"]["tags"], json!(["家庭", "旅行"]));
    assert_eq!(value["data"]["createdAt"], "2026-06-20T00:00:00+00:00");
    assert!(value["data"]["updatedAt"].as_str().is_some());
    assert_eq!(image_remark(&app.pool, &sha).await, "新备注");
}

#[tokio::test]
async fn image_upload_and_list_support_tags_filter() {
    let app = test_app().await;
    let boundary = "X-BOUNDARY";
    let png = tiny_png();
    let expected_sha = hex::encode(sha2::Sha256::digest(&png));

    let (status, value) = request_json(
        app.app.clone(),
        admin_request(&app, "/api/images")
            .method("POST")
            .header(
                header::CONTENT_TYPE,
                format!("multipart/form-data; boundary={boundary}"),
            )
            .body(Body::from(multipart_body_with_tags(
                boundary,
                &png,
                Some("带标签"),
                Some(&["家庭", "旅行", "家庭"]),
            )))
            .expect("request"),
    )
    .await;
    assert_eq!(status, StatusCode::CREATED);
    assert_eq!(value["data"]["sha256"], expected_sha);
    assert_eq!(value["data"]["tags"], json!(["家庭", "旅行"]));
    assert!(value["data"]["createdAt"].as_str().is_some());
    assert!(value["data"]["updatedAt"].as_str().is_some());

    let other_sha = valid_sha(44);
    seed_image(&app.pool, &other_sha, "ready", "家庭").await;
    seed_image_tags(&app.pool, &other_sha, &["家庭"]).await;

    let (_, filtered_value) = request_json(
        app.app.clone(),
        admin_request(
            &app,
            "/api/images?tags=%E5%AE%B6%E5%BA%AD,%E6%97%85%E8%A1%8C",
        )
        .body(Body::empty())
        .expect("request"),
    )
    .await;
    let images = filtered_value["data"].as_array().expect("images");
    assert_eq!(images.len(), 1);
    assert_eq!(images[0]["sha256"], expected_sha);
}

#[tokio::test]
async fn image_delete_clears_plan_references_and_removes_image() {
    let app = test_app().await;
    let sha = valid_sha(18);
    seed_image(&app.pool, &sha, "ready", "待删").await;
    insert_plan(&app.pool, "2026-06-06", "待重新编辑", &sha).await;
    write_display_file(&app.data_dir, &sha, b"BMready");

    let (status, value) = request_json(
        app.app.clone(),
        admin_request(&app, &format!("/api/images/{sha}"))
            .method("DELETE")
            .body(Body::empty())
            .expect("request"),
    )
    .await;

    assert_eq!(status, StatusCode::OK);
    assert_eq!(value["data"], Value::Null);
    assert!(!image_exists(&app.pool, &sha).await);
    assert!(!app
        .data_dir
        .join("images")
        .join("display")
        .join(format!("{sha}.bmp"))
        .exists());

    let (_, admin_value) = request_json(
        app.app.clone(),
        admin_request(&app, "/api/plans")
            .body(Body::empty())
            .expect("request"),
    )
    .await;
    assert_eq!(admin_value["data"][0]["caption"], "待重新编辑");
    assert_eq!(admin_value["data"][0]["image"], "");

    let (_, user_value) = request_json(
        app.app.clone(),
        user_request("/api/plans")
            .body(Body::empty())
            .expect("request"),
    )
    .await;
    assert_eq!(user_value["data"], json!([]));
}

#[tokio::test]
async fn image_redither_marks_image_pending_and_removes_display_cache() {
    let app = test_app().await;
    let sha = valid_sha(33);
    seed_image(&app.pool, &sha, "ready", "重新抖动").await;
    write_display_file(&app.data_dir, &sha, b"BMold");

    let (status, value) = request_json(
        app.app.clone(),
        admin_request(&app, &format!("/api/images/{sha}/redither"))
            .method("POST")
            .body(Body::empty())
            .expect("request"),
    )
    .await;

    assert_eq!(status, StatusCode::OK);
    assert_eq!(value["data"]["sha256"], sha);
    assert_eq!(value["data"]["status"], "pending");
    assert_eq!(image_status(&app.pool, &sha).await, "pending");
    assert!(!app
        .data_dir
        .join("images")
        .join("display")
        .join(format!("{sha}.bmp"))
        .exists());
}

#[tokio::test]
async fn upload_deduplicates_same_image_and_requeues_failed_image() {
    let app = test_app().await;
    let image_bytes = tiny_png();
    let expected_sha = hex::encode(sha2::Sha256::digest(&image_bytes));
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
                &image_bytes,
                Some("第一次"),
            )))
            .expect("request"),
    )
    .await;
    assert_eq!(status, StatusCode::CREATED);
    assert_eq!(value["data"]["sha256"], expected_sha);
    assert_eq!(value["data"]["status"], "pending");
    assert_eq!(image_remark(&app.pool, &expected_sha).await, "第一次");
    assert!(app
        .data_dir
        .join("images")
        .join("original")
        .join(format!("{expected_sha}.png"))
        .exists());

    sqlx::query("UPDATE images SET status = 'failed', updated_at = ? WHERE sha256 = ?")
        .bind("2026-06-20T00:01:00+00:00")
        .bind(&expected_sha)
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
                &image_bytes,
                Some("重试"),
            )))
            .expect("request"),
    )
    .await;
    assert_eq!(status, StatusCode::CREATED);
    assert_eq!(value["data"]["sha256"], expected_sha);
    assert_eq!(value["data"]["status"], "pending");
    assert_eq!(image_status(&app.pool, &expected_sha).await, "pending");
    assert_eq!(image_remark(&app.pool, &expected_sha).await, "重试");
}

#[tokio::test]
async fn upload_original_file_uses_detected_extension() {
    let app = test_app().await;
    let boundary = "X-BOUNDARY";
    let png = tiny_png();
    let expected_sha = hex::encode(sha2::Sha256::digest(&png));

    let (status, value) = request_json(
        app.app.clone(),
        admin_request(&app, "/api/images")
            .method("POST")
            .header(
                header::CONTENT_TYPE,
                format!("multipart/form-data; boundary={boundary}"),
            )
            .body(Body::from(multipart_body(boundary, &png, None)))
            .expect("request"),
    )
    .await;

    assert_eq!(status, StatusCode::CREATED);
    assert_eq!(value["data"]["sha256"], expected_sha);
    assert!(app
        .data_dir
        .join("images")
        .join("original")
        .join(format!("{expected_sha}.png"))
        .exists());
}

#[tokio::test]
async fn recover_ready_image_requires_display_bmp_file() {
    let app = test_app().await;
    let sha = valid_sha(21);
    seed_image(&app.pool, &sha, "ready", "legacy display").await;
    std::fs::write(
        app.data_dir.join("images").join("display").join(&sha),
        b"BMlegacy",
    )
    .expect("write legacy display file");

    routes::recover_and_enqueue_pending(&AppState {
        store: Store::new(app.pool.clone()),
        secret_key: "test-secret".to_string(),
        admin_username: "admin".to_string(),
        admin_password: "password".to_string(),
        admin_session: Arc::new(Mutex::new(AdminSession::new(
            app.admin_token.clone(),
            chrono::Utc::now() + chrono::Duration::hours(1),
        ))),
        data_dir: app.data_dir.clone(),
        enqueue_processing: false,
    })
    .await
    .expect("recover pending");

    assert_eq!(image_status(&app.pool, &sha).await, "pending");
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
