use axum::{
    extract::{Path, Query, State},
    http::{header, HeaderMap},
    response::IntoResponse,
    Json,
};

use crate::{
    auth::require_any_permission,
    error::AppError,
    files::sprite_cache_path,
    graphics::sprites::{
        ensure_sprite_cached, load_sprite_font_config, sprite_sha256, validate_sprite_font_config,
        SpriteKind,
    },
    models::{ApiResponse, SpriteMeta, SpritePayload},
    state::RuntimeState,
};

use super::validate_sha256;

pub(super) async fn sprite_metadata(
    State(state): State<RuntimeState>,
    headers: HeaderMap,
    Query(payload): Query<SpritePayload>,
) -> Result<impl IntoResponse, AppError> {
    require_any_permission(&headers, &state.app).await?;
    let kind = validate_sprite_payload(&payload)?;
    let font_config = load_sprite_font_config().await?;
    validate_sprite_font_config(&font_config.parsed)?;
    let text = payload.text.trim().to_string();
    let sha256 = sprite_sha256(kind, &text, &font_config.raw);
    ensure_sprite_cached(&state, &sha256, text, font_config).await?;

    Ok(Json(ApiResponse::ok(SpriteMeta { sha256 })))
}

pub(super) async fn download_sprite(
    State(state): State<RuntimeState>,
    headers: HeaderMap,
    Path(sha256): Path<String>,
) -> Result<impl IntoResponse, AppError> {
    require_any_permission(&headers, &state.app).await?;
    validate_sha256(&sha256)?;
    let cache_path = sprite_cache_path(&state.app.data_dir, &sha256);
    if cache_path.exists() {
        let bytes = tokio::fs::read(cache_path)
            .await
            .map_err(|error| AppError::Internal(error.into()))?;
        return Ok(([(header::CONTENT_TYPE, "image/bmp")], bytes));
    }

    Err(AppError::NotFound("精灵图不存在".to_string()))
}

fn validate_sprite_payload(payload: &SpritePayload) -> Result<SpriteKind, AppError> {
    let kind = match payload.kind.as_str() {
        "caption" => SpriteKind::Caption,
        "date" => SpriteKind::Date,
        "status" => SpriteKind::Status,
        _ => return Err(AppError::BadRequest("精灵图类型不正确".to_string())),
    };
    let text = payload.text.trim();
    if text.is_empty() {
        return Err(AppError::BadRequest("精灵图文字不能为空".to_string()));
    }
    if text.chars().count() > 64 {
        return Err(AppError::BadRequest(
            "精灵图文字不能超过 64 个字符".to_string(),
        ));
    }
    Ok(kind)
}
