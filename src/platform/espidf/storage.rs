use super::EspDeviceRunOutcome;
use crate::config::{Config, CONFIG_PATH};
use crate::device_runtime::DeviceCycleResult;
use crate::storage::{
    read_json_file_mounted, read_text_file_mounted, write_json_file_atomic_mounted,
    StorageJsonRead, StorageJsonWrite, StorageRead, PLAN_PATH, STATE_PATH, SYNC_PATH,
};

pub fn read_config_mounted() -> Option<Config> {
    match read_text_file_mounted(CONFIG_PATH) {
        StorageRead::Text(content) => match toml::from_str::<Config>(&content) {
            Ok(config) if config.has_required_values() => Some(config),
            Ok(_) | Err(_) => None,
        },
        StorageRead::Missing | StorageRead::MountError | StorageRead::ReadError => None,
    }
}

pub fn read_optional_json_mounted<T>(path: &str) -> Option<T>
where
    T: serde::de::DeserializeOwned,
{
    match read_json_file_mounted(path) {
        StorageJsonRead::Value(value) => Some(value),
        StorageJsonRead::Missing
        | StorageJsonRead::MountError
        | StorageJsonRead::ReadError
        | StorageJsonRead::ParseError => None,
    }
}

pub fn write_cycle_files(cycle: &DeviceCycleResult) -> Result<(), EspDeviceRunOutcome> {
    if let Some(plans) = &cycle.plans {
        write_json_checked(PLAN_PATH, plans)?;
    }
    write_json_checked(STATE_PATH, &cycle.persistent_state)?;
    write_json_checked(SYNC_PATH, &cycle.sync_state)?;
    Ok(())
}

fn write_json_checked<T>(path: &str, value: &T) -> Result<(), EspDeviceRunOutcome>
where
    T: serde::Serialize,
{
    match write_json_file_atomic_mounted(path, value) {
        StorageJsonWrite::Written => Ok(()),
        StorageJsonWrite::SerializeError
        | StorageJsonWrite::MountError
        | StorageJsonWrite::WriteError => Err(EspDeviceRunOutcome::StateWriteError),
    }
}
