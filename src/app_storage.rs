use crate::model::{DisplayState, PlanSnapshot, ResourceIndex};
use crate::state::PersistentDeviceState;
use crate::storage::{
    read_json_file, to_json_string, write_json_file_atomic, StorageJsonRead, StorageJsonWrite,
    CACHE_INDEX_PATH, DISPLAY_STATE_PATH, PLANS_CURRENT_PATH,
};
use std::path::{Path, PathBuf};

pub const DEVICE_STATE_PATH: &str = "/sdcard/epaper-album/device-state.json";

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct AppPaths {
    pub plan_snapshot: PathBuf,
    pub resource_index: PathBuf,
    // device-state.json is authoritative; display-state.json is an optional display snapshot.
    pub display_state: PathBuf,
    pub device_state: PathBuf,
}

impl AppPaths {
    pub fn new(
        plan_snapshot: impl Into<PathBuf>,
        resource_index: impl Into<PathBuf>,
        display_state: impl Into<PathBuf>,
        device_state: impl Into<PathBuf>,
    ) -> Self {
        Self {
            plan_snapshot: plan_snapshot.into(),
            resource_index: resource_index.into(),
            display_state: display_state.into(),
            device_state: device_state.into(),
        }
    }
}

impl Default for AppPaths {
    fn default() -> Self {
        Self {
            plan_snapshot: PathBuf::from(PLANS_CURRENT_PATH),
            resource_index: PathBuf::from(CACHE_INDEX_PATH),
            display_state: PathBuf::from(DISPLAY_STATE_PATH),
            device_state: PathBuf::from(DEVICE_STATE_PATH),
        }
    }
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct AppFiles {
    pub paths: AppPaths,
}

impl AppFiles {
    pub fn new(paths: AppPaths) -> Self {
        Self { paths }
    }

    pub fn read_plan_snapshot(&self) -> StorageJsonRead<PlanSnapshot> {
        read_plan_snapshot_file(&self.paths.plan_snapshot)
    }

    pub fn write_plan_snapshot(&self, snapshot: &PlanSnapshot) -> StorageJsonWrite {
        write_plan_snapshot_file(&self.paths.plan_snapshot, snapshot)
    }

    pub fn read_resource_index(&self) -> StorageJsonRead<ResourceIndex> {
        read_resource_index_file(&self.paths.resource_index)
    }

    pub fn write_resource_index(&self, index: &ResourceIndex) -> StorageJsonWrite {
        write_resource_index_file(&self.paths.resource_index, index)
    }

    pub fn read_display_state(&self) -> StorageJsonRead<DisplayState> {
        read_display_state_file(&self.paths.display_state)
    }

    pub fn write_display_state(&self, state: &DisplayState) -> StorageJsonWrite {
        write_display_state_file(&self.paths.display_state, state)
    }

    pub fn read_device_state(&self) -> StorageJsonRead<PersistentDeviceState> {
        read_device_state_file(&self.paths.device_state)
    }

