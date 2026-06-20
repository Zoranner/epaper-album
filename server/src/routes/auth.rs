use axum::{extract::State, response::IntoResponse, Json};
use chrono::{Duration, Utc};

use crate::{
    error::AppError,
    models::{ApiResponse, LoginRequest, LoginResponse},
    state::{AdminSession, RuntimeState},
};

pub(super) async fn healthz() -> &'static str {
    "ok"
}

pub(super) async fn login(
    State(state): State<RuntimeState>,
    Json(payload): Json<LoginRequest>,
) -> Result<impl IntoResponse, AppError> {
    if payload.username == state.app.admin_username && payload.password == state.app.admin_password
    {
        let mut session = state.app.admin_session.lock().await;
        let expires_at = Utc::now() + Duration::hours(24);
        *session = AdminSession::new(uuid::Uuid::new_v4().to_string(), expires_at);
        Ok(Json(ApiResponse::ok(LoginResponse {
            jwt_token: session.token.clone(),
            expires_at: session.expires_at.to_rfc3339(),
        })))
    } else {
        Err(AppError::Unauthorized)
    }
}
