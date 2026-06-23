use super::*;
use crate::cloud::{plans_url, sprite_url, CloudSyncError, HttpClient};
use crate::device_runtime::DeviceCloudSync;
use crate::model::Plan;
use crate::resource_sync::ResourceSyncError;
use crate::storage::{ResourceStore, StorageBinaryRead, StorageJsonWrite, StorageWrite};
use std::collections::BTreeMap;

#[derive(Default)]
struct MockHttpClient {
    responses: BTreeMap<String, Vec<u8>>,
    errors: BTreeMap<String, CloudSyncError>,
    requests: Vec<String>,
}

impl MockHttpClient {
    fn with_response(mut self, url: &str, body: &[u8]) -> Self {
        self.responses.insert(url.to_string(), body.to_vec());
        self
    }

    fn with_error(mut self, url: &str, error: CloudSyncError) -> Self {
        self.errors.insert(url.to_string(), error);
        self
    }
}

impl HttpClient for MockHttpClient {
    fn get_bytes(
        &mut self,
        url: &str,
        _secret_key: &str,
        _max_bytes: usize,
    ) -> Result<Vec<u8>, CloudSyncError> {
        self.requests.push(url.to_string());
        if let Some(error) = self.errors.get(url) {
            return Err(error.clone());
        }
        self.responses
            .get(url)
            .cloned()
            .ok_or(CloudSyncError::HttpStatus(404))
    }
}

#[derive(Default)]
struct MockStore {
    plans: Option<Vec<Plan>>,
    images: Vec<String>,
    cached_images: Vec<String>,
    sprites: Vec<String>,
    invalid_images: Vec<String>,
    invalid_sprites: Vec<String>,
}

impl ResourceStore for MockStore {
    fn save_plans(&mut self, plans: &[Plan]) -> StorageJsonWrite {
        self.plans = Some(plans.to_vec());
        StorageJsonWrite::Written
    }

    fn save_image_bytes(&mut self, sha256: &str, _content: &[u8]) -> StorageWrite {
        self.images.push(sha256.to_string());
        self.cached_images.push(sha256.to_string());
        StorageWrite::Written
    }

    fn save_sprite_bytes(&mut self, sha256: &str, _content: &[u8]) -> StorageWrite {
        self.sprites.push(sha256.to_string());
        StorageWrite::Written
    }

    fn read_image_bytes(&self, sha256: &str) -> StorageBinaryRead {
        if !self.cached_images.iter().any(|image| image == sha256) {
            return StorageBinaryRead::Missing;
        }

        if self.invalid_images.iter().any(|image| image == sha256) {
            StorageBinaryRead::Bytes(b"not-a-bmp".to_vec())
        } else {
            StorageBinaryRead::Bytes(solid_bmp(800, 480))
        }
    }

    fn read_sprite_bytes(&self, sha256: &str) -> StorageBinaryRead {
        if !self.sprites.iter().any(|sprite| sprite == sha256) {
            return StorageBinaryRead::Missing;
        }

        if self.invalid_sprites.iter().any(|sprite| sprite == sha256) {
            StorageBinaryRead::Bytes(b"not-a-bmp".to_vec())
        } else {
            StorageBinaryRead::Bytes(solid_bmp(8, 4))
        }
    }

    fn has_image(&self, sha256: &str) -> bool {
        self.cached_images.iter().any(|image| image == sha256)
    }

    fn has_sprite(&self, sha256: &str) -> bool {
        self.sprites.iter().any(|sprite| sprite == sha256)
    }
}

fn request() -> crate::device_runtime::SyncRequest {
    crate::device_runtime::SyncRequest {
        config: crate::config::Config {
            wifi_ssid: "wifi".to_string(),
            wifi_password: "password".to_string(),
            base_url: "https://example.com".to_string(),
            secret_key: "secret".to_string(),
        },
        local_plans: None,
        date: crate::model::LocalDate::parse("2026-06-08").unwrap(),
        now_epoch_seconds: 1,
    }
}

