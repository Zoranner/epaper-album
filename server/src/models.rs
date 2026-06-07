use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct PlanResponse {
    pub version: String,
    pub plans: Vec<PlanEntry>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct PlanEntry {
    pub start: String,
    pub end: String,
    pub caption: String,
    pub images: Vec<String>,
}

#[derive(Debug, Clone, PartialEq, Eq, sqlx::FromRow)]
pub struct ImageRecord {
    pub sha256: String,
    pub content_type: String,
    pub bytes: Vec<u8>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, sqlx::FromRow)]
pub struct ImageSummary {
    pub sha256: String,
    pub content_type: String,
    pub size: i64,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
pub struct ImageSummaryResponse {
    pub sha256: String,
    pub content_type: String,
    pub size: i64,
    pub url: String,
}

impl From<ImageSummary> for ImageSummaryResponse {
    fn from(image: ImageSummary) -> Self {
        let url = format!("/images/{}", image.sha256);
        Self {
            sha256: image.sha256,
            content_type: image.content_type,
            size: image.size,
            url,
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
pub struct UploadImageResponse {
    pub sha256: String,
    pub url: String,
}
