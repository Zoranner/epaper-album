use std::{
    collections::{HashMap, HashSet},
    io::Cursor,
    path::PathBuf,
    sync::Arc,
};

use axum::{
    body::Bytes,
    extract::{Multipart, Path, Query, State},
    http::{header, HeaderMap, StatusCode},
    response::IntoResponse,
    routing::{get, post, put},
    Json, Router,
};
use chrono::{Duration, Local, NaiveDate};
use image::{
    imageops::FilterType, DynamicImage, GenericImage, GenericImageView, ImageFormat, Rgba,
};
use sha2::{Digest, Sha256};
use tokio::sync::{mpsc, Mutex};
use tower_http::services::ServeDir;

use crate::{
    db::Store,
    error::AppError,
    models::{
        null_data, ApiResponse, ImageRemarkPayload, LoginRequest, LoginResponse, PlanPayload,
    },
};

const DISPLAY_WIDTH: u32 = 800;
const DISPLAY_HEIGHT: u32 = 480;

#[derive(Debug, Clone)]
pub struct AppState {
    pub store: Store,
    pub secret_key: String,
    pub admin_username: String,
    pub admin_password: String,
    pub admin_token: String,
    pub data_dir: PathBuf,
    pub enqueue_processing: bool,
}

#[derive(Debug, Clone)]
struct RuntimeState {
    app: AppState,
    queue: Option<ProcessingQueue>,
}

#[derive(Debug, Clone)]
struct ProcessingQueue {
    sender: mpsc::UnboundedSender<String>,
    queued: Arc<Mutex<HashSet<String>>>,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum Permission {
    User,
    Admin,
}

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
        .route("/api/plans/:id", put(update_plan).delete(delete_plan))
        .route("/api/images", get(list_images).post(upload_image))
        .route("/api/images/:sha256", put(update_image))
        .route("/images/:sha256", get(download_image))
        .fallback_service(ServeDir::new("web/dist").append_index_html_on_directories(true))
        .with_state(runtime)
}