fn sprite_meta(sha256: &str) -> Vec<u8> {
    format!(r#"{{"code":0,"message":"ok","data":{{"sha256":"{sha256}"}}}}"#).into_bytes()
}

fn solid_bmp(width: usize, height: usize) -> Vec<u8> {
    let row_stride = (width * 3).div_ceil(4) * 4;
    let pixel_offset = 54usize;
    let file_size = pixel_offset + row_stride * height;
    let mut bytes = vec![0u8; file_size];
    bytes[0..2].copy_from_slice(b"BM");
    bytes[2..6].copy_from_slice(&(file_size as u32).to_le_bytes());
    bytes[10..14].copy_from_slice(&(pixel_offset as u32).to_le_bytes());
    bytes[14..18].copy_from_slice(&40u32.to_le_bytes());
    bytes[18..22].copy_from_slice(&(width as i32).to_le_bytes());
    bytes[22..26].copy_from_slice(&(height as i32).to_le_bytes());
    bytes[26..28].copy_from_slice(&1u16.to_le_bytes());
    bytes[28..30].copy_from_slice(&24u16.to_le_bytes());

    bytes
}

#[test]
fn sync_downloads_remote_missing_image_and_sprites() {
    let body = br#"{
        "code": 0,
        "message": "ok",
        "data": [
            {
                "date": "2026-06-08",
                "caption": "caption",
                "image": "a"
            }
        ]
    }"#;
    let caption_sha = "1111111111111111111111111111111111111111111111111111111111111111";
    let date_sha = "2222222222222222222222222222222222222222222222222222222222222222";
    let client = MockHttpClient::default()
        .with_response(&plans_url("https://example.com", 3).unwrap(), body)
        .with_response("https://example.com/images/a", b"image-a")
        .with_response(
            "https://example.com/api/sprites?type=caption&text=caption",
            &sprite_meta(caption_sha),
        )
        .with_response(
            &format!("https://example.com/sprites/{caption_sha}"),
            b"caption-sprite",
        )
        .with_response(
            "https://example.com/api/sprites?type=date&text=2026-06-08",
            &sprite_meta(date_sha),
        )
        .with_response(
            &format!("https://example.com/sprites/{date_sha}"),
            b"date-sprite",
        );
    let store = MockStore::default();
    let mut sync = CloudResourceSync::new(client, store);

    let result = sync.sync_resources(request()).unwrap();
    let (client, store) = sync.into_parts();

    assert_eq!(result.plans[0].image, "a");
    assert_eq!(store.images, vec!["a"]);
    assert_eq!(store.sprites, vec![caption_sha, date_sha]);
    assert_eq!(result.sprites.caption.as_deref(), Some(caption_sha));
    assert_eq!(result.sprites.date.as_deref(), Some(date_sha));
    assert_eq!(
        client.requests,
        vec![
            plans_url("https://example.com", 3).unwrap(),
            "https://example.com/images/a".to_string(),
            "https://example.com/api/sprites?type=caption&text=caption".to_string(),
            format!("https://example.com/sprites/{caption_sha}"),
            "https://example.com/api/sprites?type=date&text=2026-06-08".to_string(),
            format!("https://example.com/sprites/{date_sha}")
        ]
    );
}

