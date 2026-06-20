use super::battery::PowerProfile;

pub const DAILY_SECONDS: u64 = 24 * 60 * 60;
pub const HOURLY_RUN_INTERVAL_SECONDS: u64 = 60 * 60;
pub const BEIJING_UTC_OFFSET_SECONDS: u64 = 8 * 60 * 60;

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct NextRunPlan {
    pub next_run_epoch_seconds: u64,
    pub wait_seconds: u64,
}

impl NextRunPlan {
    pub const fn new(now_epoch_seconds: u64, next_run_epoch_seconds: u64) -> Self {
        let next_run_epoch_seconds = if next_run_epoch_seconds < now_epoch_seconds {
            now_epoch_seconds
        } else {
            next_run_epoch_seconds
        };

        Self {
            next_run_epoch_seconds,
            wait_seconds: next_run_epoch_seconds - now_epoch_seconds,
        }
    }
}

pub fn next_run_plan(
    now_epoch_seconds: u64,
    next_power_interval_epoch_seconds: u64,
    next_plan_date_change_epoch_seconds: Option<u64>,
    carousel_seconds: Option<u32>,
) -> NextRunPlan {
    let carousel_epoch_seconds =
        carousel_seconds.map(|seconds| now_epoch_seconds.saturating_add(u64::from(seconds)));

    let earliest_candidate_epoch_seconds = [
        Some(next_power_interval_epoch_seconds),
        next_plan_date_change_epoch_seconds,
        carousel_epoch_seconds,
    ]
    .into_iter()
    .flatten()
    .min()
    .unwrap_or(now_epoch_seconds);
    NextRunPlan::new(now_epoch_seconds, earliest_candidate_epoch_seconds)
}

pub fn next_power_run_epoch_seconds(now_epoch_seconds: u64, power_profile: PowerProfile) -> u64 {
    match power_profile {
        PowerProfile::External | PowerProfile::Battery | PowerProfile::LowBattery => {
            next_beijing_hour_epoch_seconds(now_epoch_seconds)
        }
    }
}

pub fn next_beijing_hour_epoch_seconds(now_epoch_seconds: u64) -> u64 {
    next_local_period_epoch_seconds(now_epoch_seconds, HOURLY_RUN_INTERVAL_SECONDS)
}

pub fn local_date_start_epoch_seconds(date: crate::model::LocalDate) -> u64 {
    let days = days_from_civil(date.year as i32, u32::from(date.month), u32::from(date.day));
    let local_epoch_seconds = i128::from(days) * i128::from(DAILY_SECONDS);
    local_epoch_seconds
        .saturating_sub(i128::from(BEIJING_UTC_OFFSET_SECONDS))
        .max(0) as u64
}

fn next_local_period_epoch_seconds(now_epoch_seconds: u64, period_seconds: u64) -> u64 {
    let local_epoch_seconds = now_epoch_seconds.saturating_add(BEIJING_UTC_OFFSET_SECONDS);
    local_epoch_seconds
        .saturating_div(period_seconds)
        .saturating_add(1)
        .saturating_mul(period_seconds)
        .saturating_sub(BEIJING_UTC_OFFSET_SECONDS)
}

fn days_from_civil(year: i32, month: u32, day: u32) -> i64 {
    let year = year - i32::from(month <= 2);
    let era = if year >= 0 { year } else { year - 399 } / 400;
    let year_of_era = (year - era * 400) as u32;
    let month_prime = if month > 2 { month - 3 } else { month + 9 };
    let day_of_year = (153 * month_prime + 2) / 5 + day - 1;
    let day_of_era = year_of_era * 365 + year_of_era / 4 - year_of_era / 100 + day_of_year;
    i64::from(era) * 146_097 + i64::from(day_of_era) - 719_468
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn next_run_plan_uses_power_interval_when_it_is_earliest() {
        assert_eq!(
            next_run_plan(1_000, 1_100, Some(1_200), Some(300)),
            NextRunPlan {
                next_run_epoch_seconds: 1_100,
                wait_seconds: 100,
            }
        );
    }

    #[test]
    fn next_run_plan_uses_plan_date_change_when_it_is_earliest() {
        assert_eq!(
            next_run_plan(1_000, 1_300, Some(1_120), Some(200)),
            NextRunPlan {
                next_run_epoch_seconds: 1_120,
                wait_seconds: 120,
            }
        );
    }

    #[test]
    fn next_run_plan_uses_carousel_interval_when_it_is_earliest() {
        assert_eq!(
            next_run_plan(1_000, 1_300, Some(1_250), Some(60)),
            NextRunPlan {
                next_run_epoch_seconds: 1_060,
                wait_seconds: 60,
            }
        );
    }

    #[test]
    fn next_run_plan_runs_immediately_for_past_time() {
        assert_eq!(
            next_run_plan(1_000, 900, Some(1_250), Some(60)),
            NextRunPlan {
                next_run_epoch_seconds: 1_000,
                wait_seconds: 0,
            }
        );
    }

    #[test]
    fn external_power_runs_at_next_beijing_hour() {
        assert_eq!(
            next_power_run_epoch_seconds(1_781_273_730, PowerProfile::External),
            1_781_276_400
        );
    }

    #[test]
    fn battery_power_runs_at_next_beijing_hour_from_previous_hour() {
        assert_eq!(
            next_power_run_epoch_seconds(1_781_276_400, PowerProfile::Battery),
            1_781_280_000
        );
    }

    #[test]
    fn battery_power_runs_at_next_beijing_hour() {
        assert_eq!(
            next_power_run_epoch_seconds(1_781_276_400, PowerProfile::Battery),
            1_781_280_000
        );
        assert_eq!(
            next_power_run_epoch_seconds(1_781_280_000, PowerProfile::Battery),
            1_781_283_600
        );
    }

    #[test]
    fn battery_power_at_beijing_midnight_runs_next_day() {
        assert_eq!(
            next_power_run_epoch_seconds(1_781_280_000, PowerProfile::Battery),
            1_781_283_600
        );
    }

    #[test]
    fn local_date_start_uses_fixed_beijing_midnight() {
        let date = crate::model::LocalDate::parse("2026-06-13").unwrap();

        assert_eq!(local_date_start_epoch_seconds(date), 1_781_280_000);
    }
}
