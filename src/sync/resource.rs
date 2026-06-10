use crate::cloud::{
    image_url, parse_plan_response, parse_sprite_metadata_response, plans_url, sprite_download_url,
    sprite_url, CloudSyncError, HttpClient,
};
use crate::model::Plan;
use crate::storage::{ResourceStore, StorageJsonWrite, StorageWrite};
use sha2::{Digest, Sha256};
use std::fmt;

const MAX_PLAN_RESPONSE_BYTES: usize = 32 * 1024;
const MAX_SPRITE_META_BYTES: usize = 1024;
const MAX_IMAGE_BYTES: usize = 2 * 1024 * 1024;
const MAX_SPRITE_BYTES: usize = 256 * 1024;

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum ResourceSyncError {
    Cloud(CloudSyncError),
    Sha256Mismatch { expected: String, actual: String },
    Storage(StorageWrite),
    StorageJson(StorageJsonWrite),
}

impl fmt::Display for ResourceSyncError {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::Cloud(error) => write!(formatter, "cloud: {error}"),
            Self::Sha256Mismatch { expected, actual } => {
                write!(
                    formatter,
                    "sha256-mismatch: expected {expected}, actual {actual}"
                )
            }
            Self::Storage(error) => write!(formatter, "storage: {error:?}"),
            Self::StorageJson(error) => write!(formatter, "storage-json: {error:?}"),
        }
    }
}

impl std::error::Error for ResourceSyncError {}

impl From<CloudSyncError> for ResourceSyncError {
    fn from(error: CloudSyncError) -> Self {
        Self::Cloud(error)
    }
}

pub fn verify_sha256(expected: &str, bytes: &[u8]) -> Result<(), ResourceSyncError> {
    let actual = sha256_hex(bytes);
    let expected = expected.trim().to_ascii_lowercase();
    if actual == expected {
        Ok(())
    } else {
        Err(ResourceSyncError::Sha256Mismatch { expected, actual })
    }
}

pub fn download_plan(
    client: &mut impl HttpClient,
    store: &mut impl ResourceStore,
    base_url: &str,
    secret_key: &str,
    days: u8,
) -> Result<Vec<Plan>, ResourceSyncError> {
    let url = plans_url(base_url, days)?;
    let body = client.get_bytes(&url, secret_key, MAX_PLAN_RESPONSE_BYTES)?;
    let plans = parse_plan_response(&body)?;
    save_json_or_error(store.save_plans(&plans))?;
    Ok(plans)
}

