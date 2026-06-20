use axum::{
    body::Body,
    http::{header, StatusCode},
};
use serde_json::json;

use super::common::*;

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
