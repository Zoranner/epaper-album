use crate::model::{DisplayItem, DisplayState, ResourceIndex};
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
    #[serde(default, alias = "plan_date")]
    pub date: Option<String>,
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
            date: Some(item.date.to_string()),
            image_sha256: Some(item.image_sha256.clone()),
            image_index: Some(item.image_index),
            caption: Some(item.caption.clone()),
        }
    }

    pub fn from_display_state(state: &DisplayState) -> Self {
        Self {
            plan_id: state.plan_id,
            date: state.date.map(|date| date.to_string()),
            image_sha256: state.image_sha256.clone(),
            image_index: Some(state.image_index),
            caption: state.caption.clone(),
        }
    }

    pub fn to_display_state(&self, plan_content_hash: Option<String>) -> DisplayState {
        DisplayState {
            plan_id: self.plan_id,
            plan_content_hash,
            date: self
                .date
                .as_deref()
                .and_then(|date| crate::model::LocalDate::parse(date).ok()),
            image_sha256: self.image_sha256.clone(),
            image_index: self.image_index.unwrap_or_default(),
            caption: self.caption.clone(),
            refreshed_at_unix_secs: None,
        }
    }
}

impl From<&DisplayItem> for PersistedDisplayRecord {
    fn from(item: &DisplayItem) -> Self {
        Self::from_display_item(item)
    }
}

impl From<&DisplayState> for PersistedDisplayRecord {
    fn from(state: &DisplayState) -> Self {
        Self::from_display_state(state)
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
    #[serde(default)]
    pub resources: ResourceIndex,
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
    pub fn new() -> Self {
        Self {
            last_successful_sync_epoch_seconds: None,
            last_sync_error: None,
            current_plan_content_hash: None,
            current_display: PersistedDisplayRecord {
                plan_id: None,
                date: None,
                image_sha256: None,
                image_index: None,
                caption: None,
            },
            next_wakeup_epoch_seconds: None,
            cache: CacheState {
                resource_count: 0,
                used_bytes: 0,
                free_bytes: 0,
                resources: ResourceIndex::default(),
            },
            last_refresh_reason: None,
            last_wake_reason: None,
        }
    }

    pub fn display_state(&self) -> DisplayState {
        self.current_display
            .to_display_state(self.current_plan_content_hash.clone())
    }

    pub fn set_current_display(&mut self, display_state: &DisplayState) {
        self.current_plan_content_hash = display_state.plan_content_hash.clone();
        self.current_display = PersistedDisplayRecord::from_display_state(display_state);
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
                date: Some("2026-06-08".to_string()),
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
        assert!(json.contains("date"));
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
        assert_eq!(record.date.as_deref(), Some("2026-06-08"));
        assert_eq!(record.image_sha256.as_deref(), Some("abc"));
        assert_eq!(record.image_index, Some(2));
        assert_eq!(record.caption.as_deref(), Some("caption"));
    }

    #[test]
    fn converts_between_model_display_state_and_persistent_record() {
        let display_state = DisplayState {
            plan_id: Some(9),
            plan_content_hash: Some("hash-v1".to_string()),
            date: Some(crate::model::LocalDate::parse("2026-06-08").unwrap()),
            image_sha256: Some("abc".to_string()),
            image_index: 4,
            caption: Some("caption".to_string()),
            refreshed_at_unix_secs: Some(10),
        };

        let record = PersistedDisplayRecord::from_display_state(&display_state);
        let restored = record.to_display_state(display_state.plan_content_hash.clone());

        assert_eq!(record.date.as_deref(), Some("2026-06-08"));
        assert_eq!(restored.plan_id, display_state.plan_id);
        assert_eq!(restored.plan_content_hash, display_state.plan_content_hash);
        assert_eq!(restored.date, display_state.date);
        assert_eq!(restored.image_sha256, display_state.image_sha256);
        assert_eq!(restored.image_index, display_state.image_index);
        assert_eq!(restored.caption, display_state.caption);
    }
}
