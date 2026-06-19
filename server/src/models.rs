use serde::{Deserialize, Serialize};

pub use protocol::{ApiResponse, Plan, SpriteMeta};

#[derive(Debug, Clone, PartialEq, Eq, Serialize, sqlx::FromRow)]
pub struct ImageRecord {
    pub sha256: String,
    pub status: String,
    pub remark: String,
    #[sqlx(skip)]
    pub tags: Vec<String>,
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
pub struct ImagePayload {
    pub remark: String,
    #[serde(default)]
    pub tags: Vec<String>,
}

#[derive(Debug, Clone, PartialEq, Eq, Deserialize)]
pub struct SpritePayload {
    #[serde(rename = "type")]
    pub kind: String,
    pub text: String,
}

pub fn null_data() -> serde_json::Value {
    serde_json::Value::Null
}
