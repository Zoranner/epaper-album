use crate::cache::missing_resources;
use crate::cloud::HttpClient;
use crate::device_runtime::{DeviceCloudSync, SyncRequest, SyncResult};
use crate::resource_sync::{download_image, download_plan, ResourceSyncError};
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
        let snapshot = download_plan(
            &mut self.client,
            &mut self.store,
            base_url,
            secret_key,
            self.days,
        )?;
        let missing = missing_resources(&snapshot, &request.resource_index);
        let mut resources = Vec::with_capacity(missing.len());

        for sha256 in missing {
            resources.push(download_image(
                &mut self.client,
                &mut self.store,
                base_url,
                secret_key,
                &sha256,
            )?);
        }

        Ok(SyncResult {
            snapshot,
            resources,
        })
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::cloud::{plans_url, CloudSyncError};
    use crate::model::{PlanSnapshot, ResourceIndex};
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
        snapshot: Option<PlanSnapshot>,
        images: Vec<String>,
    }

    impl ResourceStore for MockStore {
        fn save_plan_snapshot(&mut self, snapshot: &PlanSnapshot) -> StorageJsonWrite {
            self.snapshot = Some(snapshot.clone());
            StorageJsonWrite::Written
        }

        fn save_image_bytes(&mut self, sha256: &str, _content: &[u8]) -> StorageWrite {
            self.images.push(sha256.to_string());
            StorageWrite::Written
        }

        fn save_sprite_bytes(&mut self, _sha256: &str, _content: &[u8]) -> StorageWrite {
            StorageWrite::Written
        }
    }

    fn request(index: ResourceIndex) -> SyncRequest {
        SyncRequest {
            config: crate::config::Config {
                wifi_ssid: "wifi".to_string(),
                wifi_password: "password".to_string(),
                base_url: "https://example.com".to_string(),
                secret_key: "secret".to_string(),
            },
            local_snapshot: None,
            resource_index: index,
            missing_resources: Vec::new(),
            now_epoch_seconds: 1,
        }
    }

    #[test]
    fn sync_downloads_plan_and_remote_missing_image() {
        let body = br#"{
            "code": 0,
            "message": "ok",
            "data": [
                {
                    "date": "2026-06-08",
                    "caption": "caption",
                    "image_sha256": "a"
                }
            ]
        }"#;
        let client = MockHttpClient::default()
            .with_response(&plans_url("https://example.com", 3).unwrap(), body)
            .with_response("https://example.com/images/a", b"image-a");
        let store = MockStore::default();
        let mut sync = CloudResourceSync::new(client, store);

        let result = sync
            .sync_resources(request(ResourceIndex::default()))
            .unwrap();
        let (client, store) = sync.into_parts();

        assert_eq!(result.snapshot.plans[0].image_sha256, "a");
        assert_eq!(
            result
                .resources
                .iter()
                .map(|resource| resource.sha256.as_str())
                .collect::<Vec<_>>(),
            vec!["a"]
        );
        assert_eq!(store.images, vec!["a"]);
        assert_eq!(client.requests.len(), 2);
    }

    #[test]
    fn sync_keeps_cached_images() {
        let body = br#"{
            "code": 0,
            "message": "ok",
            "data": [
                {
                    "date": "2026-06-08",
                    "caption": "caption",
                    "image_sha256": "a"
                }
            ]
        }"#;
        let client = MockHttpClient::default()
            .with_response(&plans_url("https://example.com", 3).unwrap(), body);
        let store = MockStore::default();
        let mut sync = CloudResourceSync::new(client, store);
        let index = ResourceIndex {
            resources: vec![crate::model::CachedResource {
                sha256: "a".to_string(),
                byte_size: 10,
                last_used_at_unix_secs: 1,
            }],
        };

        let result = sync.sync_resources(request(index)).unwrap();
        let (client, store) = sync.into_parts();

        assert!(result.resources.is_empty());
        assert!(store.images.is_empty());
        assert_eq!(client.requests.len(), 1);
    }
}
