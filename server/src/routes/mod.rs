mod auth;
mod image_processing;
mod paths;
mod sprites;
mod state;

use std::{
    collections::{HashMap, HashSet},
    sync::Arc,
};

use axum::{
    body::Bytes,
    extract::{rejection::JsonRejection, Multipart, Path, Query, State},
    http::{header, HeaderMap, StatusCode},
    response::IntoResponse,
    routing::{get, post, put},
    Json, Router,
};
use chrono::{Datelike, Duration, Local, Utc};
use sha2::{Digest, Sha256};
use tower_http::services::ServeDir;

use crate::{
    error::AppError,
    models::{
        null_data, ApiResponse, ImagePayload, LoginRequest, LoginResponse, Plan, SpriteMeta,
        SpritePayload,
    },
};
use protocol::{ImageStatus, LocalDate};
use tokio::sync::{mpsc, Mutex};

use auth::{require_admin, require_any_permission};
use image_processing::{detect_uploaded_image_format, render_display_bmp};
use paths::{
    display_image_path, display_image_temp_path, find_original_image_path, original_image_path,
    remove_file_if_exists, remove_image_files, sprite_cache_path,
};
use sprites::{
    ensure_sprite_cached, load_sprite_font_config, sprite_sha256, validate_sprite_font_config,
    SpriteKind,
};
pub use state::{AdminSession, AppState};
use state::{Permission, ProcessingQueue, RuntimeState};

pub fn router(state: AppState) -> Router {
    let runtime = RuntimeState {
        queue: state
            .enqueue_processing
            .then(|| start_processing_worker(state.clone())),
        app: state,
    };
    spawn_pending_enqueue(&runtime);

    Router::new()
        .route("/api/healthz", get(healthz))
        .route("/api/login", post(login))
        .route("/api/plans", get(list_plans).post(create_plan))
        .route("/api/plans/:date", put(update_plan).delete(delete_plan))
        .route("/api/images", get(list_images).post(upload_image))
        .route(
            "/api/images/:sha256",
            put(update_image).delete(delete_image),
        )
        .route("/api/images/:sha256/redither", post(redither_image))
        .route("/api/sprites", get(sprite_metadata))
        .route("/images/:sha256", get(download_image))
        .route("/sprites/:sha256", get(download_sprite))
        .fallback_service(ServeDir::new("web/dist").append_index_html_on_directories(true))
        .with_state(runtime)
}

pub async fn recover_and_enqueue_pending(state: &AppState) -> anyhow::Result<()> {
    state.store.recover_processing_images().await?;

    let ready_sha256s = state.store.ready_sha256s().await?;
    let missing = ready_sha256s
        .into_iter()
        .filter(|sha256| !display_image_path(&state.data_dir, sha256).exists())
        .collect::<Vec<_>>();
    state
        .store
        .mark_ready_missing_display_pending(&missing)
        .await?;

    Ok(())
}

fn spawn_pending_enqueue(runtime: &RuntimeState) {
    let Some(queue) = runtime.queue.clone() else {
        return;
    };
    let store = runtime.app.store.clone();
    tokio::spawn(async move {
        match store.pending_sha256s().await {
            Ok(sha256s) => {
                for sha256 in sha256s {
                    let mut queued = queue.queued.lock().await;
                    if queued.insert(sha256.clone()) && queue.sender.send(sha256.clone()).is_err() {
                        queued.remove(&sha256);
                    }
                }
            }
            Err(error) => tracing::error!("failed to enqueue pending images: {error:?}"),
        }
    });
}

async fn healthz() -> &'static str {
    "ok"
}

