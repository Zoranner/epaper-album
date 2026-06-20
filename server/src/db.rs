mod images;
mod plans;
mod schema;

use anyhow::{anyhow, Result};
use chrono::Utc;
use protocol::{ImageStatus, Plan, PlanType};
use sqlx::SqlitePool;

pub use schema::init_schema;

#[derive(Debug, Clone)]
pub struct Store {
    pool: SqlitePool,
}

impl Store {
    pub fn new(pool: SqlitePool) -> Self {
        Self { pool }
    }

    pub fn pool(&self) -> &SqlitePool {
        &self.pool
    }
}

fn plan_type_str(plan_type: &PlanType) -> &'static str {
    match plan_type {
        PlanType::Fixed => "fixed",
        PlanType::Random => "random",
    }
}

fn image_status_from_str(value: &str) -> Result<ImageStatus> {
    match value {
        "pending" => Ok(ImageStatus::Pending),
        "processing" => Ok(ImageStatus::Processing),
        "ready" => Ok(ImageStatus::Ready),
        "failed" => Ok(ImageStatus::Failed),
        _ => Err(anyhow!("Stored image status is invalid: {value}")),
    }
}

fn image_status_str(status: &ImageStatus) -> &'static str {
    match status {
        ImageStatus::Pending => "pending",
        ImageStatus::Processing => "processing",
        ImageStatus::Ready => "ready",
        ImageStatus::Failed => "failed",
    }
}

fn now_string() -> String {
    Utc::now().to_rfc3339()
}

fn normalize_plan(mut plan: Plan) -> Plan {
    plan.caption = plan.caption.trim().to_string();
    plan.tags = normalized_tags(&plan.tags);
    match plan.plan_type {
        PlanType::Fixed => plan.tags.clear(),
        PlanType::Random => plan.image.clear(),
    }
    plan
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
