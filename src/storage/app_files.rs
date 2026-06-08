use crate::model::PlanSnapshot;
use crate::state::PersistentDeviceState;
use crate::storage::{
    read_json_file, to_json_string, write_json_file_atomic, StorageJsonRead, StorageJsonWrite,
    PLAN_PATH, STATE_PATH,
};
use std::path::{Path, PathBuf};

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct AppPaths {
    pub plan_snapshot: PathBuf,
    pub device_state: PathBuf,
}

impl AppPaths {
    pub fn new(plan_snapshot: impl Into<PathBuf>, device_state: impl Into<PathBuf>) -> Self {
        Self {
            plan_snapshot: plan_snapshot.into(),
            device_state: device_state.into(),
        }
    }
}

impl Default for AppPaths {
    fn default() -> Self {
        Self {
            plan_snapshot: PathBuf::from(PLAN_PATH),
            device_state: PathBuf::from(STATE_PATH),
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
    use crate::model::{CachedResource, DisplayState, LocalDate, PlanItem, ResourceIndex};

    fn date(value: &str) -> LocalDate {
        LocalDate::parse(value).unwrap()
    }

    fn snapshot(image_sha256: &str) -> PlanSnapshot {
        PlanSnapshot {
            content_hash: "hash-v1".to_string(),
            plans: vec![PlanItem {
                caption: "caption".to_string(),
                date: date("2026-06-08"),
                image_sha256: image_sha256.to_string(),
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
            temp_dir.path().join("plan.json"),
            temp_dir.path().join("state.json"),
        ));
        let snapshot = snapshot("a");
        let resource_index = index(&["a"]);
        let display_state = DisplayState {
            plan_content_hash: Some("hash-v1".to_string()),
            date: Some(date("2026-06-08")),
            image_sha256: Some("a".to_string()),
            caption: Some("caption".to_string()),
            refreshed_at_unix_secs: Some(100),
        };
        let mut device_state = PersistentDeviceState::default();
        device_state.set_current_display(&display_state);
        device_state.cache.resources = resource_index;

        assert_eq!(
            files.write_plan_snapshot(&snapshot),
            StorageJsonWrite::Written
        );
        assert_eq!(
            files.write_device_state(&device_state),
            StorageJsonWrite::Written
        );

        assert_eq!(files.read_plan_snapshot(), StorageJsonRead::Value(snapshot));
        assert_eq!(
            files.read_device_state(),
            StorageJsonRead::Value(device_state)
        );
    }

    #[test]
    fn app_json_helpers_round_trip_domain_types() {
        let snapshot = snapshot("a");
        let resource_index = index(&["a"]);
        let display_state = DisplayState {
            plan_content_hash: Some("hash-v1".to_string()),
            date: Some(date("2026-06-08")),
            image_sha256: Some("a".to_string()),
            caption: Some("caption".to_string()),
            refreshed_at_unix_secs: Some(100),
        };
        let mut device_state = PersistentDeviceState::default();
        device_state.set_current_display(&display_state);
        device_state.cache.resources = resource_index.clone();

        assert_eq!(
            plan_snapshot_from_json(&plan_snapshot_to_json(&snapshot).unwrap()).unwrap(),
            snapshot
        );
        assert_eq!(
            device_state_from_json(&device_state_to_json(&device_state).unwrap()).unwrap(),
            device_state
        );
    }
}
