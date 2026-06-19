use serde::de::{Error, Visitor};
use serde::{Deserialize, Deserializer, Serialize, Serializer};
use std::fmt;
use std::str::FromStr;

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct ApiResponse<T> {
    pub code: u16,
    pub message: String,
    pub data: T,
}

impl<T> ApiResponse<T> {
    pub fn ok(data: T) -> Self {
        Self {
            code: 0,
            message: "ok".to_string(),
            data,
        }
    }
}

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

#[derive(Debug, Clone, Default, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum PlanType {
    #[default]
    Fixed,
    Random,
}

fn is_default_plan_type(plan_type: &PlanType) -> bool {
    matches!(plan_type, PlanType::Fixed)
}

fn is_empty_tags(tags: &[String]) -> bool {
    tags.is_empty()
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Plan {
    pub date: LocalDate,
    #[serde(default)]
    pub caption: String,
    #[serde(default)]
    #[serde(skip_serializing_if = "is_default_plan_type")]
    #[serde(rename = "type")]
    pub plan_type: PlanType,
    #[serde(default)]
    pub image: String,
    #[serde(default)]
    #[serde(skip_serializing_if = "is_empty_tags")]
    pub tags: Vec<String>,
}

impl Plan {
    pub fn fixed(date: LocalDate, caption: impl Into<String>, image: impl Into<String>) -> Self {
        Self {
            date,
            caption: caption.into(),
            plan_type: PlanType::Fixed,
            image: image.into(),
            tags: Vec::new(),
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct SpriteMeta {
    pub sha256: String,
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
    fn serializes_api_response_with_plan_data() {
        let response = ApiResponse::ok(vec![Plan {
            date: LocalDate::parse("2026-06-06").unwrap(),
            caption: "caption".to_string(),
            plan_type: PlanType::Fixed,
            image: "hash".to_string(),
            tags: Vec::new(),
        }]);
        let json = serde_json::to_string(&response).unwrap();

        assert!(json.contains(r#""code":0"#));
        assert!(json.contains(r#""date":"2026-06-06""#));
        assert!(json.contains(r#""image":"hash""#));
    }

    #[test]
    fn rejects_unknown_plan_fields() {
        let json = r#"{"date":"2026-06-06","caption":"caption","image":"hash","extra":1}"#;

        assert!(serde_json::from_str::<Plan>(json).is_err());
    }

    #[test]
    fn plan_defaults_to_fixed_type_and_empty_tags() {
        let json = r#"{"date":"2026-06-06","caption":"caption","image":"hash"}"#;

        let plan: Plan = serde_json::from_str(json).unwrap();

        assert_eq!(plan.plan_type, PlanType::Fixed);
        assert!(plan.tags.is_empty());
    }
}
