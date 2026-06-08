use crate::cloud::{
    image_url, parse_plan_response, plans_url, sprite_cache_key, sprite_url, CloudSyncError,
    HttpClient,
};
use crate::model::{CachedResource, PlanSnapshot};
use crate::storage::{ResourceStore, StorageJsonWrite, StorageWrite};
use sha2::{Digest, Sha256};
use std::fmt;

const MAX_PLAN_RESPONSE_BYTES: usize = 32 * 1024;
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
) -> Result<PlanSnapshot, ResourceSyncError> {
    let url = plans_url(base_url, days)?;
    let body = client.get_bytes(&url, secret_key, MAX_PLAN_RESPONSE_BYTES)?;
    let snapshot = parse_plan_response(&body)?;
    save_json_or_error(store.save_plan_snapshot(&snapshot))?;
    Ok(snapshot)
}

pub fn download_image(
    client: &mut impl HttpClient,
    store: &mut impl ResourceStore,
    base_url: &str,
    secret_key: &str,
    sha256: &str,
) -> Result<CachedResource, ResourceSyncError> {
    let url = image_url(base_url, sha256)?;
    let bytes = client.get_bytes(&url, secret_key, MAX_IMAGE_BYTES)?;
    let byte_size = bytes.len() as u64;
    save_or_error(store.save_image_bytes(sha256, &bytes))?;
    Ok(CachedResource {
        sha256: sha256.to_string(),
        byte_size,
        last_used_at_unix_secs: 0,
    })
}

pub fn download_sprite(
    client: &mut impl HttpClient,
    store: &mut impl ResourceStore,
    base_url: &str,
    secret_key: &str,
    kind: &str,
    text: &str,
) -> Result<String, ResourceSyncError> {
    let url = sprite_url(base_url, kind, text)?;
    let bytes = client.get_bytes(&url, secret_key, MAX_SPRITE_BYTES)?;
    let key = sprite_cache_key(kind, text);
    save_or_error(store.save_sprite_bytes(&key, &bytes))?;
    Ok(key)
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
        plan: Option<Vec<u8>>,
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
        fn save_plan_snapshot(&mut self, snapshot: &PlanSnapshot) -> StorageJsonWrite {
            self.plan = Some(
                crate::storage::to_json_string(snapshot)
                    .unwrap()
                    .into_bytes(),
            );
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

        fn save_sprite_bytes(&mut self, key: &str, content: &[u8]) -> StorageWrite {
            self.sprites.insert(key.to_string(), content.to_vec());
            self.result()
        }
    }

    #[test]
    fn verifies_sha256_digest() {
        let expected = "2cf24dba5fb0a30e26e83b2ac5b9e29e1b161e5c1fa7425e73043362938b9824";

        assert!(verify_sha256(expected, b"hello").is_ok());

        assert_eq!(
            verify_sha256(expected, b"changed").unwrap_err(),
            ResourceSyncError::Sha256Mismatch {
                expected: expected.to_string(),
                actual: sha256_hex(b"changed")
            }
        );
    }

    #[test]
    fn downloads_and_saves_plan_snapshot() {
        let body = br#"{
            "code": 0,
            "message": "ok",
            "data": [
                {
                    "id": 9,
                    "start": "2026-06-08",
                    "end": "2026-06-09",
                    "caption": "caption",
                    "images": []
                }
            ]
        }"#;
        let mut client =
            MockHttpClient::default().with_response("https://example.com/api/plans?days=2", body);
        let mut store = MockResourceStore::default();

        let snapshot =
            download_plan(&mut client, &mut store, "https://example.com", "secret", 2).unwrap();

        assert_eq!(snapshot.plans[0].id, 9);
        let stored_snapshot: PlanSnapshot = crate::storage::parse_json_str(
            std::str::from_utf8(store.plan.as_ref().unwrap()).unwrap(),
        )
        .unwrap();
        assert_eq!(stored_snapshot, snapshot);
        assert_eq!(client.requests[0].1, "secret");
    }

    #[test]
    fn saved_plan_can_be_read_as_plan_snapshot() {
        let body = br#"{
            "code": 0,
            "message": "ok",
            "data": [
                {
                    "id": 9,
                    "start": "2026-06-08",
                    "end": "2026-06-09",
                    "caption": "caption",
                    "images": ["server-key"]
                }
            ]
        }"#;
        let mut client =
            MockHttpClient::default().with_response("https://example.com/api/plans?days=2", body);
        let mut store = MockResourceStore::default();
        let snapshot =
            download_plan(&mut client, &mut store, "https://example.com", "secret", 2).unwrap();
        let temp_dir = tempfile::tempdir().unwrap();
        let plan_path = temp_dir.path().join("plans").join("current.json");

        crate::storage::write_binary_file_atomic(&plan_path, store.plan.as_ref().unwrap());
        let read_result: crate::storage::StorageJsonRead<PlanSnapshot> =
            crate::storage::read_json_file(&plan_path);

        assert_eq!(
            read_result,
            crate::storage::StorageJsonRead::Value(snapshot)
        );
    }

    #[test]
    fn downloads_image_as_cacheable_resource_without_body_sha_check() {
        let bytes = b"image bytes";
        let sha256 = "server-resource-key";
        let mut client = MockHttpClient::default()
            .with_response(&format!("https://example.com/images/{sha256}"), bytes);
        let mut store = MockResourceStore::default();

        let resource = download_image(
            &mut client,
            &mut store,
            "https://example.com",
            "secret",
            sha256,
        )
        .unwrap();

        assert_eq!(
            resource,
            CachedResource {
                sha256: sha256.to_string(),
                byte_size: bytes.len() as u64,
                last_used_at_unix_secs: 0
            }
        );
        assert_eq!(store.images.get(sha256), Some(&bytes.to_vec()));
    }

    #[test]
    fn downloads_sprite_by_cache_key_without_image_sha_check() {
        let mut client = MockHttpClient::default().with_response(
            "https://example.com/api/sprite?type=caption&text=%E6%99%9A%E9%A3%8E",
            b"sprite bytes",
        );
        let mut store = MockResourceStore::default();

        let key = download_sprite(
            &mut client,
            &mut store,
            "https://example.com",
            "secret",
            "caption",
            "晚风",
        )
        .unwrap();

        assert_eq!(key, sprite_cache_key("caption", "晚风"));
        assert_eq!(store.sprites.get(&key), Some(&b"sprite bytes".to_vec()));
    }
}
