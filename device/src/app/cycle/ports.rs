use super::SyncErrorReport;
use crate::config::Config;
use crate::model::{LocalDate, Plan};
use crate::state::RefreshReason;
use std::fmt;

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct SyncRequest {
    pub config: Config,
    pub local_plans: Option<Vec<Plan>>,
    pub date: LocalDate,
    pub now_epoch_seconds: u64,
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct SyncResult {
    pub plans: Vec<Plan>,
    pub sprites: SpriteSet,
}

#[derive(Clone, Debug, Default, Eq, PartialEq)]
pub struct SpriteSet {
    pub caption: Option<String>,
    pub date: Option<String>,
}

pub trait DeviceCloudSync {
    type Error: fmt::Display;

    fn sync_resources(&mut self, request: SyncRequest) -> Result<SyncResult, Self::Error>;

    fn describe_error(&self, error: &Self::Error) -> SyncErrorReport {
        SyncErrorReport::from_display(error)
    }
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct DisplayRefreshRequest {
    pub plan: Plan,
    pub date: LocalDate,
    pub reason: RefreshReason,
    pub sprites: SpriteSet,
    pub now_epoch_seconds: u64,
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct ErrorRefreshRequest {
    pub title: String,
    pub message: String,
    pub hint: String,
    pub detail: String,
    pub now_epoch_seconds: u64,
}

pub trait DeviceDisplay {
    type Error: fmt::Display;

    fn refresh(&mut self, request: DisplayRefreshRequest) -> Result<(), Self::Error>;
    fn refresh_error_page(&mut self, request: ErrorRefreshRequest) -> Result<(), Self::Error>;
    fn has_image(&self, sha256: &str) -> bool;
}
