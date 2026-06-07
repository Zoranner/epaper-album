use serde::{Deserialize, Serialize};
use serde_json::Value;

#[derive(Debug, Clone, PartialEq, Eq, Serialize, sqlx::FromRow)]
pub struct ImageRecord {
    pub sha256: String,
    pub status: String,
    pub remark: String,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
pub struct AdminPlan {
    pub id: i64,
    pub start: String,
    pub end: String,
    pub caption: String,
    pub images: Vec<ImageRecord>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
pub struct UserPlan {
    pub id: i64,
    pub start: String,
    pub end: String,
    pub caption: String,
    pub images: Vec<String>,
}

#[derive(Debug, Clone, PartialEq, Eq, Deserialize)]
pub struct LoginRequest {
    pub username: String,
    pub password: String,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
pub struct LoginResponse {
    pub token: String,
}

#[derive(Debug, Clone, PartialEq, Eq, Deserialize)]
pub struct PlanPayload {
    pub start: String,
    pub end: String,
    pub caption: String,
    #[serde(default)]
    pub images: Vec<String>,
}

#[derive(Debug, Clone, PartialEq, Eq, Deserialize)]
pub struct ImageRemarkPayload {
    pub remark: String,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
pub struct ApiResponse<T: Serialize> {
    pub code: u16,
    pub message: String,
    pub data: T,
}

impl<T: Serialize> ApiResponse<T> {
    pub fn ok(data: T) -> Self {
        Self {
            code: 0,
            message: "ok".to_string(),
            data,
        }
    }
}

pub fn null_data() -> Value {
    Value::Null
}
