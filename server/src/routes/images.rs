use std::collections::HashMap;

use axum::{
    body::Bytes,
    extract::{Multipart, Path, Query, State},
    http::{header, HeaderMap, StatusCode},
    response::IntoResponse,
    Json,
};
use protocol::ImageStatus;
use sha2::{Digest, Sha256};

use crate::{
    auth::{require_admin, require_any_permission},
    error::AppError,
    files::{display_image_path, original_image_path, remove_file_if_exists, remove_image_files},
    graphics::images::detect_uploaded_image_format,
    models::{null_data, ApiResponse, ImagePayload},
    state::RuntimeState,
};

use super::{enqueue_image, validate_sha256};

pub(super) async fn list_images(
    State(state): State<RuntimeState>,
    headers: HeaderMap,
    Query(params): Query<HashMap<String, String>>,
) -> Result<impl IntoResponse, AppError> {
    require_admin(&headers, &state.app).await?;
    let images = state
        .app
        .store
        .list_images(
            params.get("keyword").map(String::as_str),
            &parse_tags_param(params.get("tags").map(String::as_str)),
        )
        .await?;
    Ok(Json(ApiResponse::ok(images)))
}

pub(super) async fn upload_image(
    State(state): State<RuntimeState>,
    headers: HeaderMap,
    mut multipart: Multipart,
) -> Result<impl IntoResponse, AppError> {
    require_admin(&headers, &state.app).await?;

    let mut image: Option<Bytes> = None;
    let mut remark: Option<String> = None;
    let mut tags: Option<Vec<String>> = None;

    while let Some(field) = multipart
        .next_field()
        .await
        .map_err(|_| AppError::BadRequest("上传内容格式不正确".to_string()))?
    {
        match field.name() {
            Some("image") => {
                let bytes = field
                    .bytes()
                    .await
                    .map_err(|_| AppError::BadRequest("图片字段读取失败".to_string()))?;
                image = Some(bytes);
            }
            Some("remark") => {
                let value = field
                    .text()
                    .await
                    .map_err(|_| AppError::BadRequest("备注字段读取失败".to_string()))?;
                remark = Some(value);
            }
            Some("tags") => {
                let value = field
                    .text()
                    .await
                    .map_err(|_| AppError::BadRequest("标签字段读取失败".to_string()))?;
                tags = Some(parse_tags_json(&value)?);
            }
            _ => {}
        }
    }

    let bytes = image.ok_or_else(|| AppError::BadRequest("请选择要上传的图片".to_string()))?;
    if bytes.is_empty() {
        return Err(AppError::BadRequest("图片文件不能为空".to_string()));
    }

    let format = detect_uploaded_image_format(&bytes)?;
    let sha256 = hex::encode(Sha256::digest(&bytes));
    let original_path = original_image_path(&state.app.data_dir, &sha256, format.extension());
    if !original_path.exists() {
        tokio::fs::write(&original_path, &bytes)
            .await
            .map_err(|error| AppError::Internal(error.into()))?;
    }

    let (image, should_enqueue) = state
        .app
        .store
        .upsert_uploaded_image(&sha256, remark.as_deref(), tags.as_deref())
        .await?;
    if should_enqueue {
        enqueue_image(&state, sha256).await;
    }

    Ok((StatusCode::CREATED, Json(ApiResponse::ok(image))))
}

pub(super) async fn update_image(
    State(state): State<RuntimeState>,
    headers: HeaderMap,
    Path(sha256): Path<String>,
    Json(payload): Json<ImagePayload>,
) -> Result<impl IntoResponse, AppError> {
    require_admin(&headers, &state.app).await?;
    validate_sha256(&sha256)?;
    let image = state
        .app
        .store
        .update_image(&sha256, &payload.remark, &payload.tags)
        .await?
        .ok_or_else(|| AppError::NotFound("图片不存在".to_string()))?;
    Ok(Json(ApiResponse::ok(image)))
}

pub(super) async fn delete_image(
    State(state): State<RuntimeState>,
    headers: HeaderMap,
    Path(sha256): Path<String>,
) -> Result<impl IntoResponse, AppError> {
    require_admin(&headers, &state.app).await?;
    validate_sha256(&sha256)?;
    if !state.app.store.delete_image(&sha256).await? {
        return Err(AppError::NotFound("图片不存在".to_string()));
    }

    remove_image_files(&state.app.data_dir, &sha256).await?;
    Ok(Json(ApiResponse::ok(null_data())))
}

pub(super) async fn redither_image(
    State(state): State<RuntimeState>,
    headers: HeaderMap,
    Path(sha256): Path<String>,
) -> Result<impl IntoResponse, AppError> {
    require_admin(&headers, &state.app).await?;
    validate_sha256(&sha256)?;
    remove_file_if_exists(display_image_path(&state.app.data_dir, &sha256)).await?;
    let image = state
        .app
        .store
        .requeue_image(&sha256)
        .await?
        .ok_or_else(|| AppError::NotFound("图片不存在".to_string()))?;
    enqueue_image(&state, sha256).await;
    Ok(Json(ApiResponse::ok(image)))
}

pub(super) async fn download_image(
    State(state): State<RuntimeState>,
    headers: HeaderMap,
    Path(sha256): Path<String>,
) -> Result<impl IntoResponse, AppError> {
    require_any_permission(&headers, &state.app).await?;
    validate_sha256(&sha256)?;

    let image = state
        .app
        .store
        .get_image(&sha256)
        .await?
        .ok_or_else(|| AppError::NotFound("图片不存在".to_string()))?;
    if image.status != ImageStatus::Ready {
        return Err(AppError::NotFound("图片不存在".to_string()));
    }

    let display_path = display_image_path(&state.app.data_dir, &sha256);
    if !display_path.exists() {
        return Err(AppError::NotFound("图片不存在".to_string()));
    }

    let bytes = tokio::fs::read(display_path)
        .await
        .map_err(|error| AppError::Internal(error.into()))?;
    Ok(([(header::CONTENT_TYPE, "image/bmp")], bytes))
}

fn parse_tags_json(value: &str) -> Result<Vec<String>, AppError> {
    let tags: Vec<String> = serde_json::from_str(value)
        .map_err(|_| AppError::BadRequest("标签字段格式不正确".to_string()))?;
    Ok(normalized_tags(&tags))
}

fn parse_tags_param(value: Option<&str>) -> Vec<String> {
    let Some(value) = value else {
        return Vec::new();
    };
    normalized_tags(
        &value
            .split(',')
            .map(str::to_string)
            .collect::<Vec<String>>(),
    )
}

fn normalized_tags(tags: &[String]) -> Vec<String> {
    let mut normalized = Vec::new();
    for tag in tags {
        let tag = tag.trim();
        if !tag.is_empty() && !normalized.iter().any(|item| item == tag) {
            normalized.push(tag.to_string());
        }
    }
    normalized
}
