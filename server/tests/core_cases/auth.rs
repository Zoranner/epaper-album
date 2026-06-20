use axum::{
    body::Body,
    http::{header, Request, StatusCode},
    response::IntoResponse,
};
use http_body_util::BodyExt;
use inkframe_server::error::AppError;
use serde_json::{json, Value};

use super::common::*;
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
