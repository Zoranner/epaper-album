use crate::config::Config;
use crate::model::LocalDate;
use crate::power::{BatteryStatus, PowerProfile};
use crate::state::PersistentSyncState;

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum SyncAction {
    Fetch,
    Skip,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum SyncCause {
    External,
    Daily,
    Done,
    LowBattery,
    MissingConfig,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct SyncDecision {
    pub action: SyncAction,
    pub cause: SyncCause,
}

pub fn decide_sync(
    config: Option<&Config>,
    battery: &BatteryStatus,
    sync_state: &PersistentSyncState,
    date: LocalDate,
) -> SyncDecision {
    if battery.effective_low_battery() {
        return SyncDecision {
            action: SyncAction::Skip,
            cause: SyncCause::LowBattery,
        };
    }

    if config.is_none_or(|config| !config.has_required_values()) {
        return SyncDecision {
            action: SyncAction::Skip,
            cause: SyncCause::MissingConfig,
        };
    }

    if matches!(PowerProfile::from(battery), PowerProfile::External) {
        return SyncDecision {
            action: SyncAction::Fetch,
            cause: SyncCause::External,
        };
    }

    if sync_state.date != Some(date) {
        return SyncDecision {
            action: SyncAction::Fetch,
            cause: SyncCause::Daily,
        };
    }

    SyncDecision {
        action: SyncAction::Skip,
        cause: SyncCause::Done,
    }
}
