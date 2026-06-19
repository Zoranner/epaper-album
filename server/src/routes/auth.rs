use axum::http::{header, HeaderMap};
use chrono::Utc;

use crate::error::AppError;

use super::state::{AppState, Permission};

pub(super) async fn require_any_permission(
    headers: &HeaderMap,
    state: &AppState,
) -> Result<Permission, AppError> {
    if is_admin(headers, state).await {
        return Ok(Permission::Admin);
    }
    if headers
        .get("secret-key")
        .and_then(|value| value.to_str().ok())
        .is_some_and(|value| value == state.secret_key)
    {
        return Ok(Permission::User);
    }
    Err(AppError::Unauthorized)
}

pub(super) async fn require_admin(headers: &HeaderMap, state: &AppState) -> Result<(), AppError> {
    if is_admin(headers, state).await {
        Ok(())
    } else {
        Err(AppError::Unauthorized)
    }
}

async fn is_admin(headers: &HeaderMap, state: &AppState) -> bool {
    let session = state.admin_session.lock().await;
    if Utc::now() >= session.expires_at {
        return false;
    }

    headers
        .get(header::AUTHORIZATION)
        .and_then(|value| value.to_str().ok())
        .and_then(|value| value.strip_prefix("Bearer "))
        .is_some_and(|token| token == session.token)
}
