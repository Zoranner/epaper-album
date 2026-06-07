use crate::model::DisplayItem;
use serde::{Deserialize, Serialize};

#[derive(Clone, Copy, Debug, Eq, PartialEq, Serialize, Deserialize)]
pub enum WakeReason {
    Startup,
    Timer,
    ManualButton,
    External,
    Unknown,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq, Serialize, Deserialize)]
pub enum RefreshReason {
    FirstBoot,
    DisplayItemChanged,
    ErrorPage,
    Manual,
}

#[derive(Clone, Debug, Default, Eq, PartialEq, Serialize, Deserialize)]
pub struct PersistedDisplayRecord {
    #[serde(default)]
    pub plan_id: Option<i64>,
    #[serde(default)]
    pub plan_date: Option<String>,
    #[serde(default)]
    pub image_sha256: Option<String>,
    #[serde(default)]
    pub image_index: Option<usize>,
    #[serde(default)]
    pub caption: Option<String>,
}

impl PersistedDisplayRecord {
    pub fn from_display_item(item: &DisplayItem) -> Self {
        Self {
            plan_id: Some(item.plan_id),
            plan_date: Some(item.date.to_string()),
            image_sha256: Some(item.image_sha256.clone()),
            image_index: Some(item.image_index),
            caption: Some(item.caption.clone()),
        }
    }
}

#[derive(Clone, Debug, Default, Eq, PartialEq, Serialize, Deserialize)]
pub struct CacheState {
    #[serde(default)]
    pub resource_count: u32,
    #[serde(default)]
    pub used_bytes: u64,
    #[serde(default)]
    pub free_bytes: u64,
}

#[derive(Clone, Debug, Eq, PartialEq, Serialize, Deserialize)]
pub struct PersistentDeviceState {
    #[serde(default)]
    pub last_successful_sync_epoch_seconds: Option<u64>,
    #[serde(default)]
    pub last_sync_error: Option<String>,
    #[serde(default)]
    pub current_plan_content_hash: Option<String>,
    #[serde(default)]
    pub current_display: PersistedDisplayRecord,
    #[serde(default)]
    pub next_wakeup_epoch_seconds: Option<u64>,
    #[serde(default)]
    pub cache: CacheState,
    #[serde(default)]
    pub last_refresh_reason: Option<RefreshReason>,
    #[serde(default)]
    pub last_wake_reason: Option<WakeReason>,
}

impl PersistentDeviceState {
    pub const fn new() -> Self {
        Self {
            last_successful_sync_epoch_seconds: None,
            last_sync_error: None,
            current_plan_content_hash: None,
            current_display: PersistedDisplayRecord {
                plan_id: None,
                plan_date: None,
                image_sha256: None,
                image_index: None,
                caption: None,
            },
            next_wakeup_epoch_seconds: None,
            cache: CacheState {
                resource_count: 0,
                used_bytes: 0,
                free_bytes: 0,
            },
            last_refresh_reason: None,
            last_wake_reason: None,
        }
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

    #[test]
    fn serializes_persistent_state_without_model_display_state_name() {
        let state = PersistentDeviceState {
            current_display: PersistedDisplayRecord {
                plan_id: Some(7),
                plan_date: Some("2026-06-08".to_string()),
                image_sha256: Some("abc".to_string()),
                image_index: Some(1),
                caption: Some("caption".to_string()),
            },
            last_wake_reason: Some(WakeReason::ManualButton),
            ..PersistentDeviceState::default()
        };

        let json = serde_json::to_string(&state).unwrap();

        assert!(json.contains("current_display"));
        assert!(json.contains("plan_id"));
        assert!(json.contains("image_index"));
        assert!(json.contains("last_wake_reason"));
        assert!(!json.contains("DisplayState"));
    }

    #[test]
    fn builds_persisted_display_record_from_selected_item() {
        let item = DisplayItem {
            plan_id: 3,
            plan_content_hash: Some("hash".to_string()),
            date: crate::model::LocalDate::parse("2026-06-08").unwrap(),
            image_sha256: "abc".to_string(),
            image_index: 2,
            caption: "caption".to_string(),
        };

        let record = PersistedDisplayRecord::from_display_item(&item);

        assert_eq!(record.plan_id, Some(3));
        assert_eq!(record.plan_date.as_deref(), Some("2026-06-08"));
        assert_eq!(record.image_sha256.as_deref(), Some("abc"));
        assert_eq!(record.image_index, Some(2));
        assert_eq!(record.caption.as_deref(), Some("caption"));
    }
}
