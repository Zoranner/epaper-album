use serde::{Deserialize, Serialize};
use serde_json::Value;

pub use schema::{PlanItem, PlanPayload};

#[derive(Debug, Clone, PartialEq, Eq, Serialize, sqlx::FromRow)]
pub struct ImageRecord {
    pub sha256: String,
    pub status: String,
    pub remark: String,
}

#[derive(Debug, Clone, PartialEq, Eq, Deserialize)]
pub struct LoginRequest {
    pub username: String,
    pub password: String,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
pub struct LoginResponse {
    #[serde(rename = "jwtToken")]
    pub jwt_token: String,
    #[serde(rename = "expiresAt")]
    pub expires_at: String,
}

#[derive(Debug, Clone, PartialEq, Eq, Deserialize)]
pub struct ImageRemarkPayload {
    pub remark: String,
}

#[derive(Debug, Clone, PartialEq, Eq, Deserialize)]
pub struct SpritePayload {
    #[serde(rename = "type")]
    pub kind: String,
    pub text: String,
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