#[test]
fn sync_keeps_existing_images() {
    let body = br#"{
        "code": 0,
        "message": "ok",
        "data": [
            {
                "date": "2026-06-08",
                "caption": " ",
                "image": "a"
            }
        ]
    }"#;
    let date_sha = "2222222222222222222222222222222222222222222222222222222222222222";
    let client = MockHttpClient::default()
        .with_response(&plans_url("https://example.com", 3).unwrap(), body)
        .with_response(
            "https://example.com/api/sprites?type=date&text=2026-06-08",
            &sprite_meta(date_sha),
        )
        .with_response(
            &format!("https://example.com/sprites/{date_sha}"),
            b"date-sprite",
        );
    let store = MockStore {
        cached_images: vec!["a".to_string()],
        ..MockStore::default()
    };
    let mut sync = CloudResourceSync::new(client, store);

    sync.sync_resources(request()).unwrap();
    let (client, store) = sync.into_parts();

    assert!(store.images.is_empty());
    assert_eq!(
        client.requests,
        vec![
            plans_url("https://example.com", 3).unwrap(),
            "https://example.com/api/sprites?type=date&text=2026-06-08".to_string(),
            format!("https://example.com/sprites/{date_sha}")
        ]
    );
}

#[test]
fn sync_caption_sprite_matches_selected_plan_for_request_date() {
    let body = br#"{
        "code": 0,
        "message": "ok",
        "data": [
            {
                "date": "2026-06-08",
                "caption": "today",
                "image": "today-image"
            },
            {
                "date": "2026-06-09",
                "caption": "tomorrow",
                "image": "tomorrow-image"
            }
        ]
    }"#;
    let caption_sha = "1111111111111111111111111111111111111111111111111111111111111111";
    let date_sha = "2222222222222222222222222222222222222222222222222222222222222222";
    let client = MockHttpClient::default()
        .with_response(&plans_url("https://example.com", 3).unwrap(), body)
        .with_response(
            "https://example.com/api/sprites?type=caption&text=today",
            &sprite_meta(caption_sha),
        )
        .with_response(
            &format!("https://example.com/sprites/{caption_sha}"),
            b"caption-sprite",
        )
        .with_response(
            "https://example.com/api/sprites?type=date&text=2026-06-08",
            &sprite_meta(date_sha),
        )
        .with_response(
            &format!("https://example.com/sprites/{date_sha}"),
            b"date-sprite",
        );
    let store = MockStore {
        cached_images: vec!["today-image".to_string(), "tomorrow-image".to_string()],
        ..MockStore::default()
    };
    let mut sync = CloudResourceSync::new(client, store);

    let result = sync.sync_resources(request()).unwrap();
    let (client, _store) = sync.into_parts();

    assert_eq!(result.sprites.caption.as_deref(), Some(caption_sha));
    assert!(client
        .requests
        .iter()
        .any(|request| request == "https://example.com/api/sprites?type=caption&text=today"));
    assert!(!client
        .requests
        .iter()
        .any(|request| request == "https://example.com/api/sprites?type=caption&text=tomorrow"));
}

#[test]
fn sync_redownloads_unrenderable_cached_image_and_sprites() {
    let body = br#"{
        "code": 0,
        "message": "ok",
        "data": [
            {
                "date": "2026-06-08",
                "caption": "caption",
                "image": "a"
            }
        ]
    }"#;
    let caption_sha = "1111111111111111111111111111111111111111111111111111111111111111";
    let date_sha = "2222222222222222222222222222222222222222222222222222222222222222";
    let client = MockHttpClient::default()
        .with_response(&plans_url("https://example.com", 3).unwrap(), body)
        .with_response("https://example.com/images/a", b"fresh-image")
        .with_response(
            "https://example.com/api/sprites?type=caption&text=caption",
            &sprite_meta(caption_sha),
        )
        .with_response(
            &format!("https://example.com/sprites/{caption_sha}"),
            b"fresh-caption",
        )
        .with_response(
            "https://example.com/api/sprites?type=date&text=2026-06-08",
            &sprite_meta(date_sha),
        )
        .with_response(
            &format!("https://example.com/sprites/{date_sha}"),
            b"fresh-date",
        );
    let store = MockStore {
        cached_images: vec!["a".to_string()],
        invalid_images: vec!["a".to_string()],
        sprites: vec![caption_sha.to_string(), date_sha.to_string()],
        invalid_sprites: vec![caption_sha.to_string(), date_sha.to_string()],
        ..MockStore::default()
    };
    let mut sync = CloudResourceSync::new(client, store);

    sync.sync_resources(request()).unwrap();
    let (client, store) = sync.into_parts();

    assert_eq!(store.images, vec!["a"]);
    assert_eq!(
        store.sprites,
        vec![caption_sha, date_sha, caption_sha, date_sha]
    );
    assert_eq!(
        client.requests,
        vec![
            plans_url("https://example.com", 3).unwrap(),
            "https://example.com/images/a".to_string(),
            "https://example.com/api/sprites?type=caption&text=caption".to_string(),
            format!("https://example.com/sprites/{caption_sha}"),
            "https://example.com/api/sprites?type=date&text=2026-06-08".to_string(),
            format!("https://example.com/sprites/{date_sha}")
        ]
    );
}