async fn login(
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

async fn list_plans(
    State(state): State<RuntimeState>,
    headers: HeaderMap,
    Query(params): Query<HashMap<String, String>>,
) -> Result<impl IntoResponse, AppError> {
    let permission = require_any_permission(&headers, &state.app).await?;
    let (start, end) = parse_plan_range(&params)?;

    match permission {
        Permission::Admin => Ok(Json(ApiResponse::ok(
            serde_json::to_value(state.app.store.list_admin_plans(start, end).await?)
                .map_err(|error| AppError::Internal(error.into()))?,
        ))),
        Permission::User => Ok(Json(ApiResponse::ok(
            serde_json::to_value(state.app.store.list_user_plans(start, end).await?)
                .map_err(|error| AppError::Internal(error.into()))?,
        ))),
    }
}

async fn create_plan(
    State(state): State<RuntimeState>,
    headers: HeaderMap,
    payload: Result<Json<Plan>, JsonRejection>,
) -> Result<impl IntoResponse, AppError> {
    require_admin(&headers, &state.app).await?;
    let Json(payload) = payload.map_err(map_json_rejection)?;
    validate_plan_payload(&payload)?;
    let plan = state
        .app
        .store
        .create_plan(payload)
        .await
        .map_err(map_store_error)?;
    Ok((StatusCode::CREATED, Json(ApiResponse::ok(plan))))
}

async fn update_plan(
    State(state): State<RuntimeState>,
    headers: HeaderMap,
    Path(date): Path<String>,
    payload: Result<Json<Plan>, JsonRejection>,
) -> Result<impl IntoResponse, AppError> {
    require_admin(&headers, &state.app).await?;
    let date = parse_plan_date(&date)?;
    let Json(payload) = payload.map_err(map_json_rejection)?;
    validate_plan_payload(&payload)?;
    let plan = state
        .app
        .store
        .update_plan(date, payload)
        .await
        .map_err(map_store_error)?
        .ok_or_else(|| AppError::NotFound("计划不存在".to_string()))?;
    Ok(Json(ApiResponse::ok(plan)))
}

async fn delete_plan(
    State(state): State<RuntimeState>,
    headers: HeaderMap,
    Path(date): Path<String>,
) -> Result<impl IntoResponse, AppError> {
    require_admin(&headers, &state.app).await?;
    let date = parse_plan_date(&date)?;
    if state.app.store.delete_plan(date).await? {
        Ok(Json(ApiResponse::ok(null_data())))
    } else {
        Err(AppError::NotFound("计划不存在".to_string()))
    }
}

async fn list_images(
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

async fn upload_image(
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

async fn update_image(
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

async fn delete_image(
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

async fn redither_image(
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

async fn download_image(
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

async fn sprite_metadata(
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

async fn download_sprite(
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

async fn enqueue_image(state: &RuntimeState, sha256: String) {
    let Some(queue) = &state.queue else {
        return;
    };

    let mut queued = queue.queued.lock().await;
    if queued.insert(sha256.clone()) && queue.sender.send(sha256.clone()).is_err() {
        queued.remove(&sha256);
    }
}

fn start_processing_worker(state: AppState) -> ProcessingQueue {
    let (sender, mut receiver) = mpsc::unbounded_channel::<String>();
    let queued = Arc::new(Mutex::new(HashSet::new()));
    let worker_queued = queued.clone();

    tokio::spawn(async move {
        while let Some(sha256) = receiver.recv().await {
            {
                let mut queued = worker_queued.lock().await;
                queued.remove(&sha256);
            }

            if let Err(error) = process_one_image(&state, &sha256).await {
                tracing::error!("image processing failed for {sha256}: {error:?}");
                if let Err(mark_error) = state.store.mark_failed(&sha256).await {
                    tracing::error!("failed to mark image failed for {sha256}: {mark_error:?}");
                }
            }
        }
    });

    ProcessingQueue { sender, queued }
}

async fn process_one_image(state: &AppState, sha256: &str) -> anyhow::Result<()> {
    if !state.store.claim_pending(sha256).await? {
        return Ok(());
    }

    let result = async {
        let original_path = find_original_image_path(&state.data_dir, sha256)?;
        let display_path = display_image_path(&state.data_dir, sha256);
        let temp_path = display_image_temp_path(&state.data_dir, sha256);
        let bytes = tokio::fs::read(original_path).await?;
        let bmp = tokio::task::spawn_blocking(move || render_display_bmp(&bytes)).await??;
        tokio::fs::write(&temp_path, bmp).await?;
        if display_path.exists() {
            tokio::fs::remove_file(&display_path).await?;
        }
        tokio::fs::rename(&temp_path, &display_path).await?;
        anyhow::Ok(())
    }
    .await;

    match result {
        Ok(()) => state.store.mark_ready(sha256).await?,
        Err(error) => {
            state.store.mark_failed(sha256).await?;
            return Err(error);
        }
    }

    Ok(())
}

fn parse_days(value: Option<&String>) -> u32 {
    value
        .and_then(|value| value.parse::<u32>().ok())
        .unwrap_or(3)
        .clamp(1, 7)
}

fn parse_plan_range(params: &HashMap<String, String>) -> Result<(LocalDate, LocalDate), AppError> {
    if let (Some(start), Some(end)) = (params.get("start"), params.get("end")) {
        let start = parse_plan_date(start)?;
        let end = parse_plan_date(end)?;
        if start > end {
            return Err(AppError::BadRequest(
                "计划结束日期不能早于开始日期".to_string(),
            ));
        }
        return Ok((start, end));
    }

    let days = parse_days(params.get("days"));
    let start = Local::now().date_naive();
    let end = start + Duration::days((days - 1) as i64);
    Ok((local_date_from_chrono(start)?, local_date_from_chrono(end)?))
}

fn validate_plan_payload(payload: &Plan) -> Result<(), AppError> {
    if payload.caption.trim().is_empty() {
        return Err(AppError::BadRequest("计划标题不能为空".to_string()));
    }
    match payload.plan_type {
        protocol::PlanType::Fixed => {
            if !payload.image.is_empty() {
                validate_sha256(&payload.image)?;
            }
        }
        protocol::PlanType::Random => {
            if normalized_tags(&payload.tags).is_empty() {
                return Err(AppError::BadRequest("随机计划标签不能为空".to_string()));
            }
        }
    }
    Ok(())
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

fn parse_plan_date(value: &str) -> Result<LocalDate, AppError> {
    if value.trim().is_empty() {
        return Err(AppError::BadRequest("计划日期不能为空".to_string()));
    }
    LocalDate::parse(value)
        .map_err(|_| AppError::BadRequest("计划日期格式应为 YYYY-MM-DD".to_string()))
}

fn local_date_from_chrono(date: chrono::NaiveDate) -> Result<LocalDate, AppError> {
    LocalDate::new(date.year() as u16, date.month() as u8, date.day() as u8).ok_or_else(|| {
        AppError::Internal(anyhow::anyhow!(
            "failed to convert chrono date to local date"
        ))
    })
}

fn validate_sha256(value: &str) -> Result<(), AppError> {
    let valid = value.len() == 64 && value.bytes().all(|byte| byte.is_ascii_hexdigit());
    if valid {
        Ok(())
    } else {
        Err(AppError::BadRequest("图片标识格式不正确".to_string()))
    }
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

fn map_store_error(error: anyhow::Error) -> AppError {
    let message = error.to_string();
    if message.starts_with("Unknown image sha256:") {
        AppError::BadRequest("请选择已有图片".to_string())
    } else if message.starts_with("Plan date already exists:")
        || message.contains("UNIQUE constraint failed: plans.date")
    {
        AppError::BadRequest("该日期已有计划".to_string())
    } else {
        AppError::Internal(error)
    }
}

fn map_json_rejection(_error: JsonRejection) -> AppError {
    AppError::BadRequest("请求内容格式不正确".to_string())
}
