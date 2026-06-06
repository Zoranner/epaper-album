use crate::power::BatteryStatus;

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum RefreshReason {
    FirstBoot,
    DisplayItemChanged,
    ErrorPage,
    Manual,
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct DisplayState {
    pub plan_date: Option<String>,
    pub image_sha256: Option<String>,
    pub image_position: Option<u16>,
    pub caption: Option<String>,
}

impl DisplayState {
    pub const fn empty() -> Self {
        Self {
            plan_date: None,
            image_sha256: None,
            image_position: None,
            caption: None,
        }
    }
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct CacheState {
    pub resource_count: u32,
    pub used_bytes: u64,
    pub free_bytes: u64,
}

impl CacheState {
    pub const fn empty() -> Self {
        Self {
            resource_count: 0,
            used_bytes: 0,
            free_bytes: 0,
        }
    }
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct DeviceRunState {
    pub last_successful_sync_epoch_seconds: Option<u64>,
    pub last_sync_error: Option<String>,
    pub current_plan_version: Option<String>,
    pub current_display: DisplayState,
    pub next_wakeup_epoch_seconds: Option<u64>,
    pub cache: CacheState,
    pub battery: BatteryStatus,
    pub last_refresh_reason: Option<RefreshReason>,
}

impl DeviceRunState {
    pub const fn new(battery: BatteryStatus) -> Self {
        Self {
            last_successful_sync_epoch_seconds: None,
            last_sync_error: None,
            current_plan_version: None,
            current_display: DisplayState::empty(),
            next_wakeup_epoch_seconds: None,
            cache: CacheState::empty(),
            battery,
            last_refresh_reason: None,
        }
    }
}
