use axum::{
    body::Bytes,
    extract::{Multipart, Path, State},
    http::{header, HeaderMap, StatusCode},
    response::IntoResponse,
    routing::{delete, get},
    Json, Router,
};
use sha2::{Digest, Sha256};
use tower_http::services::ServeDir;

use crate::{
    db::{NewImage, NewPlanEntry, Store},
    error::AppError,
    models::{ImageSummaryResponse, PlanResponse, UploadImageResponse},
};

#[derive(Debug, Clone)]
pub struct AppState {
    pub store: Store,
    pub secret_key: String,
}

pub fn router(state: AppState) -> Router {
    Router::new()
        .route("/api/healthz", get(healthz))
        .route("/api/manifest", get(get_manifest).put(update_manifest))
        .route("/api/images", get(list_images).post(upload_image))
        .route("/api/images/:sha256", delete(delete_image))
        .route("/images/:sha256", get(download_image))
        .fallback_service(ServeDir::new("web/dist").append_index_html_on_directories(true))
        .with_state(state)
}

async fn healthz() -> &'static str {
    "ok"
}

async fn get_manifest(
    State(state): State<AppState>,
    headers: HeaderMap,
) -> Result<impl IntoResponse, AppError> {
    require_secret_key(&headers, &state.secret_key)?;
    let manifest = state
        .store
        .load_plan_response()
        .await?
        .unwrap_or_else(|| PlanResponse {
            version: "0".to_string(),
            plans: Vec::new(),
        });

    Ok(Json(manifest))
}

async fn update_manifest(
    State(state): State<AppState>,
    headers: HeaderMap,
    Json(manifest): Json<PlanResponse>,
) -> Result<impl IntoResponse, AppError> {
    require_secret_key(&headers, &state.secret_key)?;
    validate_manifest(&manifest)?;

    let entries = manifest
        .plans
        .iter()
        .map(|plan| NewPlanEntry {
            start: plan.start.clone(),
            end: plan.end.clone(),
            caption: plan.caption.clone(),
            images: plan.images.clone(),
        })
        .collect::<Vec<_>>();
    state
        .store
        .replace_plan_entries(&manifest.version, &entries)
        .await?;

    Ok(Json(manifest))
}

async fn list_images(
    State(state): State<AppState>,
    headers: HeaderMap,
) -> Result<impl IntoResponse, AppError> {
    require_secret_key(&headers, &state.secret_key)?;
    let images = state
        .store
        .list_images()
        .await?
        .into_iter()
        .map(ImageSummaryResponse::from)
        .collect::<Vec<_>>();

    Ok(Json(images))
}

async fn upload_image(
    State(state): State<AppState>,
    headers: HeaderMap,
    mut multipart: Multipart,
) -> Result<impl IntoResponse, AppError> {
    require_secret_key(&headers, &state.secret_key)?;

    let mut image: Option<(String, Bytes)> = None;
    while let Some(field) = multipart
        .next_field()
        .await
        .map_err(|error| AppError::BadRequest(format!("Invalid multipart body: {error}")))?
    {
        if field.name() != Some("image") {
            continue;
        }
        let content_type = field
            .content_type()
            .map(ToOwned::to_owned)
            .unwrap_or_else(|| "application/octet-stream".to_string());
        let bytes = field
            .bytes()
            .await
            .map_err(|error| AppError::BadRequest(format!("Invalid image field: {error}")))?;
        image = Some((content_type, bytes));
        break;
    }

    let Some((content_type, bytes)) = image else {
        return Err(AppError::BadRequest(
            "Missing multipart field: image".to_string(),
        ));
    };
    if bytes.is_empty() {
        return Err(AppError::BadRequest("Image file is empty".to_string()));
    }

    let sha256 = to_hex(&Sha256::digest(&bytes));
    state
        .store
        .upsert_image(NewImage {
            sha256: &sha256,
            content_type: &content_type,
            bytes: &bytes,
        })
        .await?;

    Ok((
        StatusCode::CREATED,
        Json(UploadImageResponse {
            url: format!("/images/{sha256}"),
            sha256,
        }),
    ))
}

async fn delete_image(
    State(state): State<AppState>,
    headers: HeaderMap,
    Path(sha256): Path<String>,
) -> Result<impl IntoResponse, AppError> {
    require_secret_key(&headers, &state.secret_key)?;
    validate_sha256(&sha256)?;

    if state.store.delete_image(&sha256).await? {
        Ok(StatusCode::NO_CONTENT)
    } else {
        Err(AppError::NotFound(format!("Image {sha256} not found")))
    }
}

async fn download_image(
    State(state): State<AppState>,
    headers: HeaderMap,
    Path(sha256): Path<String>,
) -> Result<impl IntoResponse, AppError> {
    require_secret_key(&headers, &state.secret_key)?;
    validate_sha256(&sha256)?;

    let image = state
        .store
        .get_image(&sha256)
        .await?
        .ok_or_else(|| AppError::NotFound(format!("Image {sha256} not found")))?;

    Ok(([(header::CONTENT_TYPE, image.content_type)], image.bytes))
}

fn require_secret_key(headers: &HeaderMap, secret_key: &str) -> Result<(), AppError> {
    let Some(value) = headers
        .get("secret-key")
        .and_then(|value| value.to_str().ok())
    else {
        return Err(AppError::Unauthorized);
    };
    if value != secret_key {
        return Err(AppError::Unauthorized);
    }
    Ok(())
}

fn validate_manifest(manifest: &PlanResponse) -> Result<(), AppError> {
    if manifest.version.trim().is_empty() {
        return Err(AppError::BadRequest("version cannot be empty".to_string()));
    }

    for plan in &manifest.plans {
        if plan.start.trim().is_empty() || plan.end.trim().is_empty() {
            return Err(AppError::BadRequest(
                "plan start and end cannot be empty".to_string(),
            ));
        }
        if plan.images.is_empty() {
            return Err(AppError::BadRequest(
                "plan images cannot be empty".to_string(),
            ));
        }
        for sha256 in &plan.images {
            validate_sha256(sha256)?;
        }
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

fn to_hex(bytes: &[u8]) -> String {
    const HEX: &[u8; 16] = b"0123456789abcdef";
    let mut output = String::with_capacity(bytes.len() * 2);
    for byte in bytes {
        output.push(HEX[(byte >> 4) as usize] as char);
        output.push(HEX[(byte & 0x0f) as usize] as char);
    }
    output
}