pub async fn recover_and_enqueue_pending(state: &AppState) -> anyhow::Result<()> {
    state.store.recover_processing_images().await?;

    let ready_sha256s = state.store.ready_sha256s().await?;
    let missing = ready_sha256s
        .into_iter()
        .filter(|sha256| !state.data_dir.join("display").join(sha256).exists())
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
        Ok(Json(ApiResponse::ok(LoginResponse {
            token: state.app.admin_token,
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
    let permission = require_any_permission(&headers, &state.app)?;
    let days = parse_days(params.get("days"));
    let start = Local::now().date_naive();
    let end = start + Duration::days((days - 1) as i64);
    let start = start.format("%Y-%m-%d").to_string();
    let end = end.format("%Y-%m-%d").to_string();

    match permission {
        Permission::Admin => Ok(Json(ApiResponse::ok(
            serde_json::to_value(state.app.store.list_admin_plans(&start, &end).await?)
                .map_err(|error| AppError::Internal(error.into()))?,
        ))),
        Permission::User => Ok(Json(ApiResponse::ok(
            serde_json::to_value(state.app.store.list_user_plans(&start, &end).await?)
                .map_err(|error| AppError::Internal(error.into()))?,
        ))),
    }
}

async fn create_plan(
    State(state): State<RuntimeState>,
    headers: HeaderMap,
    Json(payload): Json<PlanPayload>,
) -> Result<impl IntoResponse, AppError> {
    require_admin(&headers, &state.app)?;
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
    Path(id): Path<i64>,
    Json(payload): Json<PlanPayload>,
) -> Result<impl IntoResponse, AppError> {
    require_admin(&headers, &state.app)?;
    validate_plan_payload(&payload)?;
    let plan = state
        .app
        .store
        .update_plan(id, payload)
        .await
        .map_err(map_store_error)?
        .ok_or_else(|| AppError::NotFound(format!("Plan {id} not found")))?;
    Ok(Json(ApiResponse::ok(plan)))
}

async fn delete_plan(
    State(state): State<RuntimeState>,
    headers: HeaderMap,
    Path(id): Path<i64>,
) -> Result<impl IntoResponse, AppError> {
    require_admin(&headers, &state.app)?;
    if state.app.store.delete_plan(id).await? {
        Ok(Json(ApiResponse::ok(null_data())))
    } else {
        Err(AppError::NotFound(format!("Plan {id} not found")))
    }
}

async fn list_images(
    State(state): State<RuntimeState>,
    headers: HeaderMap,
    Query(params): Query<HashMap<String, String>>,
) -> Result<impl IntoResponse, AppError> {
    require_admin(&headers, &state.app)?;
    let images = state
        .app
        .store
        .list_images(params.get("keyword").map(String::as_str))
        .await?;
    Ok(Json(ApiResponse::ok(images)))
}

async fn upload_image(
    State(state): State<RuntimeState>,
    headers: HeaderMap,
    mut multipart: Multipart,
) -> Result<impl IntoResponse, AppError> {
    require_admin(&headers, &state.app)?;

    let mut image: Option<Bytes> = None;
    let mut remark: Option<String> = None;

    while let Some(field) = multipart
        .next_field()
        .await
        .map_err(|error| AppError::BadRequest(format!("Invalid multipart body: {error}")))?
    {
        match field.name() {
            Some("image") => {
                let bytes = field.bytes().await.map_err(|error| {
                    AppError::BadRequest(format!("Invalid image field: {error}"))
                })?;
                image = Some(bytes);
            }
            Some("remark") => {
                let value = field.text().await.map_err(|error| {
                    AppError::BadRequest(format!("Invalid remark field: {error}"))
                })?;
                remark = Some(value);
            }
            _ => {}
        }
    }

    let bytes =
        image.ok_or_else(|| AppError::BadRequest("Missing multipart field: image".to_string()))?;
    if bytes.is_empty() {
        return Err(AppError::BadRequest("Image file is empty".to_string()));
    }

    let sha256 = hex::encode(Sha256::digest(&bytes));
    let origin_path = state.app.data_dir.join("origin").join(&sha256);
    if !origin_path.exists() {
        tokio::fs::write(&origin_path, &bytes)
            .await
            .map_err(|error| AppError::Internal(error.into()))?;
    }

    let (image, should_enqueue) = state
        .app
        .store
        .upsert_uploaded_image(&sha256, remark.as_deref())
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
    Json(payload): Json<ImageRemarkPayload>,
) -> Result<impl IntoResponse, AppError> {
    require_admin(&headers, &state.app)?;
    validate_sha256(&sha256)?;
    let image = state
        .app
        .store
        .update_image_remark(&sha256, &payload.remark)
        .await?
        .ok_or_else(|| AppError::NotFound(format!("Image {sha256} not found")))?;
    Ok(Json(ApiResponse::ok(image)))
}

async fn download_image(
    State(state): State<RuntimeState>,
    headers: HeaderMap,
    Path(sha256): Path<String>,
) -> Result<impl IntoResponse, AppError> {
    require_any_permission(&headers, &state.app)?;
    validate_sha256(&sha256)?;

    let image = state
        .app
        .store
        .get_image(&sha256)
        .await?
        .ok_or_else(|| AppError::NotFound(format!("Image {sha256} not found")))?;
    if image.status != "ready" {
        return Err(AppError::NotFound(format!("Image {sha256} not found")));
    }

    let display_path = state.app.data_dir.join("display").join(&sha256);
    if !display_path.exists() {
        return Err(AppError::NotFound(format!("Image {sha256} not found")));
    }

    let bytes = tokio::fs::read(display_path)
        .await
        .map_err(|error| AppError::Internal(error.into()))?;
    Ok(([(header::CONTENT_TYPE, "image/bmp")], bytes))
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
        let origin_path = state.data_dir.join("origin").join(sha256);
        let display_path = state.data_dir.join("display").join(sha256);
        let temp_path = state.data_dir.join("display").join(format!("{sha256}.tmp"));
        let bytes = tokio::fs::read(origin_path).await?;
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

fn render_display_bmp(bytes: &[u8]) -> anyhow::Result<Vec<u8>> {
    let image = image::load_from_memory(bytes)?;
    let fitted = fit_to_display(image);
    let paletted = quantize_six_color(fitted);
    let mut output = Cursor::new(Vec::new());
    paletted.write_to(&mut output, ImageFormat::Bmp)?;
    Ok(output.into_inner())
}

fn fit_to_display(image: DynamicImage) -> DynamicImage {
    let (width, height) = image.dimensions();
    let scale = (DISPLAY_WIDTH as f32 / width as f32).max(DISPLAY_HEIGHT as f32 / height as f32);
    let resized_width = (width as f32 * scale).round() as u32;
    let resized_height = (height as f32 * scale).round() as u32;
    let resized = image.resize_exact(resized_width, resized_height, FilterType::Triangle);
    let left = (resized_width.saturating_sub(DISPLAY_WIDTH)) / 2;
    let top = (resized_height.saturating_sub(DISPLAY_HEIGHT)) / 2;
    resized.crop_imm(left, top, DISPLAY_WIDTH, DISPLAY_HEIGHT)
}

fn quantize_six_color(image: DynamicImage) -> DynamicImage {
    let palette = [
        Rgba([0, 0, 0, 255]),
        Rgba([255, 255, 255, 255]),
        Rgba([255, 0, 0, 255]),
        Rgba([255, 255, 0, 255]),
        Rgba([0, 0, 255, 255]),
        Rgba([0, 255, 0, 255]),
    ];
    let mut output = DynamicImage::new_rgba8(DISPLAY_WIDTH, DISPLAY_HEIGHT);
    let mut work = image.to_rgba8();

    for y in 0..DISPLAY_HEIGHT {
        for x in 0..DISPLAY_WIDTH {
            let pixel = *work.get_pixel(x, y);
            let nearest = palette
                .iter()
                .copied()
                .min_by_key(|candidate| color_distance(pixel, *candidate))
                .unwrap_or(palette[0]);
            output.put_pixel(x, y, nearest);
            diffuse_error(&mut work, x, y, pixel, nearest);
        }
    }

    output
}

fn diffuse_error(
    image: &mut image::RgbaImage,
    x: u32,
    y: u32,
    current: Rgba<u8>,
    quantized: Rgba<u8>,
) {
    let error = [
        current[0] as i16 - quantized[0] as i16,
        current[1] as i16 - quantized[1] as i16,
        current[2] as i16 - quantized[2] as i16,
    ];
    add_error(image, x as i32 + 1, y as i32, error, 7);
    add_error(image, x as i32 - 1, y as i32 + 1, error, 3);
    add_error(image, x as i32, y as i32 + 1, error, 5);
    add_error(image, x as i32 + 1, y as i32 + 1, error, 1);
}

fn add_error(image: &mut image::RgbaImage, x: i32, y: i32, error: [i16; 3], weight: i16) {
    if x < 0 || y < 0 || x >= DISPLAY_WIDTH as i32 || y >= DISPLAY_HEIGHT as i32 {
        return;
    }

    let pixel = image.get_pixel_mut(x as u32, y as u32);
    for channel in 0..3 {
        let value = pixel[channel] as i16 + error[channel] * weight / 16;
        pixel[channel] = value.clamp(0, 255) as u8;
    }
}

fn color_distance(left: Rgba<u8>, right: Rgba<u8>) -> u32 {
    left.0[..3]
        .iter()
        .zip(right.0[..3].iter())
        .map(|(left, right)| {
            let diff = *left as i32 - *right as i32;
            (diff * diff) as u32
        })
        .sum()
}

fn require_any_permission(headers: &HeaderMap, state: &AppState) -> Result<Permission, AppError> {
    if is_admin(headers, state) {
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

fn require_admin(headers: &HeaderMap, state: &AppState) -> Result<(), AppError> {
    if is_admin(headers, state) {
        Ok(())
    } else {
        Err(AppError::Unauthorized)
    }
}

fn is_admin(headers: &HeaderMap, state: &AppState) -> bool {
    headers
        .get(header::AUTHORIZATION)
        .and_then(|value| value.to_str().ok())
        .and_then(|value| value.strip_prefix("Bearer "))
        .is_some_and(|token| token == state.admin_token)
}

fn parse_days(value: Option<&String>) -> u32 {
    value
        .and_then(|value| value.parse::<u32>().ok())
        .unwrap_or(3)
        .clamp(1, 7)
}

fn validate_plan_payload(payload: &PlanPayload) -> Result<(), AppError> {
    if payload.start.trim().is_empty() || payload.end.trim().is_empty() {
        return Err(AppError::BadRequest(
            "plan start and end cannot be empty".to_string(),
        ));
    }
    let start = NaiveDate::parse_from_str(&payload.start, "%Y-%m-%d")
        .map_err(|_| AppError::BadRequest("plan start must use YYYY-MM-DD format".to_string()))?;
    let end = NaiveDate::parse_from_str(&payload.end, "%Y-%m-%d")
        .map_err(|_| AppError::BadRequest("plan end must use YYYY-MM-DD format".to_string()))?;
    if start > end {
        return Err(AppError::BadRequest(
            "plan start cannot be later than end".to_string(),
        ));
    }
    for sha256 in &payload.images {
        validate_sha256(sha256)?;
    }
    Ok(())
}

fn validate_sha256(value: &str) -> Result<(), AppError> {
    let valid = value.len() == 64 && value.bytes().all(|byte| byte.is_ascii_hexdigit());
    if valid {
        Ok(())
    } else {
        Err(AppError::BadRequest(format!("Invalid sha256: {value}")))
    }
}

fn map_store_error(error: anyhow::Error) -> AppError {
    let message = error.to_string();
    if message.starts_with("Unknown image sha256:") {
        AppError::BadRequest(message)
    } else {
        AppError::Internal(error)
    }
}
