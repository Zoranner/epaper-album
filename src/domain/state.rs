use crate::model::{LocalDate, Plan};
use serde::{Deserialize, Serialize};

#[derive(Clone, Copy, Debug, Eq, PartialEq, Serialize, Deserialize)]
pub enum WakeReason {
    Startup,
    Timer,
    Button,
    External,
    Unknown,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq, Serialize, Deserialize)]
pub enum RefreshReason {
    FirstBoot,
    PlanChanged,
    OverlayChanged,
    NoticeChanged,
    ErrorPage,
}

#[derive(Clone, Debug, Default, Eq, PartialEq, Serialize, Deserialize)]
pub struct PersistedDisplay {
    #[serde(default)]
    pub date: Option<LocalDate>,
    #[serde(default)]
    pub image: Option<String>,
    #[serde(default)]
    pub caption: Option<String>,
}

impl PersistedDisplay {
    pub fn from_plan(plan: &Plan) -> Self {
        Self {
            date: Some(plan.date),
            image: Some(plan.image.clone()),
            caption: Some(plan.caption.clone()),
        }
    }

    pub fn matches_plan(&self, plan: &Plan) -> bool {
        self.date == Some(plan.date)
            && self.image.as_deref() == Some(plan.image.as_str())
            && self.caption.as_deref() == Some(plan.caption.as_str())
    }
}

#[derive(Clone, Debug, Eq, PartialEq, Serialize, Deserialize)]
pub struct PersistentDeviceState {
    #[serde(default)]
    pub last_successful_sync_epoch_seconds: Option<u64>,
    #[serde(default)]
    pub last_sync_error: Option<String>,
    #[serde(default)]
    pub current_display: PersistedDisplay,
    #[serde(default)]
    pub next_wakeup_epoch_seconds: Option<u64>,
    #[serde(default)]
    pub last_refresh_reason: Option<RefreshReason>,
    #[serde(default)]
    pub last_wake_reason: Option<WakeReason>,
}

impl PersistentDeviceState {
    pub fn new() -> Self {
        Self {
            last_successful_sync_epoch_seconds: None,
            last_sync_error: None,
            current_display: PersistedDisplay::default(),
            next_wakeup_epoch_seconds: None,
            last_refresh_reason: None,
            last_wake_reason: None,
        }
    }

    pub fn set_current_display(&mut self, plan: &Plan) {
        self.current_display = PersistedDisplay::from_plan(plan);
    }
}

impl Default for PersistentDeviceState {
    fn default() -> Self {
        Self::new()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn plan() -> Plan {
        Plan {
            date: LocalDate::parse("2026-06-08").unwrap(),
            image: "abc".to_string(),
            caption: "caption".to_string(),
        }
    }

    #[test]
    fn serializes_persistent_state_with_current_display() {
        let state = PersistentDeviceState {
            current_display: PersistedDisplay::from_plan(&plan()),
            last_wake_reason: Some(WakeReason::Button),
            ..PersistentDeviceState::default()
        };

        let json = serde_json::to_string(&state).unwrap();

        assert!(json.contains("current_display"));
        assert!(json.contains("2026-06-08"));
        assert!(json.contains("last_wake_reason"));
    }

    #[test]
    fn compares_persisted_display_with_plan() {
        let plan = plan();
        let display = PersistedDisplay::from_plan(&plan);

        assert!(display.matches_plan(&plan));
    }
}
