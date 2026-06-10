use crate::config::Config;
use crate::model::{ApiResponse, Plan, SpriteMeta};
use serde::Deserialize;
use serde_json::Value;
use std::fmt;

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum CloudSyncError {
    InvalidBaseUrl,
    HttpClient,
    HttpRequest,
    HttpResponse,
    HttpRead,
    HttpStatus(u16),
    ApiStatus(u16, String),
    InvalidJson,
    InvalidSha256,
}

impl fmt::Display for CloudSyncError {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::InvalidBaseUrl => formatter.write_str("invalid-base-url"),
            Self::HttpClient => formatter.write_str("http-client"),
            Self::HttpRequest => formatter.write_str("http-request"),
            Self::HttpResponse => formatter.write_str("http-response"),
            Self::HttpRead => formatter.write_str("http-read"),
            Self::HttpStatus(status) => write!(formatter, "http-status-{status}"),
            Self::ApiStatus(code, message) => write!(formatter, "api-status-{code}: {message}"),
            Self::InvalidJson => formatter.write_str("invalid-json"),
            Self::InvalidSha256 => formatter.write_str("invalid-sha256"),
        }
    }
}

impl std::error::Error for CloudSyncError {}

pub trait HttpClient {
    fn get_bytes(
        &mut self,
        url: &str,
        secret_key: &str,
        max_bytes: usize,
    ) -> Result<Vec<u8>, CloudSyncError>;
}

#[derive(Debug, Deserialize)]
struct RawApiResponse {
    code: u16,
    message: String,
    data: Value,
}

pub fn plans_url(base_url: &str, days: u8) -> Result<String, CloudSyncError> {
    endpoint_url(base_url, &format!("api/plans?days={}", days.clamp(1, 7)))
}

pub fn image_url(base_url: &str, sha256: &str) -> Result<String, CloudSyncError> {
    endpoint_url(base_url, &format!("images/{sha256}"))
}

pub fn sprite_url(base_url: &str, kind: &str, text: &str) -> Result<String, CloudSyncError> {
    endpoint_url(
        base_url,
        &format!(
            "api/sprites?type={}&text={}",
            percent_encode_query(kind),
            percent_encode_query(text)
        ),
    )
}

pub fn sprite_download_url(base_url: &str, sha256: &str) -> Result<String, CloudSyncError> {
    if !is_sha256_hex(sha256) {
        return Err(CloudSyncError::InvalidSha256);
    }
    endpoint_url(base_url, &format!("sprites/{sha256}"))
}

pub fn parse_plan_response(body: &[u8]) -> Result<Vec<Plan>, CloudSyncError> {
    let response =
        serde_json::from_slice::<RawApiResponse>(body).map_err(|_| CloudSyncError::InvalidJson)?;
    if response.code != 0 {
        return Err(CloudSyncError::ApiStatus(response.code, response.message));
    }

    let response: ApiResponse<Vec<Plan>> = ApiResponse {
        code: response.code,
        message: response.message,
        data: serde_json::from_value(response.data).map_err(|_| CloudSyncError::InvalidJson)?,
    };

    Ok(response.data)
}

pub fn parse_sprite_metadata_response(body: &[u8]) -> Result<SpriteMeta, CloudSyncError> {
    let response =
        serde_json::from_slice::<RawApiResponse>(body).map_err(|_| CloudSyncError::InvalidJson)?;
    if response.code != 0 {
        return Err(CloudSyncError::ApiStatus(response.code, response.message));
    }

    let response: ApiResponse<SpriteMeta> = ApiResponse {
        code: response.code,
        message: response.message,
        data: serde_json::from_value(response.data).map_err(|_| CloudSyncError::InvalidJson)?,
    };

    if !is_sha256_hex(&response.data.sha256) {
        return Err(CloudSyncError::InvalidSha256);
    }
    Ok(response.data)
}

pub fn parse_typed_api_response<T>(body: &[u8]) -> Result<ApiResponse<T>, CloudSyncError>
where
    T: serde::de::DeserializeOwned,
{
    let response =
        serde_json::from_slice::<ApiResponse<T>>(body).map_err(|_| CloudSyncError::InvalidJson)?;
    if response.code != 0 {
        return Err(CloudSyncError::ApiStatus(response.code, response.message));
    }

    Ok(response)
}

pub fn trim_secret_key(config: &Config) -> &str {
    config.secret_key.trim()
}

fn endpoint_url(base_url: &str, path: &str) -> Result<String, CloudSyncError> {
    let base_url = base_url.trim().trim_end_matches('/');
    if !(base_url.starts_with("http://") || base_url.starts_with("https://")) {
        return Err(CloudSyncError::InvalidBaseUrl);
    }

    Ok(format!("{base_url}/{path}"))
}

pub fn is_sha256_hex(value: &str) -> bool {
    value.len() == 64 && value.bytes().all(|byte| byte.is_ascii_hexdigit())
}

fn percent_encode_query(value: &str) -> String {
    let mut output = String::new();
    for byte in value.as_bytes() {
        match *byte {
            b'A'..=b'Z' | b'a'..=b'z' | b'0'..=b'9' | b'-' | b'.' | b'_' | b'~' => {
                output.push(*byte as char);
            }
            byte => {
                output.push('%');
                output.push(HEX_UPPER[(byte >> 4) as usize] as char);
                output.push(HEX_UPPER[(byte & 0x0f) as usize] as char);
            }
        }
    }
    output
}

