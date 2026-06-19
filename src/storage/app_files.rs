use crate::model::Plan;
use crate::state::{PersistentDeviceState, PersistentSyncState};
use crate::storage::{
    read_json_file, to_json_string, write_json_file_atomic, StorageJsonRead, StorageJsonWrite,
    PLAN_PATH, STATE_PATH, SYNC_PATH,
};
use std::path::{Path, PathBuf};

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct AppPaths {
    pub plans: PathBuf,
    pub device_state: PathBuf,
    pub sync_state: PathBuf,
}

impl AppPaths {
    pub fn new(
        plans: impl Into<PathBuf>,
        device_state: impl Into<PathBuf>,
        sync_state: impl Into<PathBuf>,
    ) -> Self {
        Self {
            plans: plans.into(),
            device_state: device_state.into(),
            sync_state: sync_state.into(),
        }
    }
}

impl Default for AppPaths {
    fn default() -> Self {
        Self {
            plans: PathBuf::from(PLAN_PATH),
            device_state: PathBuf::from(STATE_PATH),
            sync_state: PathBuf::from(SYNC_PATH),
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

    pub fn read_plans(&self) -> StorageJsonRead<Vec<Plan>> {
        read_plans_file(&self.paths.plans)
    }

    pub fn write_plans(&self, plans: &[Plan]) -> StorageJsonWrite {
        write_plans_file(&self.paths.plans, plans)
    }

    pub fn read_device_state(&self) -> StorageJsonRead<PersistentDeviceState> {
        read_device_state_file(&self.paths.device_state)
    }

    pub fn write_device_state(&self, state: &PersistentDeviceState) -> StorageJsonWrite {
        write_device_state_file(&self.paths.device_state, state)
    }

    pub fn read_sync_state(&self) -> StorageJsonRead<PersistentSyncState> {
        read_sync_state_file(&self.paths.sync_state)
    }

    pub fn write_sync_state(&self, state: &PersistentSyncState) -> StorageJsonWrite {
        write_sync_state_file(&self.paths.sync_state, state)
    }
}

impl Default for AppFiles {
    fn default() -> Self {
        Self::new(AppPaths::default())
    }
}

pub fn plans_from_json(content: &str) -> Result<Vec<Plan>, serde_json::Error> {
    crate::storage::parse_json_str(content)
}

pub fn plans_to_json(plans: &[Plan]) -> Result<String, serde_json::Error> {
    to_json_string(&plans)
}

pub fn device_state_from_json(content: &str) -> Result<PersistentDeviceState, serde_json::Error> {
    crate::storage::parse_json_str(content)
}

pub fn device_state_to_json(state: &PersistentDeviceState) -> Result<String, serde_json::Error> {
    to_json_string(state)
}

pub fn sync_state_from_json(content: &str) -> Result<PersistentSyncState, serde_json::Error> {
    crate::storage::parse_json_str(content)
}

pub fn sync_state_to_json(state: &PersistentSyncState) -> Result<String, serde_json::Error> {
    to_json_string(state)
}

pub fn read_plans_file(path: impl AsRef<Path>) -> StorageJsonRead<Vec<Plan>> {
    read_json_file(path)
}

pub fn write_plans_file(path: impl AsRef<Path>, plans: &[Plan]) -> StorageJsonWrite {
    write_json_file_atomic(path, &plans)
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

pub fn read_sync_state_file(path: impl AsRef<Path>) -> StorageJsonRead<PersistentSyncState> {
    read_json_file(path)
}

pub fn write_sync_state_file(
    path: impl AsRef<Path>,
    state: &PersistentSyncState,
) -> StorageJsonWrite {
    write_json_file_atomic(path, state)
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::model::LocalDate;

    fn date(value: &str) -> LocalDate {
        LocalDate::parse(value).unwrap()
    }

    fn plan(image: &str) -> Plan {
        Plan::fixed(date("2026-06-08"), "caption", image)
    }

    #[test]
    fn app_files_round_trip_persistent_json() {
        let temp_dir = tempfile::tempdir().unwrap();
        let files = AppFiles::new(AppPaths::new(
            temp_dir.path().join("plan.json"),
            temp_dir.path().join("state.json"),
            temp_dir.path().join("sync.json"),
        ));
        let plans = vec![plan("a")];
        let device_state = PersistentDeviceState::from_display(date("2026-06-13"), &plans[0]);
        let sync_state = PersistentSyncState {
            date: Some(date("2026-06-08")),
        };

        assert_eq!(files.write_plans(&plans), StorageJsonWrite::Written);
        assert_eq!(
            files.write_device_state(&device_state),
            StorageJsonWrite::Written
        );
        assert_eq!(
            files.write_sync_state(&sync_state),
            StorageJsonWrite::Written
        );

        assert_eq!(files.read_plans(), StorageJsonRead::Value(plans));
        assert_eq!(
            files.read_device_state(),
            StorageJsonRead::Value(device_state)
        );
        assert_eq!(files.read_sync_state(), StorageJsonRead::Value(sync_state));
    }

    #[test]
    fn app_json_helpers_round_trip_domain_types() {
        let plans = vec![plan("a")];
        let device_state = PersistentDeviceState::from_display(date("2026-06-13"), &plans[0]);
        let sync_state = PersistentSyncState {
            date: Some(date("2026-06-08")),
        };

        assert_eq!(
            plans_from_json(&plans_to_json(&plans).unwrap()).unwrap(),
            plans
        );
        assert_eq!(
            device_state_from_json(&device_state_to_json(&device_state).unwrap()).unwrap(),
            device_state
        );
        assert_eq!(
            sync_state_from_json(&sync_state_to_json(&sync_state).unwrap()).unwrap(),
            sync_state
        );
    }
}
