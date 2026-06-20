use super::{
    DeviceCloudSync, DeviceCycleInput, DeviceDisplay, DisplayRefreshRequest, ErrorRefreshRequest,
    SyncRequest, SyncResult,
};
use crate::app::RunTrigger;
use crate::config::Config;
use crate::model::{LocalDate, Plan};
use crate::power::BatteryStatus;
use crate::state::{PersistentDeviceState, PersistentSyncState, WakeReason};
use std::fmt;

#[derive(Debug, Clone, Eq, PartialEq)]
pub(super) struct FakeError(pub(super) &'static str);

impl fmt::Display for FakeError {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter.write_str(self.0)
    }
}

#[derive(Default)]
pub(super) struct FakeSync {
    pub(super) result: Option<Result<SyncResult, FakeError>>,
    pub(super) requests: Vec<SyncRequest>,
}

impl DeviceCloudSync for FakeSync {
    type Error = FakeError;

    fn sync_resources(&mut self, request: SyncRequest) -> Result<SyncResult, Self::Error> {
        self.requests.push(request);
        self.result
            .take()
            .unwrap_or(Err(FakeError("unexpected sync")))
    }
}

#[derive(Default)]
pub(super) struct FakeDisplay {
    pub(super) images: Vec<String>,
    pub(super) result: Option<Result<(), FakeError>>,
    pub(super) requests: Vec<DisplayRefreshRequest>,
    pub(super) error_requests: Vec<ErrorRefreshRequest>,
}

impl DeviceDisplay for FakeDisplay {
    type Error = FakeError;

    fn refresh(&mut self, request: DisplayRefreshRequest) -> Result<(), Self::Error> {
        self.requests.push(request);
        self.result.take().unwrap_or(Ok(()))
    }

    fn refresh_error_page(&mut self, request: ErrorRefreshRequest) -> Result<(), Self::Error> {
        self.error_requests.push(request);
        self.result.take().unwrap_or(Ok(()))
    }

    fn has_image(&self, sha256: &str) -> bool {
        self.images.iter().any(|image| image == sha256)
    }
}

pub(super) fn date(value: &str) -> LocalDate {
    LocalDate::parse(value).unwrap()
}

pub(super) fn config() -> Config {
    Config {
        wifi_ssid: "wifi".to_string(),
        wifi_password: "password".to_string(),
        base_url: "https://example.com".to_string(),
        secret_key: "secret".to_string(),
    }
}

pub(super) fn plan(image: &str) -> Plan {
    Plan::fixed(date("2026-06-08"), "caption", image)
}

pub(super) fn input(plans: Option<Vec<Plan>>) -> DeviceCycleInput {
    DeviceCycleInput {
        config: Some(config()),
        plans,
        persistent_state: PersistentDeviceState::default(),
        persistent_state_loaded: false,
        sync_state: PersistentSyncState::default(),
        trigger: RunTrigger::Wake(WakeReason::Timer),
        now_epoch_seconds: 100,
        date: date("2026-06-08"),
        battery: BatteryStatus::unknown(),
    }
}