#[test]
fn sync_error_includes_failed_stage() {
    let body = br#"{
        "code": 0,
        "message": "ok",
        "data": [
            {
                "date": "2026-06-08",
                "caption": " ",
                "image": "a"
            }
        ]
    }"#;
    let client = MockHttpClient::default()
        .with_response(&plans_url("https://example.com", 3).unwrap(), body);
    let store = MockStore {
        cached_images: vec!["a".to_string()],
        ..MockStore::default()
    };
    let mut sync = CloudResourceSync::new(client, store);

    let error = sync.sync_resources(request()).unwrap_err().to_string();

    assert!(error.contains("sprite date 2026-06-08"), "{error}");
    assert!(error.contains("cloud: http-status-404"), "{error}");
}

#[test]
fn caption_sprite_http_status_report_omits_non_ascii_caption() {
    let caption = "\u{53ef}\u{53ef}\u{7231}\u{7231}\u{5c0f}\u{4e1b}\u{4e1b}";
    let body = format!(
        r#"{{
        "code": 0,
        "message": "ok",
        "data": [
            {{
                "date": "2026-06-08",
                "caption": "{caption}",
                "image": "a"
            }}
        ]
    }}"#
    )
    .into_bytes();
    let client = MockHttpClient::default()
        .with_response(&plans_url("https://example.com", 3).unwrap(), &body)
        .with_error(
            &sprite_url("https://example.com", "caption", caption).unwrap(),
            CloudSyncError::HttpStatus(500),
        );
    let store = MockStore {
        cached_images: vec!["a".to_string()],
        ..MockStore::default()
    };
    let mut sync = CloudResourceSync::new(client, store);

    let error = sync.sync_resources(request()).unwrap_err();
    let report = error.report();

    assert_eq!(report.code, "resource.cloud.http-status");
    assert_eq!(report.category, "resource");
    assert_eq!(report.stage.as_deref(), Some("sprite caption"));
    assert_eq!(report.message, "sprite caption sync failed");
    assert_eq!(report.detail, "cloud: http-status-500");
    assert!(report.stage.unwrap().is_ascii());
    assert!(report.message.is_ascii());
    assert!(report.detail.is_ascii());
}

#[test]
fn sync_error_exposes_stable_error_page_fields() {
    let error = DeviceSyncError::resource(
        "sprite date 2026-06-08",
        ResourceSyncError::Cloud(CloudSyncError::HttpRequest(
            "ESP_ERR_HTTP_CONNECT".to_string(),
        )),
    );

    assert_eq!(error.code(), "resource.cloud.http-request");
    assert_eq!(error.category(), "resource");
    assert_eq!(error.stage(), "sprite date 2026-06-08");
    assert_eq!(error.message(), "sprite date 2026-06-08 sync failed");
    assert_eq!(
        error.detail(),
        Some("cloud: http-request: ESP_ERR_HTTP_CONNECT".to_string())
    );
    assert_eq!(
        error.to_string(),
        "sprite date 2026-06-08: cloud: http-request: ESP_ERR_HTTP_CONNECT"
    );
}
