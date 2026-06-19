use serde::{Deserialize, Serialize};

pub use protocol::{ApiResponse, ImagePayload, Plan, SpriteMeta};

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
pub struct SpritePayload {
    #[serde(rename = "type")]
    pub kind: String,
    pub text: String,
}

pub fn null_data() -> serde_json::Value {
    serde_json::Value::Null
}
