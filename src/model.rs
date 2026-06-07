use serde::de::{Error, Visitor};
use serde::{Deserialize, Deserializer, Serialize, Serializer};
use std::collections::BTreeSet;
use std::fmt;
use std::str::FromStr;

#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub struct LocalDate {
    pub year: u16,
    pub month: u8,
    pub day: u8,
}

impl LocalDate {
    pub fn new(year: u16, month: u8, day: u8) -> Option<Self> {
        let date = Self { year, month, day };
        date.is_valid().then_some(date)
    }

    pub fn parse(value: &str) -> Result<Self, DateParseError> {
        value.parse()
    }

    pub fn next_day(self) -> Self {
        let days_in_month = Self::days_in_month(self.year, self.month);
        if self.day < days_in_month {
            return Self {
                day: self.day + 1,
                ..self
            };
        }

        if self.month < 12 {
            return Self {
                month: self.month + 1,
                day: 1,
                ..self
            };
        }

        Self {
            year: self.year + 1,
            month: 1,
            day: 1,
        }
    }

    fn is_valid(self) -> bool {
        self.year > 0
            && (1..=12).contains(&self.month)
            && (1..=Self::days_in_month(self.year, self.month)).contains(&self.day)
    }

    fn days_in_month(year: u16, month: u8) -> u8 {
        match month {
            1 | 3 | 5 | 7 | 8 | 10 | 12 => 31,
            4 | 6 | 9 | 11 => 30,
            2 if Self::is_leap_year(year) => 29,
            2 => 28,
            _ => 0,
        }
    }

    fn is_leap_year(year: u16) -> bool {
        year.is_multiple_of(4) && (!year.is_multiple_of(100) || year.is_multiple_of(400))
    }
}

impl fmt::Display for LocalDate {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(
            formatter,
            "{:04}-{:02}-{:02}",
            self.year, self.month, self.day
        )
    }
}

impl FromStr for LocalDate {
    type Err = DateParseError;

    fn from_str(value: &str) -> Result<Self, Self::Err> {
        let mut parts = value.split('-');
        let year = parts
            .next()
            .ok_or(DateParseError)?
            .parse::<u16>()
            .map_err(|_| DateParseError)?;
        let month = parts
            .next()
            .ok_or(DateParseError)?
            .parse::<u8>()
            .map_err(|_| DateParseError)?;
        let day = parts
            .next()
            .ok_or(DateParseError)?
            .parse::<u8>()
            .map_err(|_| DateParseError)?;

        if parts.next().is_some() {
            return Err(DateParseError);
        }

        Self::new(year, month, day).ok_or(DateParseError)
    }
}

impl Serialize for LocalDate {
    fn serialize<S>(&self, serializer: S) -> Result<S::Ok, S::Error>
    where
        S: Serializer,
    {
        serializer.serialize_str(&self.to_string())
    }
}

impl<'de> Deserialize<'de> for LocalDate {
    fn deserialize<D>(deserializer: D) -> Result<Self, D::Error>
    where
        D: Deserializer<'de>,
    {
        struct LocalDateVisitor;

        impl Visitor<'_> for LocalDateVisitor {
            type Value = LocalDate;

            fn expecting(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
                formatter.write_str("a date string formatted as YYYY-MM-DD")
            }

            fn visit_str<E>(self, value: &str) -> Result<Self::Value, E>
            where
                E: Error,
            {
                LocalDate::parse(value).map_err(E::custom)
            }
        }

        deserializer.deserialize_str(LocalDateVisitor)
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct DateParseError;

impl fmt::Display for DateParseError {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter.write_str("invalid local date, expected YYYY-MM-DD")
    }
}

impl std::error::Error for DateParseError {}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct ServerPlanResponse {
    pub code: u16,
    pub message: String,
    #[serde(default)]
    pub data: Vec<PlanItem>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct PlanSnapshot {
    pub content_hash: String,
    #[serde(default)]
    pub plans: Vec<PlanItem>,
}

impl PlanSnapshot {
    pub fn referenced_resources(&self) -> BTreeSet<String> {
        self.plans
            .iter()
            .flat_map(|plan| plan.images.iter().cloned())
            .collect()
    }
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct PlanItem {
    pub id: i64,
    pub start: LocalDate,
    pub end: LocalDate,
    #[serde(default)]
    pub caption: String,
    #[serde(default)]
    pub images: Vec<String>,
}

impl PlanItem {
    pub fn contains_date(&self, date: LocalDate) -> bool {
        self.start <= date && date <= self.end
    }