pub fn download_image(
    client: &mut impl HttpClient,
    store: &mut impl ResourceStore,
    base_url: &str,
    secret_key: &str,
    sha256: &str,
) -> Result<(), ResourceSyncError> {
    let url = image_url(base_url, sha256)?;
    let bytes = client.get_bytes(&url, secret_key, MAX_IMAGE_BYTES)?;
    save_or_error(store.save_image_bytes(sha256, &bytes))?;
    Ok(())
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct SpriteSyncResult {
    pub sha256: String,
    pub downloaded: bool,
}

pub fn sync_sprite(
    client: &mut impl HttpClient,
    store: &mut impl ResourceStore,
    base_url: &str,
    secret_key: &str,
    kind: &str,
    text: &str,
) -> Result<SpriteSyncResult, ResourceSyncError> {
    let metadata_url = sprite_url(base_url, kind, text)?;
    let metadata_body = client.get_bytes(&metadata_url, secret_key, MAX_SPRITE_META_BYTES)?;
    let metadata = parse_sprite_metadata_response(&metadata_body)?;
    if store.has_sprite(&metadata.sha256) {
        return Ok(SpriteSyncResult {
            sha256: metadata.sha256,
            downloaded: false,
        });
    }

    let download_url = sprite_download_url(base_url, &metadata.sha256)?;
    let bytes = client.get_bytes(&download_url, secret_key, MAX_SPRITE_BYTES)?;
    save_or_error(store.save_sprite_bytes(&metadata.sha256, &bytes))?;
    Ok(SpriteSyncResult {
        sha256: metadata.sha256,
        downloaded: true,
    })
}

fn save_or_error(result: StorageWrite) -> Result<(), ResourceSyncError> {
    match result {
        StorageWrite::Written => Ok(()),
        error => Err(ResourceSyncError::Storage(error)),
    }
}

fn save_json_or_error(result: StorageJsonWrite) -> Result<(), ResourceSyncError> {
    match result {
        StorageJsonWrite::Written => Ok(()),
        error => Err(ResourceSyncError::StorageJson(error)),
    }
}

fn sha256_hex(bytes: &[u8]) -> String {
    let digest = Sha256::digest(bytes);
    hex_lower(&digest)
}

fn hex_lower(bytes: &[u8]) -> String {
    const HEX: &[u8; 16] = b"0123456789abcdef";
    let mut output = String::with_capacity(bytes.len() * 2);
    for byte in bytes {
        output.push(HEX[(byte >> 4) as usize] as char);
        output.push(HEX[(byte & 0x0f) as usize] as char);
    }
    output
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::collections::BTreeMap;

    #[derive(Default)]
    struct MockHttpClient {
        responses: BTreeMap<String, Vec<u8>>,
        requests: Vec<(String, String, usize)>,
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
            secret_key: &str,
            max_bytes: usize,
        ) -> Result<Vec<u8>, CloudSyncError> {
            self.requests
                .push((url.to_string(), secret_key.to_string(), max_bytes));
            self.responses
                .get(url)
                .cloned()
                .ok_or(CloudSyncError::HttpStatus(404))
        }
    }

    #[derive(Default)]
    struct MockResourceStore {
        plans: Option<Vec<Plan>>,
        images: BTreeMap<String, Vec<u8>>,
        sprites: BTreeMap<String, Vec<u8>>,
        write_result: Option<StorageWrite>,
    }

    impl MockResourceStore {
        fn result(&self) -> StorageWrite {
            self.write_result.clone().unwrap_or(StorageWrite::Written)
        }
    }

    impl ResourceStore for MockResourceStore {
        fn save_plans(&mut self, plans: &[Plan]) -> StorageJsonWrite {
            self.plans = Some(plans.to_vec());
            match self.result() {
                StorageWrite::Written => StorageJsonWrite::Written,
                StorageWrite::MountError => StorageJsonWrite::MountError,
                StorageWrite::WriteError => StorageJsonWrite::WriteError,
            }
        }

        fn save_image_bytes(&mut self, sha256: &str, content: &[u8]) -> StorageWrite {
            self.images.insert(sha256.to_string(), content.to_vec());
            self.result()
        }

        fn save_sprite_bytes(&mut self, sha256: &str, content: &[u8]) -> StorageWrite {
            self.sprites.insert(sha256.to_string(), content.to_vec());
            self.result()
        }

        fn has_image(&self, sha256: &str) -> bool {
            self.images.contains_key(sha256)
        }

        fn has_sprite(&self, sha256: &str) -> bool {
            self.sprites.contains_key(sha256)
        }
    }

    #[test]
    fn downloads_and_saves_plans() {
        let body = br#"{
            "code": 0,
            "message": "ok",
            "data": [
                {
                    "date": "2026-06-08",
                    "caption": "caption",
                    "image": "server-key"
                }
            ]
        }"#;
        let mut client =
            MockHttpClient::default().with_response("https://example.com/api/plans?days=2", body);
        let mut store = MockResourceStore::default();

        let plans =
            download_plan(&mut client, &mut store, "https://example.com", "secret", 2).unwrap();

        assert_eq!(plans[0].date.to_string(), "2026-06-08");
        assert_eq!(plans[0].caption, "caption");
        assert_eq!(plans[0].image, "server-key");
        assert_eq!(store.plans, Some(plans));
        assert_eq!(client.requests[0].1, "secret");
    }

    #[test]
    fn downloads_image_to_sha256_path_key() {
        let bytes = b"image bytes";
        let sha256 = "server-resource-key";
        let mut client = MockHttpClient::default()
            .with_response(&format!("https://example.com/images/{sha256}"), bytes);
        let mut store = MockResourceStore::default();

        download_image(
            &mut client,
            &mut store,
            "https://example.com",
            "secret",
            sha256,
        )
        .unwrap();

        assert_eq!(store.images.get(sha256), Some(&bytes.to_vec()));
    }

    #[test]
    fn syncs_sprite_meta_then_downloads_missing_bmp_by_sha256() {
        let sha256 = "1111111111111111111111111111111111111111111111111111111111111111";
        let mut client = MockHttpClient::default()
            .with_response(
                "https://example.com/api/sprites?type=caption&text=%E6%99%9A%E9%A3%8E",
                format!(r#"{{"code":0,"message":"ok","data":{{"sha256":"{sha256}"}}}}"#).as_bytes(),
            )
            .with_response(
                &format!("https://example.com/sprites/{sha256}"),
                b"sprite bytes",
            );
        let mut store = MockResourceStore::default();

        let result = sync_sprite(
            &mut client,
            &mut store,
            "https://example.com",
            "secret",
            "caption",
            "晚风",
        )
        .unwrap();

        assert_eq!(
            result,
            SpriteSyncResult {
                sha256: sha256.to_string(),
                downloaded: true
            }
        );
        assert_eq!(store.sprites.get(sha256), Some(&b"sprite bytes".to_vec()));
    }
}
