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
}

impl PersistentDeviceState {
    pub fn new() -> Self {
        Self {
            date: None,
            image: None,
            caption: None,
        }
    }

    pub fn from_display(date: LocalDate, plan: &Plan) -> Self {
        Self::from_photo(date, plan.image.clone(), plan.caption.clone())
    }

    pub fn from_photo(date: LocalDate, image: String, caption: String) -> Self {
        Self {
            date: Some(date),
            image: Some(image),
            caption: Some(caption),
        }
    }

    pub fn from_page(date: LocalDate) -> Self {
        Self {
            date: Some(date),
            image: None,
            caption: None,
        }
    }

    pub fn set_display(&mut self, date: LocalDate, plan: &Plan) {
        *self = Self::from_display(date, plan);
    }

    pub fn set_photo(&mut self, date: LocalDate, image: String, caption: String) {
        *self = Self::from_photo(date, image, caption);
    }

    pub fn set_page(&mut self, date: LocalDate) {
        *self = Self::from_page(date);
    }

    pub fn matches_display(&self, date: LocalDate, plan: &Plan) -> bool {
        self.date == Some(date)
            && self.image.as_deref() == Some(plan.image.as_str())
            && self.caption.as_deref() == Some(plan.caption.as_str())
    }
}

impl Default for PersistentDeviceState {
    fn default() -> Self {
        Self::new()
    }
}

#[derive(Clone, Debug, Default, Eq, PartialEq, Serialize, Deserialize)]
pub struct PersistentSyncState {
    #[serde(default)]
    pub date: Option<LocalDate>,
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
        }

        let wire = WireState::deserialize(deserializer)?;

        Ok(Self {
            date: wire.date,
            image: wire.image,
            caption: wire.caption,
        })
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn plan() -> Plan {
        Plan::fixed(LocalDate::parse("2026-06-08").unwrap(), "caption", "abc")
    }

    #[test]
    fn serializes_flat_persistent_state() {
        let state =
            PersistentDeviceState::from_display(LocalDate::parse("2026-06-13").unwrap(), &plan());

        let json = serde_json::to_string(&state).unwrap();

        assert!(json.contains("date"));
        assert!(json.contains("image"));
        assert!(json.contains("caption"));
        assert!(json.contains("2026-06-13"));
        assert!(!json.contains("current_display"));
        assert!(!json.contains("last_wake_reason"));
    }

    #[test]
    fn compares_state_with_screen_display() {
        let plan = plan();
        let state =
            PersistentDeviceState::from_display(LocalDate::parse("2026-06-13").unwrap(), &plan);

        assert!(state.matches_display(LocalDate::parse("2026-06-13").unwrap(), &plan));
    }

    #[test]
    fn display_state_uses_screen_date_independent_from_plan_date() {
        let plan = plan();
        let state =
            PersistentDeviceState::from_display(LocalDate::parse("2026-06-13").unwrap(), &plan);

        assert_eq!(state.date, Some(LocalDate::parse("2026-06-13").unwrap()));
        assert_eq!(state.image.as_deref(), Some("abc"));
        assert_eq!(state.caption.as_deref(), Some("caption"));
    }
}