    pub fn image_at_slot(&self, rotation_slot: u64) -> Option<(usize, &str)> {
        if self.images.is_empty() {
            return None;
        }

        let image_index = (rotation_slot % self.images.len() as u64) as usize;
        self.images
            .get(image_index)
            .map(|sha256| (image_index, sha256.as_str()))
    }
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, Default)]
pub struct ResourceIndex {
    #[serde(default)]
    pub resources: Vec<CachedResource>,
}

impl ResourceIndex {
    pub fn upsert(&mut self, resource: CachedResource) {
        if let Some(existing) = self
            .resources
            .iter_mut()
            .find(|existing| existing.sha256 == resource.sha256)
        {
            existing.byte_size = resource.byte_size;
            existing.last_used_at_unix_secs = resource.last_used_at_unix_secs;
            return;
        }

        self.resources.push(resource);
    }

    pub fn update(&mut self, resource: CachedResource) {
        self.upsert(resource);
    }

    pub fn touch(&mut self, sha256: &str, last_used_at_unix_secs: u64) -> bool {
        if let Some(resource) = self
            .resources
            .iter_mut()
            .find(|resource| resource.sha256 == sha256)
        {
            resource.last_used_at_unix_secs = last_used_at_unix_secs;
            return true;
        }

        false
    }

    pub fn mark_used(&mut self, sha256: &str, last_used_at_unix_secs: u64) -> bool {
        self.touch(sha256, last_used_at_unix_secs)
    }

    pub fn contains(&self, sha256: &str) -> bool {
        self.resources
            .iter()
            .any(|resource| resource.sha256 == sha256)
    }

    pub fn resource(&self, sha256: &str) -> Option<&CachedResource> {
        self.resources
            .iter()
            .find(|resource| resource.sha256 == sha256)
    }

