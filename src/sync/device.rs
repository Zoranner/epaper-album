use crate::cloud::HttpClient;
use crate::device_runtime::{DeviceCloudSync, SpriteSet, SyncRequest, SyncResult};
use crate::resource_sync::{download_image, download_plan, sync_sprite, ResourceSyncError};
use crate::storage::ResourceStore;
use std::fmt;

pub const PLAN_LOOKAHEAD_DAYS: u8 = 3;

pub struct CloudResourceSync<C, S> {
    client: C,
    store: S,
    days: u8,
}

impl<C, S> CloudResourceSync<C, S> {
    pub const fn new(client: C, store: S) -> Self {
        Self {
            client,
            store,
            days: PLAN_LOOKAHEAD_DAYS,
        }
    }

    pub const fn with_days(mut self, days: u8) -> Self {
        self.days = days;
        self
    }

    pub fn into_parts(self) -> (C, S) {
        (self.client, self.store)
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum DeviceSyncError {
    Resource(ResourceSyncError),
}

impl fmt::Display for DeviceSyncError {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::Resource(error) => write!(formatter, "{error}"),
        }
    }
}

impl std::error::Error for DeviceSyncError {}

impl From<ResourceSyncError> for DeviceSyncError {
    fn from(error: ResourceSyncError) -> Self {
        Self::Resource(error)
    }
}

impl<C, S> DeviceCloudSync for CloudResourceSync<C, S>
where
    C: HttpClient,
    S: ResourceStore,
{
    type Error = DeviceSyncError;

    fn sync_resources(&mut self, request: SyncRequest) -> Result<SyncResult, Self::Error> {
        let base_url = request.config.base_url.trim();
        let secret_key = request.config.secret_key.trim();
        let plans = download_plan(
            &mut self.client,
            &mut self.store,
            base_url,
            secret_key,
            self.days,
        )?;
        let mut sprites = SpriteSet::default();
        let mut sprite_download_count = 0usize;

        for plan in &plans {
            if self.store.has_image(&plan.image) {
                continue;
            }
            download_image(
                &mut self.client,
                &mut self.store,
                base_url,
                secret_key,
                &plan.image,
            )?;
        }

        for plan in &plans {
            if !plan.caption.trim().is_empty() {
                let result = sync_sprite(
                    &mut self.client,
                    &mut self.store,
                    base_url,
                    secret_key,
                    "caption",
                    &plan.caption,
                )?;
                sprites.caption = Some(result.sha256);
                if result.downloaded {
                    sprite_download_count += 1;
                }
            }
            let result = sync_sprite(
                &mut self.client,
                &mut self.store,
                base_url,
                secret_key,
                "date",
                &plan.date.to_string(),
            )?;
            sprites.date = Some(result.sha256);
            if result.downloaded {
                sprite_download_count += 1;
            }
        }

        if let Some(notice) = request.notice {
            let result = sync_sprite(
                &mut self.client,
                &mut self.store,
                base_url,
                secret_key,
                "notice",
                notice.text(),
            )?;
            sprites.notice = Some(result.sha256);
            if result.downloaded {
                sprite_download_count += 1;
            }
        }

        Ok(SyncResult {
            plans,
            sprites,
            sprites_changed: sprite_download_count > 0,
        })
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::cloud::{plans_url, CloudSyncError};
    use crate::model::Plan;
    use crate::storage::{StorageJsonWrite, StorageWrite};
    use std::collections::BTreeMap;

    #[derive(Default)]
    struct MockHttpClient {
        responses: BTreeMap<String, Vec<u8>>,
        requests: Vec<String>,
    }

    impl MockHttpClient {
        fn with_response(mut self, url: &str, body: &[u8]) -> Self {
            self.responses.insert(url.to_string(), body.to_vec());
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

        fn has_image(&self, sha256: &str) -> bool {
            self.cached_images.iter().any(|image| image == sha256)
        }

        fn has_sprite(&self, sha256: &str) -> bool {
            self.sprites.iter().any(|sprite| sprite == sha256)
        }
    }

    fn request() -> SyncRequest {
        SyncRequest {
            config: crate::config::Config {
                wifi_ssid: "wifi".to_string(),
                wifi_password: "password".to_string(),
                base_url: "https://example.com".to_string(),
                secret_key: "secret".to_string(),
            },
            local_plans: None,
            notice: None,
            now_epoch_seconds: 1,
        }
    }

    fn sprite_meta(sha256: &str) -> Vec<u8> {
        format!(r#"{{"code":0,"message":"ok","data":{{"sha256":"{sha256}"}}}}"#).into_bytes()
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
        assert!(result.sprites_changed);
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
}
