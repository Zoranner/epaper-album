use crate::model::{LocalDate, Plan};
use crate::render::RenderNotice;
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

#[derive(Clone, Debug, Eq, PartialEq, Serialize)]
pub struct PersistentDeviceState {
    #[serde(default)]
    pub date: Option<LocalDate>,
    #[serde(default)]
    pub image: Option<String>,
    #[serde(default)]
    pub caption: Option<String>,
    #[serde(default)]
    pub notice: Option<RenderNotice>,
}

impl PersistentDeviceState {
    pub fn new() -> Self {
        Self {
            date: None,
            image: None,
            caption: None,
            notice: None,
        }
    }

    pub fn from_plan(plan: &Plan, notice: Option<RenderNotice>) -> Self {
        Self {
            date: Some(plan.date),
            image: Some(plan.image.clone()),
            caption: Some(plan.caption.clone()),
            notice,
        }
    }

    pub fn set(&mut self, plan: &Plan, notice: Option<RenderNotice>) {
        *self = Self::from_plan(plan, notice);
    }

    pub fn matches_plan(&self, plan: &Plan) -> bool {
        self.date == Some(plan.date)
            && self.image.as_deref() == Some(plan.image.as_str())
            && self.caption.as_deref() == Some(plan.caption.as_str())
    }

    pub fn to_plan(&self) -> Option<Plan> {
        Some(Plan {
            date: self.date?,
            image: self.image.clone()?,
            caption: self.caption.clone().unwrap_or_default(),
        })
    }
}

impl Default for PersistentDeviceState {
    fn default() -> Self {
        Self::new()
    }
}

impl<'de> Deserialize<'de> for PersistentDeviceState {
    fn deserialize<D>(deserializer: D) -> Result<Self, D::Error>
    where
        D: serde::Deserializer<'de>,
    {
        #[derive(Deserialize)]
        struct WireState {
            #[serde(default)]
            date: Option<LocalDate>,
            #[serde(default)]
            image: Option<String>,
            #[serde(default)]
            caption: Option<String>,
            #[serde(default)]
            notice: Option<RenderNotice>,
            #[serde(default)]
            current_display: Option<WireDisplay>,
            #[serde(default)]
            current_notice: Option<RenderNotice>,
        }

        #[derive(Deserialize)]
        struct WireDisplay {
            #[serde(default)]
            date: Option<LocalDate>,
            #[serde(default)]
            image: Option<String>,
            #[serde(default)]
            caption: Option<String>,
        }

        let wire = WireState::deserialize(deserializer)?;
        let old_display = wire.current_display;

        Ok(Self {
            date: wire
                .date
                .or_else(|| old_display.as_ref().and_then(|display| display.date)),
            image: wire.image.or_else(|| {
                old_display
                    .as_ref()
                    .and_then(|display| display.image.clone())
            }),
            caption: wire.caption.or_else(|| {
                old_display
                    .as_ref()
                    .and_then(|display| display.caption.clone())
            }),
            notice: wire.notice.or(wire.current_notice),
        })
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
    fn serializes_flat_persistent_state() {
        let state = PersistentDeviceState::from_plan(&plan(), Some(RenderNotice::LowBattery));

        let json = serde_json::to_string(&state).unwrap();

        assert!(json.contains("date"));
        assert!(json.contains("image"));
        assert!(json.contains("caption"));
        assert!(json.contains("notice"));
        assert!(json.contains("2026-06-08"));
        assert!(!json.contains("current_display"));
        assert!(!json.contains("last_wake_reason"));
    }

    #[test]
    fn compares_state_with_plan() {
        let plan = plan();
        let state = PersistentDeviceState::from_plan(&plan, None);

        assert!(state.matches_plan(&plan));
    }

    #[test]
    fn reads_legacy_current_display_state() {
        let state: PersistentDeviceState = serde_json::from_str(
            r#"{
                "current_display": {
                    "date": "2026-06-08",
                    "image": "abc",
                    "caption": "caption"
                },
                "current_notice": "LowBattery",
                "last_successful_sync_epoch_seconds": 100
            }"#,
        )
        .unwrap();

        assert_eq!(state.date, Some(LocalDate::parse("2026-06-08").unwrap()));
        assert_eq!(state.image.as_deref(), Some("abc"));
        assert_eq!(state.caption.as_deref(), Some("caption"));
        assert_eq!(state.notice, Some(RenderNotice::LowBattery));
    }
}