    pub fn known_resources(&self) -> BTreeSet<String> {
        self.resources
            .iter()
            .map(|resource| resource.sha256.clone())
            .collect()
    }
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct CachedResource {
    pub sha256: String,
    pub byte_size: u64,
    #[serde(default)]
    pub last_used_at_unix_secs: u64,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, Default)]
pub struct DisplayState {
    #[serde(default)]
    pub plan_id: Option<i64>,
    #[serde(default)]
    pub plan_content_hash: Option<String>,
    #[serde(default)]
    pub date: Option<LocalDate>,
    #[serde(default)]
    pub image_sha256: Option<String>,
    #[serde(default)]
    pub image_index: usize,
    #[serde(default)]
    pub caption: Option<String>,
    #[serde(default)]
    pub refreshed_at_unix_secs: Option<u64>,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct DisplayItem {
    pub plan_id: i64,
    pub plan_content_hash: Option<String>,
    pub date: LocalDate,
    pub image_sha256: String,
    pub image_index: usize,
    pub caption: String,
}

impl From<&DisplayItem> for DisplayState {
    fn from(item: &DisplayItem) -> Self {
        Self {
            plan_id: Some(item.plan_id),
            plan_content_hash: item.plan_content_hash.clone(),
            date: Some(item.date),
            image_sha256: Some(item.image_sha256.clone()),
            image_index: item.image_index,
            caption: Some(item.caption.clone()),
            refreshed_at_unix_secs: None,
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn parses_and_orders_local_dates() {
        let date = LocalDate::parse("2026-06-06").unwrap();

        assert_eq!(date.to_string(), "2026-06-06");
        assert!(date > LocalDate::parse("2026-06-05").unwrap());
    }

    #[test]
    fn rejects_invalid_dates() {
        assert!(LocalDate::parse("2026-02-29").is_err());
        assert!(LocalDate::parse("2024-02-29").is_ok());
        assert!(LocalDate::parse("2026-13-01").is_err());
    }

    #[test]
    fn advances_to_next_day_across_month_and_year() {
        assert_eq!(
            LocalDate::parse("2026-06-30").unwrap().next_day(),
            LocalDate::parse("2026-07-01").unwrap()
        );
        assert_eq!(
            LocalDate::parse("2026-12-31").unwrap().next_day(),
            LocalDate::parse("2027-01-01").unwrap()
        );
    }

    #[test]
    fn plan_matches_inclusive_date_range() {
        let plan = PlanItem {
            id: 7,
            start: LocalDate::parse("2026-06-06").unwrap(),
            end: LocalDate::parse("2026-06-08").unwrap(),
            caption: "caption".to_string(),
            images: vec!["hash".to_string()],
        };

        assert!(plan.contains_date(LocalDate::parse("2026-06-06").unwrap()));
        assert!(plan.contains_date(LocalDate::parse("2026-06-08").unwrap()));
        assert!(!plan.contains_date(LocalDate::parse("2026-06-09").unwrap()));
    }

    #[test]
    fn resource_index_inserts_resource() {
        let mut index = ResourceIndex::default();

        index.upsert(CachedResource {
            sha256: "hash-a".to_string(),
            byte_size: 128,
            last_used_at_unix_secs: 10,
        });

        assert!(index.contains("hash-a"));
        assert_eq!(
            index.known_resources(),
            BTreeSet::from(["hash-a".to_string()])
        );
        assert_eq!(
            index.resource("hash-a"),
            Some(&CachedResource {
                sha256: "hash-a".to_string(),
                byte_size: 128,
                last_used_at_unix_secs: 10,
            })
        );
    }

    #[test]
    fn resource_index_updates_existing_resource() {
        let mut index = ResourceIndex {
            resources: vec![CachedResource {
                sha256: "hash-a".to_string(),
                byte_size: 128,
                last_used_at_unix_secs: 10,
            }],
        };

        index.upsert(CachedResource {
            sha256: "hash-a".to_string(),
            byte_size: 256,
            last_used_at_unix_secs: 20,
        });

        assert_eq!(index.resources.len(), 1);
        assert_eq!(
            index.resource("hash-a"),
            Some(&CachedResource {
                sha256: "hash-a".to_string(),
                byte_size: 256,
                last_used_at_unix_secs: 20,
            })
        );
    }

    #[test]
    fn resource_index_touch_updates_existing_resource() {
        let mut index = ResourceIndex {
            resources: vec![CachedResource {
                sha256: "hash-a".to_string(),
                byte_size: 128,
                last_used_at_unix_secs: 10,
            }],
        };

        assert!(index.touch("hash-a", 30));

        assert_eq!(index.resource("hash-a").unwrap().byte_size, 128);
        assert_eq!(index.resource("hash-a").unwrap().last_used_at_unix_secs, 30);
    }

    #[test]
    fn resource_index_touch_reports_missing_resource() {
        let mut index = ResourceIndex {
            resources: vec![CachedResource {
                sha256: "hash-a".to_string(),
                byte_size: 128,
                last_used_at_unix_secs: 10,
            }],
        };

        assert!(!index.touch("hash-missing", 30));

        assert_eq!(index.resource("hash-a").unwrap().last_used_at_unix_secs, 10);
    }

    #[test]
    fn converts_selected_display_item_to_display_state() {
        let item = DisplayItem {
            plan_id: 3,
            plan_content_hash: Some("hash-v1".to_string()),
            date: LocalDate::parse("2026-06-08").unwrap(),
            image_sha256: "abc".to_string(),
            image_index: 2,
            caption: "caption".to_string(),
        };

        let state = DisplayState::from(&item);

        assert_eq!(state.plan_id, Some(3));
        assert_eq!(state.plan_content_hash.as_deref(), Some("hash-v1"));
        assert_eq!(state.date, Some(LocalDate::parse("2026-06-08").unwrap()));
        assert_eq!(state.image_sha256.as_deref(), Some("abc"));
        assert_eq!(state.image_index, 2);
        assert_eq!(state.caption.as_deref(), Some("caption"));
    }
}
