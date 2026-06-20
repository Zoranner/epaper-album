use std::collections::HashMap;

use axum::{
    extract::{rejection::JsonRejection, Path, Query, State},
    http::{HeaderMap, StatusCode},
    response::IntoResponse,
    Json,
};
use chrono::{Datelike, Duration, Local};

use crate::{
    auth::{require_admin, require_any_permission},
    error::AppError,
    models::{null_data, ApiResponse, Plan},
    state::{Permission, RuntimeState},
};
use protocol::LocalDate;

use super::validate_sha256;

pub(super) async fn list_plans(
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

pub(super) async fn create_plan(
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

pub(super) async fn update_plan(
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

pub(super) async fn delete_plan(
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