    pub fn write_device_state(&self, state: &PersistentDeviceState) -> StorageJsonWrite {
        write_device_state_file(&self.paths.device_state, state)
    }
}

impl Default for AppFiles {
    fn default() -> Self {
        Self::new(AppPaths::default())
    }
}

pub fn plan_snapshot_from_json(content: &str) -> Result<PlanSnapshot, serde_json::Error> {
    crate::storage::parse_json_str(content)
}

pub fn plan_snapshot_to_json(snapshot: &PlanSnapshot) -> Result<String, serde_json::Error> {
    to_json_string(snapshot)
}

pub fn resource_index_from_json(content: &str) -> Result<ResourceIndex, serde_json::Error> {
    crate::storage::parse_json_str(content)
}

pub fn resource_index_to_json(index: &ResourceIndex) -> Result<String, serde_json::Error> {
    to_json_string(index)
}

pub fn display_state_from_json(content: &str) -> Result<DisplayState, serde_json::Error> {
    crate::storage::parse_json_str(content)
}

pub fn display_state_to_json(state: &DisplayState) -> Result<String, serde_json::Error> {
    to_json_string(state)
}

pub fn device_state_from_json(content: &str) -> Result<PersistentDeviceState, serde_json::Error> {
    crate::storage::parse_json_str(content)
}

pub fn device_state_to_json(state: &PersistentDeviceState) -> Result<String, serde_json::Error> {
    to_json_string(state)
}

pub fn read_plan_snapshot_file(path: impl AsRef<Path>) -> StorageJsonRead<PlanSnapshot> {
    read_json_file(path)
}

pub fn write_plan_snapshot_file(
    path: impl AsRef<Path>,
    snapshot: &PlanSnapshot,
) -> StorageJsonWrite {
    write_json_file_atomic(path, snapshot)
}

pub fn read_resource_index_file(path: impl AsRef<Path>) -> StorageJsonRead<ResourceIndex> {
    read_json_file(path)
}

pub fn write_resource_index_file(
    path: impl AsRef<Path>,
    index: &ResourceIndex,
) -> StorageJsonWrite {
    write_json_file_atomic(path, index)
}

pub fn read_display_state_file(path: impl AsRef<Path>) -> StorageJsonRead<DisplayState> {
    read_json_file(path)
}

pub fn write_display_state_file(path: impl AsRef<Path>, state: &DisplayState) -> StorageJsonWrite {
    write_json_file_atomic(path, state)
}

pub fn read_device_state_file(path: impl AsRef<Path>) -> StorageJsonRead<PersistentDeviceState> {
    read_json_file(path)
}

pub fn write_device_state_file(
    path: impl AsRef<Path>,
    state: &PersistentDeviceState,
) -> StorageJsonWrite {
    write_json_file_atomic(path, state)
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::model::{CachedResource, LocalDate, PlanItem};

    fn date(value: &str) -> LocalDate {
        LocalDate::parse(value).unwrap()
    }

    fn snapshot(images: &[&str]) -> PlanSnapshot {
        PlanSnapshot {
            content_hash: "hash-v1".to_string(),
            plans: vec![PlanItem {
                id: 7,
                start: date("2026-06-08"),
                end: date("2026-06-08"),
                caption: "caption".to_string(),
                images: images.iter().map(|image| image.to_string()).collect(),
            }],
        }
    }

    fn index(images: &[&str]) -> ResourceIndex {
        ResourceIndex {
            resources: images
                .iter()
                .map(|image| CachedResource {
                    sha256: image.to_string(),
                    byte_size: 128,
                    last_used_at_unix_secs: 1,
                })
                .collect(),
        }
    }

    #[test]
    fn app_files_round_trip_persistent_json() {
        let temp_dir = tempfile::tempdir().unwrap();
        let files = AppFiles::new(AppPaths::new(
            temp_dir.path().join("plans").join("current.json"),
            temp_dir.path().join("cache-index.json"),
            temp_dir.path().join("display-state.json"),
            temp_dir.path().join("device-state.json"),
        ));
        let snapshot = snapshot(&["a"]);
        let resource_index = index(&["a"]);
        let display_state = DisplayState {
            plan_id: Some(7),
            plan_content_hash: Some("hash-v1".to_string()),
            date: Some(date("2026-06-08")),
            image_sha256: Some("a".to_string()),
            image_index: 0,
            caption: Some("caption".to_string()),
            refreshed_at_unix_secs: Some(100),
        };
        let mut device_state = PersistentDeviceState::default();
        device_state.set_current_display(&display_state);

        assert_eq!(
            files.write_plan_snapshot(&snapshot),
            StorageJsonWrite::Written
        );
        assert_eq!(
            files.write_resource_index(&resource_index),
            StorageJsonWrite::Written
        );
        assert_eq!(
            files.write_display_state(&display_state),
            StorageJsonWrite::Written
        );
        assert_eq!(
            files.write_device_state(&device_state),
            StorageJsonWrite::Written
        );

        assert_eq!(files.read_plan_snapshot(), StorageJsonRead::Value(snapshot));
        assert_eq!(
            files.read_resource_index(),
            StorageJsonRead::Value(resource_index)
        );
        assert_eq!(
            files.read_display_state(),
            StorageJsonRead::Value(display_state)
        );
        assert_eq!(
            files.read_device_state(),
            StorageJsonRead::Value(device_state)
        );
    }

    #[test]
    fn app_json_helpers_round_trip_domain_types() {
        let snapshot = snapshot(&["a"]);
        let resource_index = index(&["a"]);
        let display_state = DisplayState {
            plan_id: Some(7),
            plan_content_hash: Some("hash-v1".to_string()),
            date: Some(date("2026-06-08")),
            image_sha256: Some("a".to_string()),
            image_index: 0,
            caption: Some("caption".to_string()),
            refreshed_at_unix_secs: Some(100),
        };
        let mut device_state = PersistentDeviceState::default();
        device_state.set_current_display(&display_state);

        assert_eq!(
            plan_snapshot_from_json(&plan_snapshot_to_json(&snapshot).unwrap()).unwrap(),
            snapshot
        );
        assert_eq!(
            resource_index_from_json(&resource_index_to_json(&resource_index).unwrap()).unwrap(),
            resource_index
        );
        assert_eq!(
            display_state_from_json(&display_state_to_json(&display_state).unwrap()).unwrap(),
            display_state
        );
        assert_eq!(
            device_state_from_json(&device_state_to_json(&device_state).unwrap()).unwrap(),
            device_state
        );
    }
}
