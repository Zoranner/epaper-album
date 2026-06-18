use std::{
    collections::{HashMap, HashSet},
    io::Cursor,
    path::PathBuf,
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
use chrono::{DateTime, Datelike, Duration, Local, Utc};
use fontdue::{
    layout::{CoordinateSystem, Layout, LayoutSettings, TextStyle},
    Font, FontSettings,
};
use image::{
    codecs::bmp::BmpEncoder, imageops::FilterType, DynamicImage, ExtendedColorType, GenericImage,
    GenericImageView, ImageEncoder, Rgba, RgbaImage,
};
use serde::Deserialize;
use sha2::{Digest, Sha256};
use tokio::sync::{mpsc, Mutex};
use tower_http::services::ServeDir;

use crate::{
    db::Store,
    error::AppError,
    models::{
        null_data, ApiResponse, ImageRemarkPayload, LoginRequest, LoginResponse, Plan, SpriteMeta,
        SpritePayload,
    },
};
use protocol::LocalDate;

const DISPLAY_WIDTH: u32 = 800;
const DISPLAY_HEIGHT: u32 = 480;
const SPRITE_FONT_DIR: &str = "assets/fonts";
const SPRITE_FONT_CONFIG_PATH: &str = "assets/fonts.toml";

#[derive(Debug, Clone)]
pub struct AppState {
    pub store: Store,
    pub secret_key: String,
    pub admin_username: String,
    pub admin_password: String,
    pub admin_session: Arc<Mutex<AdminSession>>,
    pub data_dir: PathBuf,
    pub enqueue_processing: bool,
}

#[derive(Debug, Clone)]
pub struct AdminSession {
    pub token: String,
    pub expires_at: DateTime<Utc>,
}

impl AdminSession {
    pub fn new(token: String, expires_at: DateTime<Utc>) -> Self {
        Self { token, expires_at }
    }
}

#[derive(Debug, Clone, Copy)]
enum SpriteKind {
    Caption,
    Date,
    Status,
}

#[derive(Debug, Clone, Copy, Deserialize)]
struct SpriteStyle {
    font_size: f32,
    padding_x: u32,
    padding_y: u32,
    background: SpriteColor,
    color: SpriteColor,
    border_color: SpriteColor,
    border_width: u32,
}

#[derive(Debug)]
struct SpriteFontAsset {
    path: PathBuf,
}

#[derive(Debug, Deserialize)]
struct SpriteFontConfig {
    files: Vec<String>,
    style: SpriteStyle,
}

#[derive(Debug)]
struct LoadedSpriteFontConfig {
    raw: String,
    parsed: SpriteFontConfig,
}

struct SpriteCanvas<'a> {
    image: &'a mut RgbaImage,
    width: u32,
    height: u32,
    style: SpriteStyle,
}

#[derive(Debug, Clone, Copy, Deserialize)]
#[serde(rename_all = "lowercase")]
enum SpriteColor {
    Black,
    White,
    Red,
    Yellow,
    Blue,
    Green,
}

impl SpriteColor {
    const fn rgba(self) -> Rgba<u8> {
        match self {
            Self::Black => Rgba([0, 0, 0, 255]),
            Self::White => Rgba([255, 255, 255, 255]),
            Self::Red => Rgba([255, 0, 0, 255]),
            Self::Yellow => Rgba([255, 255, 0, 255]),
            Self::Blue => Rgba([0, 0, 255, 255]),
            Self::Green => Rgba([0, 255, 0, 255]),
        }
    }
}

#[derive(Debug, Clone, Copy)]
enum UploadedImageFormat {
    Bmp,
    Jpeg,
    Png,
}

impl UploadedImageFormat {
    const fn extension(self) -> &'static str {
        match self {
            Self::Bmp => "bmp",
            Self::Jpeg => "jpg",
            Self::Png => "png",
        }
    }
}

