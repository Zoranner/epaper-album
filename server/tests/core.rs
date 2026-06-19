use std::sync::Arc;

use axum::{
    body::Body,
    http::{header, Request, StatusCode},
    response::IntoResponse,
};
use epaper_album_server::{
    db::{self, Store},
    error::AppError,
    routes::{self, AdminSession, AppState},
};
use http_body_util::BodyExt;
use serde_json::{json, Value};
use sha2::Digest;
use sqlx::sqlite::SqlitePoolOptions;
use tokio::sync::Mutex;
use tower::ServiceExt;

mod common;

use common::*;

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
            "message": "登录信息已失效，请重新登录",
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
    let token = value["data"]["jwtToken"].as_str().expect("jwtToken string");
    assert!(!token.is_empty());
    assert_ne!(token, app.admin_token);
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
async fn login_refreshes_expired_admin_session() {
    let app = test_app_with_options(chrono::Utc::now() - chrono::Duration::seconds(1)).await;

    let (status, value) = login(&app, "admin", "password").await;
    assert_eq!(status, StatusCode::OK);
    let token = value["data"]["jwtToken"].as_str().expect("jwtToken string");
    let expires_at = value["data"]["expiresAt"]
        .as_str()
        .expect("expiresAt string");
    assert_ne!(token, app.admin_token);
    assert!(
        chrono::DateTime::parse_from_rfc3339(expires_at).expect("expiresAt rfc3339")
            > chrono::Utc::now()
    );

    let (status, value) = request_json(
        app.app.clone(),
        Request::builder()
            .uri("/api/images")
            .header(header::AUTHORIZATION, format!("Bearer {token}"))
            .body(Body::empty())
            .expect("request"),
    )
    .await;
    assert_eq!(status, StatusCode::OK);
    assert_eq!(value["code"], 0);
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
        "image": sha_a
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
    assert_eq!(value["data"]["image"], valid_sha(10));

    let bad_body = json!({
        "date": "2026-06-07",
        "caption": "未知图片",
        "image": sha_unknown
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
        "image": sha
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
    sqlx::query("INSERT INTO plans (date, caption, image) VALUES (?, ?, ?)")
        .bind("2026-06-06")
        .bind("current")
        .bind(valid_sha(14))
        .execute(&pool)
        .await
        .expect("insert current plan");
}

#[tokio::test]
async fn init_schema_replaces_legacy_plan_foreign_key_table() {
    let pool = SqlitePoolOptions::new()
        .max_connections(1)
        .connect("sqlite::memory:")
        .await
        .expect("connect in-memory sqlite");

    sqlx::query(
        r#"
        CREATE TABLE images (
            sha256 TEXT PRIMARY KEY,
            status TEXT NOT NULL,
            remark TEXT NOT NULL DEFAULT ''
        )
        "#,
    )
    .execute(&pool)
    .await
    .expect("create current images");
    sqlx::query(
        r#"
        CREATE TABLE plans (
            date TEXT PRIMARY KEY,
            caption TEXT NOT NULL,
            image TEXT NOT NULL,
            FOREIGN KEY (image) REFERENCES images(sha256)
        )
        "#,
    )
    .execute(&pool)
    .await
    .expect("create legacy plans");

    db::init_schema(&pool).await.expect("init schema");

    sqlx::query("INSERT INTO plans (date, caption, image) VALUES (?, ?, '')")
        .bind("2026-06-06")
        .bind("empty image")
        .execute(&pool)
        .await
        .expect("insert empty image plan");
}

#[tokio::test]
async fn plan_rows_with_invalid_dates_return_errors_without_panicking() {
    let pool = SqlitePoolOptions::new()
        .max_connections(1)
        .connect("sqlite::memory:")
        .await
        .expect("connect in-memory sqlite");
    sqlx::query(
        r#"
        CREATE TABLE plans (
            date TEXT PRIMARY KEY,
            caption TEXT NOT NULL,
            type TEXT NOT NULL DEFAULT 'fixed',
            image TEXT NOT NULL,
            tags TEXT NOT NULL DEFAULT '[]'
        )
        "#,
    )
    .execute(&pool)
    .await
    .expect("create legacy plans");
    sqlx::query(
        r#"
        CREATE TABLE images (
            sha256 TEXT PRIMARY KEY,
            status TEXT NOT NULL CHECK (status IN ('pending', 'processing', 'ready', 'failed')),
            remark TEXT NOT NULL DEFAULT ''
        )
        "#,
    )
    .execute(&pool)
    .await
    .expect("create images");
    let sha = valid_sha(61);
    seed_image(&pool, &sha, "ready", "").await;
    sqlx::query("INSERT INTO plans (date, caption, image) VALUES (?, ?, ?)")
        .bind("2026-06-99")
        .bind("bad date")
        .bind(&sha)
        .execute(&pool)
        .await
        .expect("insert dirty plan");

    let store = Store::new(pool);
    let result = store
        .list_admin_plans(
            protocol::LocalDate::parse("2026-01-01").expect("start"),
            protocol::LocalDate::parse("2026-12-31").expect("end"),
        )
        .await;

    assert!(result.is_err());
    assert!(result
        .expect_err("dirty date should return an error")
        .to_string()
        .contains("Stored plan date is invalid"));
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
        "image": sha_b
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
    assert_eq!(value["data"]["image"], valid_sha(16));

    let missing_body = json!({
        "date": "2026-06-08",
        "caption": "缺失计划",
        "image": sha_a
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
async fn plan_update_accepts_empty_image_and_can_select_image_again() {
    let app = test_app().await;
    let sha = valid_sha(32);
    seed_image(&app.pool, &sha, "ready", "A").await;
    insert_plan(&app.pool, "2026-06-06", "旧计划", &sha).await;

    let empty_body = json!({
        "date": "2026-06-06",
        "caption": "暂不显示",
        "image": ""
    });
    let (status, value) = request_json(
        app.app.clone(),
        admin_request(&app, "/api/plans/2026-06-06")
            .method("PUT")
            .header(header::CONTENT_TYPE, "application/json")
            .body(Body::from(empty_body.to_string()))
            .expect("request"),
    )
    .await;
    assert_eq!(status, StatusCode::OK);
    assert_eq!(value["data"]["image"], "");

    let selected_body = json!({
        "date": "2026-06-06",
        "caption": "恢复显示",
        "image": sha
    });
    let (status, value) = request_json(
        app.app.clone(),
        admin_request(&app, "/api/plans/2026-06-06")
            .method("PUT")
            .header(header::CONTENT_TYPE, "application/json")
            .body(Body::from(selected_body.to_string()))
            .expect("request"),
    )
    .await;
    assert_eq!(status, StatusCode::OK);
    assert_eq!(value["data"]["caption"], "恢复显示");
    assert_eq!(value["data"]["image"], sha);
}

#[tokio::test]
async fn random_plan_returns_ready_image_matching_all_tags_for_users() {
    let app = test_app().await;
    let family_trip = valid_sha(41);
    let family_only = valid_sha(42);
    let pending_match = valid_sha(43);
    seed_image(&app.pool, &family_trip, "ready", "家庭旅行").await;
    seed_image(&app.pool, &family_only, "ready", "家庭").await;
    seed_image(&app.pool, &pending_match, "pending", "待处理家庭旅行").await;
    seed_image_tags(&app.pool, &family_trip, &["家庭", "旅行"]).await;
    seed_image_tags(&app.pool, &family_only, &["家庭"]).await;
    seed_image_tags(&app.pool, &pending_match, &["家庭", "旅行"]).await;

    let today = chrono::Local::now()
        .date_naive()
        .format("%Y-%m-%d")
        .to_string();
    let body = json!({
        "date": today,
        "caption": "随机家庭旅行",
        "type": "random",
        "image": "",
        "tags": ["家庭", "旅行"]
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
    assert_eq!(value["data"]["type"], "random");
    assert_eq!(value["data"]["tags"], json!(["家庭", "旅行"]));

    let (_, user_value) = request_json(
        app.app.clone(),
        user_request("/api/plans")
            .body(Body::empty())
            .expect("request"),
    )
    .await;
    assert_eq!(
        user_value["data"],
        json!([{ "date": today, "caption": "随机家庭旅行", "image": family_trip }])
    );
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
        json!([{ "date": day0, "caption": "可用计划", "image": ready_sha }])
    );

    let (_, admin_value) = request_json(
        app.app.clone(),
        admin_request(&app, "/api/plans")
            .body(Body::empty())
            .expect("request"),
    )
    .await;
    assert_eq!(admin_value["data"][0]["caption"], "可用计划");
    assert_eq!(admin_value["data"][0]["image"], valid_sha(1));
    assert_eq!(admin_value["data"][1]["caption"], "失败计划");
    assert_eq!(admin_value["data"][1]["image"], valid_sha(2));
}

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