const HEX_UPPER: &[u8; 16] = b"0123456789ABCDEF";

#[cfg(target_os = "espidf")]
pub mod espidf {
    use core::time::Duration;

    use esp_idf_svc::http::client::{
        Configuration as HttpConfiguration, EspHttpConnection, FollowRedirectsPolicy,
    };
    use esp_idf_svc::http::Method;

    use super::{parse_plan_response, plans_url, trim_secret_key, CloudSyncError, HttpClient};
    use crate::config::Config;
    use crate::model::Plan;

    const HTTP_TIMEOUT_SECONDS: u64 = 15;
    const MAX_PLAN_RESPONSE_BYTES: usize = 32 * 1024;

    pub struct EspIdfHttpClient;

    pub fn fetch_plan_response(config: &Config, days: u8) -> Result<Vec<Plan>, CloudSyncError> {
        let url = plans_url(&config.base_url, days)?;
        let mut client = EspIdfHttpClient;
        let body = client.get_bytes(&url, trim_secret_key(config), MAX_PLAN_RESPONSE_BYTES)?;
        parse_plan_response(&body)
    }

    impl HttpClient for EspIdfHttpClient {
        fn get_bytes(
            &mut self,
            url: &str,
            secret_key: &str,
            max_bytes: usize,
        ) -> Result<Vec<u8>, CloudSyncError> {
            get_bytes(url, secret_key, max_bytes)
        }
    }

    pub fn get_bytes(
        url: &str,
        secret_key: &str,
        max_bytes: usize,
    ) -> Result<Vec<u8>, CloudSyncError> {
        let mut connection = EspHttpConnection::new(&HttpConfiguration {
            timeout: Some(Duration::from_secs(HTTP_TIMEOUT_SECONDS)),
            follow_redirects_policy: FollowRedirectsPolicy::FollowAll,
            crt_bundle_attach: Some(esp_idf_sys::esp_crt_bundle_attach),
            ..Default::default()
        })
        .map_err(|_| CloudSyncError::HttpClient)?;
        connection
            .initiate_request(Method::Get, url, &[("secret-key", secret_key)])
            .map_err(|_| CloudSyncError::HttpRequest)?;
        connection
            .initiate_response()
            .map_err(|_| CloudSyncError::HttpResponse)?;

        let status = connection.status();
        if status != 200 {
            return Err(CloudSyncError::HttpStatus(status));
        }

        let mut body = Vec::new();
        let mut buffer = [0u8; 512];
        loop {
            let read_len = connection
                .read(&mut buffer)
                .map_err(|_| CloudSyncError::HttpRead)?;
            if read_len == 0 {
                break;
            }
            if body.len().saturating_add(read_len) > max_bytes {
                return Err(CloudSyncError::HttpRead);
            }
            body.extend_from_slice(&buffer[..read_len]);
        }

        Ok(body)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn builds_server_endpoint_urls() {
        assert_eq!(
            plans_url("https://example.com/epaper/", 9).unwrap(),
            "https://example.com/epaper/api/plans?days=7"
        );
        assert_eq!(
            image_url("http://example.com", "abc").unwrap(),
            "http://example.com/images/abc"
        );
        assert_eq!(
            sprite_url("http://example.com", "caption", "晚风 2026").unwrap(),
            "http://example.com/api/sprites?type=caption&text=%E6%99%9A%E9%A3%8E%202026"
        );
        assert_eq!(
            sprite_download_url(
                "http://example.com",
                "1111111111111111111111111111111111111111111111111111111111111111"
            )
            .unwrap(),
            "http://example.com/sprites/1111111111111111111111111111111111111111111111111111111111111111"
        );
        assert_eq!(
            plans_url("example.com", 3),
            Err(CloudSyncError::InvalidBaseUrl)
        );
    }

    #[test]
    fn parses_current_server_plan_response() {
        let body = r#"{
            "code": 0,
            "message": "ok",
            "data": [
                {
                    "date": "2026-06-08",
                    "caption": "晚风",
                    "image": "1111111111111111111111111111111111111111111111111111111111111111"
                }
            ]
        }"#;

        let plans = parse_plan_response(body.as_bytes()).unwrap();

        assert_eq!(plans.len(), 1);
        assert_eq!(plans[0].caption, "晚风");
        assert_eq!(plans[0].date.to_string(), "2026-06-08");
        assert_eq!(
            plans[0].image,
            "1111111111111111111111111111111111111111111111111111111111111111"
        );
    }

    #[test]
    fn rejects_non_ok_server_response() {
        let error = parse_plan_response(br#"{"code":401,"message":"Unauthorized","data":null}"#)
            .unwrap_err();

        assert_eq!(
            error,
            CloudSyncError::ApiStatus(401, "Unauthorized".to_string())
        );
    }

    #[test]
    fn parses_sprite_metadata_response() {
        let body = br#"{
            "code": 0,
            "message": "ok",
            "data": {
                "sha256": "1111111111111111111111111111111111111111111111111111111111111111"
            }
        }"#;

        let metadata = parse_sprite_metadata_response(body).unwrap();

        assert_eq!(
            metadata.sha256,
            "1111111111111111111111111111111111111111111111111111111111111111"
        );
    }

    #[test]
    fn rejects_invalid_sprite_metadata_response() {
        let error =
            parse_sprite_metadata_response(br#"{"code":0,"message":"ok","data":{"sha256":"bad"}}"#)
                .unwrap_err();

        assert_eq!(error, CloudSyncError::InvalidSha256);
    }
}