impl SpriteKind {
    const fn as_str(self) -> &'static str {
        match self {
            Self::Caption => "caption",
            Self::Date => "date",
            Self::Status => "status",
        }
    }
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
    let days = parse_days(params.get("days"));
    let start = Local::now().date_naive();
    let end = start + Duration::days((days - 1) as i64);
    let start = local_date_from_chrono(start)?;
    let end = local_date_from_chrono(end)?;

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
        .list_images(params.get("keyword").map(String::as_str))
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
            _ => {}
        }
    }

    let bytes = image.ok_or_else(|| AppError::BadRequest("请选择要上传的图片".to_string()))?;
    if bytes.is_empty() {
        return Err(AppError::BadRequest("图片文件不能为空".to_string()));
    }

    let format = detect_uploaded_image_format(&bytes)?;
    let sha256 = hex::encode(Sha256::digest(&bytes));
    let original_path = original_image_path(&state.app.data_dir, &sha256, format);
    if !original_path.exists() {
        tokio::fs::write(&original_path, &bytes)
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
    require_admin(&headers, &state.app).await?;
    validate_sha256(&sha256)?;
    let image = state
        .app
        .store
        .update_image_remark(&sha256, &payload.remark)
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
    if image.status != "ready" {
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

async fn ensure_sprite_cached(
    state: &RuntimeState,
    sha256: &str,
    text: String,
    font_config: LoadedSpriteFontConfig,
) -> Result<(), AppError> {
    let cache_path = sprite_cache_path(&state.app.data_dir, sha256);

    if cache_path.exists() {
        return Ok(());
    }

    let font_assets = load_sprite_font_assets(&font_config.parsed).await?;
    let mut font_bytes = Vec::with_capacity(font_assets.len());
    for asset in font_assets {
        font_bytes.push(
            tokio::fs::read(asset.path)
                .await
                .map_err(|error| AppError::Internal(error.into()))?,
        );
    }
    let style = font_config.parsed.style;
    let bmp = tokio::task::spawn_blocking(move || render_sprite_bmp(&text, font_bytes, style))
        .await
        .map_err(|error| AppError::Internal(error.into()))??;

    write_sprite_cache(&cache_path, &bmp).await?;
    Ok(())
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

fn render_display_bmp(bytes: &[u8]) -> anyhow::Result<Vec<u8>> {
    let image = image::load_from_memory(bytes)?;
    let fitted = fit_to_display(image);
    let paletted = quantize_six_color(fitted);
    encode_rgb_bmp(&paletted.to_rgb8())
}

fn render_sprite_bmp(
    text: &str,
    font_bytes: Vec<Vec<u8>>,
    style: SpriteStyle,
) -> anyhow::Result<Vec<u8>> {
    let fonts = font_bytes
        .into_iter()
        .map(|bytes| {
            Font::from_bytes(bytes, FontSettings::default())
                .map_err(|error| anyhow::anyhow!("{error}"))
        })
        .collect::<anyhow::Result<Vec<_>>>()?;
    let mut layout = Layout::new(CoordinateSystem::PositiveYDown);
    layout.reset(&LayoutSettings {
        x: style.padding_x as f32,
        y: style.padding_y as f32,
        ..LayoutSettings::default()
    });
    for character in text.chars() {
        let font_index = fallback_font_index(&fonts, character);
        layout.append(
            &fonts,
            &TextStyle::new(&character.to_string(), style.font_size, font_index),
        );
    }

    let glyphs = layout.glyphs();
    let text_width = glyphs
        .iter()
        .map(|glyph| (glyph.x + glyph.width as f32).ceil() as i32)
        .max()
        .unwrap_or(style.padding_x as i32);
    let text_height = glyphs
        .iter()
        .map(|glyph| (glyph.y + glyph.height as f32).ceil() as i32)
        .max()
        .unwrap_or(style.padding_y as i32);
    let width = (text_width.max(style.padding_x as i32) as u32 + style.padding_x).max(1);
    let height = (text_height.max(style.padding_y as i32) as u32 + style.padding_y).max(1);
    let mut image = RgbaImage::from_pixel(width, height, style.background.rgba());

    for glyph in glyphs {
        let (metrics, bitmap) = fonts[glyph.font_index].rasterize_config(glyph.key);
        let left = glyph.x.round() as i32;
        let top = glyph.y.round() as i32;

        let mut canvas = SpriteCanvas {
            image: &mut image,
            width,
            height,
            style,
        };
        draw_glyph_stroke(&mut canvas, left, top, &metrics, &bitmap);
        draw_glyph_fill(&mut canvas, left, top, &metrics, &bitmap);
    }

    encode_rgb_bmp(&DynamicImage::ImageRgba8(image).to_rgb8())
}

fn draw_glyph_stroke(
    canvas: &mut SpriteCanvas<'_>,
    left: i32,
    top: i32,
    metrics: &fontdue::Metrics,
    bitmap: &[u8],
) {
    let stroke_width = canvas.style.border_width.min(4) as i32;
    if stroke_width == 0 {
        return;
    }

    for y in 0..metrics.height {
        for x in 0..metrics.width {
            let coverage = bitmap[y * metrics.width + x];
            if coverage < 32 {
                continue;
            }

            for offset_y in -stroke_width..=stroke_width {
                for offset_x in -stroke_width..=stroke_width {
                    let target_x = left + x as i32 + offset_x;
                    let target_y = top + y as i32 + offset_y;
                    put_sprite_pixel(
                        canvas.image,
                        canvas.width,
                        canvas.height,
                        target_x,
                        target_y,
                        canvas.style.border_color.rgba(),
                    );
                }
            }
        }
    }
}

fn draw_glyph_fill(
    canvas: &mut SpriteCanvas<'_>,
    left: i32,
    top: i32,
    metrics: &fontdue::Metrics,
    bitmap: &[u8],
) {
    for y in 0..metrics.height {
        for x in 0..metrics.width {
            let coverage = bitmap[y * metrics.width + x];
            if coverage < 96 {
                continue;
            }

            let target_x = left + x as i32;
            let target_y = top + y as i32;
            put_sprite_pixel(
                canvas.image,
                canvas.width,
                canvas.height,
                target_x,
                target_y,
                canvas.style.color.rgba(),
            );
        }
    }
}

fn put_sprite_pixel(
    image: &mut RgbaImage,
    width: u32,
    height: u32,
    target_x: i32,
    target_y: i32,
    color: Rgba<u8>,
) {
    if target_x < 0 || target_y < 0 || target_x >= width as i32 || target_y >= height as i32 {
        return;
    }

    image.put_pixel(target_x as u32, target_y as u32, color);
}

fn encode_rgb_bmp(image: &image::RgbImage) -> anyhow::Result<Vec<u8>> {
    let mut output = Cursor::new(Vec::new());
    let encoder = BmpEncoder::new(&mut output);
    encoder.write_image(
        image.as_raw(),
        image.width(),
        image.height(),
        ExtendedColorType::Rgb8,
    )?;
    Ok(output.into_inner())
}

async fn load_sprite_font_assets(
    config: &SpriteFontConfig,
) -> Result<Vec<SpriteFontAsset>, AppError> {
    let mut assets = Vec::new();
    for file_name in &config.files {
        let file_name = file_name.trim();
        if file_name.is_empty() {
            continue;
        }
        let path = PathBuf::from(SPRITE_FONT_DIR).join(file_name);
        tokio::fs::metadata(&path)
            .await
            .map_err(|error| AppError::Internal(error.into()))?;
        assets.push(SpriteFontAsset { path });
    }
    if assets.is_empty() {
        return Err(AppError::Internal(anyhow::anyhow!(
            "sprite font config has no font files"
        )));
    }
    Ok(assets)
}

async fn load_sprite_font_config() -> Result<LoadedSpriteFontConfig, AppError> {
    let config = tokio::fs::read_to_string(SPRITE_FONT_CONFIG_PATH)
        .await
        .map_err(|error| AppError::Internal(error.into()))?;
    let parsed = toml::from_str(&config).map_err(|error| AppError::Internal(error.into()))?;
    Ok(LoadedSpriteFontConfig {
        raw: config,
        parsed,
    })
}

fn validate_sprite_font_config(config: &SpriteFontConfig) -> Result<(), AppError> {
    if config
        .files
        .iter()
        .all(|file_name| file_name.trim().is_empty())
    {
        return Err(AppError::Internal(anyhow::anyhow!(
            "sprite font config has no font files"
        )));
    }
    if config.style.font_size <= 0.0 {
        return Err(AppError::Internal(anyhow::anyhow!(
            "sprite font size must be positive"
        )));
    }
    Ok(())
}

fn fallback_font_index(fonts: &[Font], character: char) -> usize {
    fonts
        .iter()
        .position(|font| font.has_glyph(character))
        .unwrap_or(0)
}

async fn write_sprite_cache(path: &std::path::Path, bytes: &[u8]) -> Result<(), AppError> {
    let directory = path
        .parent()
        .ok_or_else(|| AppError::Internal(anyhow::anyhow!("invalid sprite cache path")))?;
    tokio::fs::create_dir_all(directory)
        .await
        .map_err(|error| AppError::Internal(error.into()))?;
    let temp_path = path.with_extension("tmp");
    tokio::fs::write(&temp_path, bytes)
        .await
        .map_err(|error| AppError::Internal(error.into()))?;
    if path.exists() {
        tokio::fs::remove_file(path)
            .await
            .map_err(|error| AppError::Internal(error.into()))?;
    }
    tokio::fs::rename(temp_path, path)
        .await
        .map_err(|error| AppError::Internal(error.into()))?;
    Ok(())
}

fn sprite_sha256(kind: SpriteKind, text: &str, font_config: &str) -> String {
    let mut hasher = Sha256::new();
    hasher.update(kind.as_str().as_bytes());
    hasher.update(text.as_bytes());
    hasher.update(font_config.as_bytes());
    hex::encode(hasher.finalize())
}

fn detect_uploaded_image_format(bytes: &[u8]) -> Result<UploadedImageFormat, AppError> {
    if bytes.starts_with(b"BM") {
        return Ok(UploadedImageFormat::Bmp);
    }
    if bytes.starts_with(&[0xff, 0xd8, 0xff]) {
        return Ok(UploadedImageFormat::Jpeg);
    }
    if bytes.starts_with(b"\x89PNG\r\n\x1a\n") {
        return Ok(UploadedImageFormat::Png);
    }
    Err(AppError::BadRequest(
        "图片格式支持 JPG、PNG 和 BMP".to_string(),
    ))
}

async fn remove_image_files(data_dir: &std::path::Path, sha256: &str) -> Result<(), AppError> {
    remove_file_if_exists(display_image_path(data_dir, sha256)).await?;
    for extension in ["jpg", "png", "bmp", "jpeg"] {
        remove_file_if_exists(original_image_dir(data_dir).join(format!("{sha256}.{extension}")))
            .await?;
    }
    Ok(())
}

async fn remove_file_if_exists(path: PathBuf) -> Result<(), AppError> {
    match tokio::fs::remove_file(path).await {
        Ok(()) => Ok(()),
        Err(error) if error.kind() == std::io::ErrorKind::NotFound => Ok(()),
        Err(error) => Err(AppError::Internal(error.into())),
    }
}

fn original_image_path(
    data_dir: &std::path::Path,
    sha256: &str,
    format: UploadedImageFormat,
) -> PathBuf {
    original_image_dir(data_dir).join(format!("{sha256}.{}", format.extension()))
}

fn find_original_image_path(data_dir: &std::path::Path, sha256: &str) -> anyhow::Result<PathBuf> {
    let directory = original_image_dir(data_dir);
    for extension in ["jpg", "png", "bmp", "jpeg"] {
        let path = directory.join(format!("{sha256}.{extension}"));
        if path.exists() {
            return Ok(path);
        }
    }

    let legacy_path = directory.join(sha256);
    if legacy_path.exists() {
        return Ok(legacy_path);
    }

    Err(anyhow::anyhow!("original image file missing: {sha256}"))
}

fn original_image_dir(data_dir: &std::path::Path) -> PathBuf {
    data_dir.join("images").join("original")
}

fn display_image_path(data_dir: &std::path::Path, sha256: &str) -> PathBuf {
    data_dir
        .join("images")
        .join("display")
        .join(format!("{sha256}.bmp"))
}

fn display_image_temp_path(data_dir: &std::path::Path, sha256: &str) -> PathBuf {
    data_dir
        .join("images")
        .join("display")
        .join(format!("{sha256}.tmp"))
}

fn sprite_cache_path(data_dir: &std::path::Path, sha256: &str) -> PathBuf {
    data_dir.join("sprites").join(format!("{sha256}.bmp"))
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

async fn require_any_permission(
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

async fn require_admin(headers: &HeaderMap, state: &AppState) -> Result<(), AppError> {
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

fn parse_days(value: Option<&String>) -> u32 {
    value
        .and_then(|value| value.parse::<u32>().ok())
        .unwrap_or(3)
        .clamp(1, 7)
}

fn validate_plan_payload(payload: &Plan) -> Result<(), AppError> {
    if payload.caption.trim().is_empty() {
        return Err(AppError::BadRequest("计划标题不能为空".to_string()));
    }
    if !payload.image.is_empty() {
        validate_sha256(&payload.image)?;
    }
    Ok(())
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

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn render_display_bmp_outputs_device_compatible_bmp() {
        let mut input = image::RgbImage::new(32, 32);
        for (x, y, pixel) in input.enumerate_pixels_mut() {
            *pixel = image::Rgb([(x * 8) as u8, (y * 8) as u8, 128]);
        }

        let mut encoded_input = Cursor::new(Vec::new());
        DynamicImage::ImageRgb8(input)
            .write_to(&mut encoded_input, image::ImageFormat::Png)
            .expect("encode input png");

        let bmp = render_display_bmp(&encoded_input.into_inner()).expect("render display bmp");

        assert_eq!(&bmp[0..2], b"BM");
        assert_eq!(u32::from_le_bytes(bmp[14..18].try_into().unwrap()), 40);
        assert_eq!(i32::from_le_bytes(bmp[18..22].try_into().unwrap()), 800);
        assert_eq!(i32::from_le_bytes(bmp[22..26].try_into().unwrap()), 480);
        assert_eq!(u16::from_le_bytes(bmp[26..28].try_into().unwrap()), 1);
        assert_eq!(u16::from_le_bytes(bmp[28..30].try_into().unwrap()), 24);
        assert_eq!(u32::from_le_bytes(bmp[30..34].try_into().unwrap()), 0);
    }

    #[test]
    fn display_image_path_uses_bmp_extension() {
        let path = display_image_path(std::path::Path::new("data"), "abc");
        assert_eq!(
            path,
            std::path::Path::new("data")
                .join("images")
                .join("display")
                .join("abc.bmp")
        );
    }

    #[test]
    fn sprite_style_accepts_panel_color_names_only() {
        let config = toml::from_str::<SpriteFontConfig>(
            r#"
files = ["TerminessTTF NF.ttf"]

[style]
font_size = 32.0
padding_x = 12
padding_y = 8
background = "green"
color = "white"
border_color = "black"
border_width = 1
"#,
        )
        .expect("parse panel color style");

        assert_eq!(config.style.background.rgba(), Rgba([0, 255, 0, 255]));
        assert_eq!(config.style.color.rgba(), Rgba([255, 255, 255, 255]));
        assert_eq!(config.style.border_color.rgba(), Rgba([0, 0, 0, 255]));

        let invalid = toml::from_str::<SpriteFontConfig>(
            r##"
files = ["TerminessTTF NF.ttf"]

[style]
font_size = 32.0
padding_x = 12
padding_y = 8
background = "#155e75"
color = "white"
border_color = "black"
border_width = 1
"##,
        );

        assert!(invalid.is_err());
    }
}
