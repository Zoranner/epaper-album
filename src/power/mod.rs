#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum ChargeState {
    Unknown,
    Discharging,
    Charging,
    Full,
}

pub const DAILY_SECONDS: u64 = 24 * 60 * 60;
pub const HOURLY_RUN_INTERVAL_SECONDS: u64 = 60 * 60;
pub const BEIJING_UTC_OFFSET_SECONDS: u64 = 8 * 60 * 60;

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct BatteryStatus {
    pub millivolts: u16,
    pub percent: Option<u8>,
    pub charge_state: ChargeState,
    pub low_battery: bool,
}

impl BatteryStatus {
    pub const fn new(
        millivolts: u16,
        percent: Option<u8>,
        charge_state: ChargeState,
        low_battery: bool,
    ) -> Self {
        Self {
            millivolts,
            percent,
            charge_state,
            low_battery,
        }
    }

    pub const fn unknown() -> Self {
        Self {
            millivolts: 0,
            percent: None,
            charge_state: ChargeState::Unknown,
            low_battery: false,
        }
    }

    pub const fn externally_powered(&self) -> bool {
        matches!(self.charge_state, ChargeState::Charging | ChargeState::Full)
    }

    pub const fn effective_low_battery(&self) -> bool {
        self.low_battery && !self.externally_powered()
    }
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum PowerProfile {
    Battery,
    External,
    LowBattery,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum WakeProbe {
    Timer,
    Button,
    External,
    Ulp,
    Unknown,
    Other(u32),
}

impl WakeProbe {
    pub const fn label(self) -> &'static str {
        match self {
            Self::Timer => "timer",
            Self::Button => "button",
            Self::External => "external",
            Self::Ulp => "ulp",
            Self::Unknown => "unknown",
            Self::Other(_) => "other",
        }
    }
}

impl PowerProfile {
    pub const fn run_interval_seconds(self) -> u64 {
        match self {
            Self::Battery | Self::External | Self::LowBattery => HOURLY_RUN_INTERVAL_SECONDS,
        }
    }
}

impl From<&BatteryStatus> for PowerProfile {
    fn from(status: &BatteryStatus) -> Self {
        if status.effective_low_battery() {
            return Self::LowBattery;
        }

        match status.charge_state {
            ChargeState::Charging | ChargeState::Full => Self::External,
            ChargeState::Unknown | ChargeState::Discharging => Self::Battery,
        }
    }
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct LowBatteryPolicy {
    pub stop_cloud_sync: bool,
    pub minimum_percent: Option<u8>,
    pub minimum_millivolts: Option<u16>,
}

impl Default for LowBatteryPolicy {
    fn default() -> Self {
        Self {
            stop_cloud_sync: true,
            minimum_percent: None,
            minimum_millivolts: None,
        }
    }
}

impl LowBatteryPolicy {
    pub fn is_low_battery(&self, battery: &BatteryStatus) -> bool {
        if battery.externally_powered() {
            return false;
        }

        if battery.low_battery {
            return true;
        }

        if let (Some(percent), Some(minimum_percent)) = (battery.percent, self.minimum_percent) {
            if percent <= minimum_percent {
                return true;
            }
        }

        if let Some(minimum_millivolts) = self.minimum_millivolts {
            if battery.millivolts != 0 && battery.millivolts <= minimum_millivolts {
                return true;
            }
        }

        false
    }
}

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
    fn power_profile_follows_battery_charge_state() {
        assert_eq!(
            PowerProfile::from(&BatteryStatus::new(
                3_900,
                Some(50),
                ChargeState::Discharging,
                false
            )),
            PowerProfile::Battery
        );
        assert_eq!(
            PowerProfile::from(&BatteryStatus::new(
                4_000,
                Some(70),
                ChargeState::Charging,
                false
            )),
            PowerProfile::External
        );
        assert_eq!(
            PowerProfile::from(&BatteryStatus::new(
                4_200,
                Some(100),
                ChargeState::Full,
                false
            )),
            PowerProfile::External
        );
        assert_eq!(
            PowerProfile::from(&BatteryStatus::new(
                3_500,
                Some(10),
                ChargeState::Charging,
                true
            )),
            PowerProfile::External
        );
    }

    #[test]
    fn externally_powered_status_is_not_effective_low_battery() {
        for charge_state in [ChargeState::Charging, ChargeState::Full] {
            let battery = BatteryStatus::new(0, Some(0), charge_state, true);

            assert!(battery.externally_powered());
            assert!(!battery.effective_low_battery());
            assert!(!LowBatteryPolicy::default().is_low_battery(&battery));
            assert_ne!(PowerProfile::from(&battery), PowerProfile::LowBattery);
        }
    }

    #[test]
    fn discharging_status_keeps_low_battery_policy() {
        let explicit_low = BatteryStatus::new(0, Some(80), ChargeState::Discharging, true);

        assert!(explicit_low.effective_low_battery());
        assert!(LowBatteryPolicy::default().is_low_battery(&explicit_low));
        assert_eq!(PowerProfile::from(&explicit_low), PowerProfile::LowBattery);
    }

    #[test]
    fn default_low_battery_policy_ignores_uncalibrated_percent() {
        let battery = BatteryStatus::new(0, Some(0), ChargeState::Unknown, false);

        assert!(!LowBatteryPolicy::default().is_low_battery(&battery));
    }

    #[test]
    fn percent_threshold_only_applies_when_explicitly_configured() {
        let policy = LowBatteryPolicy {
            minimum_percent: Some(15),
            ..LowBatteryPolicy::default()
        };
        let battery = BatteryStatus::new(0, Some(0), ChargeState::Discharging, false);

        assert!(policy.is_low_battery(&battery));
    }

    #[test]
    fn power_profile_run_interval_matches_power_state() {
        assert_eq!(
            PowerProfile::Battery.run_interval_seconds(),
            HOURLY_RUN_INTERVAL_SECONDS
        );
        assert_eq!(
            PowerProfile::External.run_interval_seconds(),
            HOURLY_RUN_INTERVAL_SECONDS
        );
        assert_eq!(
            PowerProfile::LowBattery.run_interval_seconds(),
            HOURLY_RUN_INTERVAL_SECONDS
        );
    }

    #[test]
    fn external_profile_wakes_every_hour() {
        assert_eq!(
            PowerProfile::External.run_interval_seconds(),
            HOURLY_RUN_INTERVAL_SECONDS
        );
    }

    #[test]
    fn external_profile_uses_hourly_interval() {
        assert_eq!(
            PowerProfile::External.run_interval_seconds(),
            HOURLY_RUN_INTERVAL_SECONDS
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

    #[test]
    fn wake_probe_has_external_label() {
        assert_eq!(WakeProbe::External.label(), "external");
    }
}

#[cfg(target_os = "espidf")]
pub mod espidf {
    use core::time::Duration;

    pub use super::WakeProbe;
    use esp_idf_hal::gpio::PinId;
    use esp_idf_hal::reset::WakeupReason;
    use esp_idf_hal::sleep::{DeepSleep, RtcWakeLevel, RtcWakeupPins};
    use esp_idf_sys::{
        esp, esp_restart, esp_sleep_get_ext1_wakeup_status, gpio_config, gpio_config_t,
        gpio_get_level, gpio_int_type_t_GPIO_INTR_DISABLE, gpio_mode_t_GPIO_MODE_INPUT,
        gpio_num_t_GPIO_NUM_21, gpio_num_t_GPIO_NUM_4, gpio_pulldown_t_GPIO_PULLDOWN_DISABLE,
        gpio_pullup_t_GPIO_PULLUP_ENABLE,
    };

    pub const SELF_TEST_TIMER_WAKE_SECONDS: u64 = 20;
    pub const SELF_TEST_KEY_GPIO: i32 = gpio_num_t_GPIO_NUM_4;
    const PMIC_IRQ_GPIO: i32 = gpio_num_t_GPIO_NUM_21;
    const SELF_TEST_KEY_HOLD_MS: u32 = 5_000;
    const SELF_TEST_KEY_SAMPLE_MS: u32 = 30;

    struct PmicIrqWakePin;

    impl RtcWakeupPins for PmicIrqWakePin {
        type Iterator<'a> = core::iter::Once<PinId>;

        fn iter(&self) -> Self::Iterator<'_> {
            core::iter::once(PMIC_IRQ_GPIO as PinId)
        }
    }

    impl From<WakeupReason> for WakeProbe {
        fn from(value: WakeupReason) -> Self {
            match value {
                WakeupReason::Timer => Self::Timer,
                WakeupReason::Button => Self::Button,
                WakeupReason::ULP => Self::Ulp,
                WakeupReason::Unknown => Self::Unknown,
                WakeupReason::Other(value) => Self::Other(value),
            }
        }
    }

    pub fn wake_probe() -> WakeProbe {
        match WakeupReason::get() {
            WakeupReason::Button if external_wakeup_gpio_mask() & pmic_irq_gpio_mask() != 0 => {
                WakeProbe::External
            }
            wakeup_reason => wakeup_reason.into(),
        }
    }

    pub fn self_test_key_long_pressed() -> bool {
        self_test_key_pressed_for(SELF_TEST_KEY_HOLD_MS)
    }

    pub fn self_test_key_clicked() -> bool {
        if configure_self_test_key().is_err() {
            return false;
        }

        let was_pressed = self_test_key_pressed();
        esp_idf_hal::delay::FreeRtos::delay_ms(120);
        was_pressed && !self_test_key_pressed()
    }

    pub fn self_test_key_pressed_for(milliseconds: u32) -> bool {
        if configure_self_test_key().is_err() {
            return false;
        }

        if !self_test_key_pressed() {
            return false;
        }

        let samples = milliseconds / SELF_TEST_KEY_SAMPLE_MS;
        for _ in 0..samples {
            esp_idf_hal::delay::FreeRtos::delay_ms(SELF_TEST_KEY_SAMPLE_MS);
            if !self_test_key_pressed() {
                return false;
            }
        }

        true
    }

    fn configure_self_test_key() -> Result<(), esp_idf_sys::EspError> {
        let config = gpio_config_t {
            pin_bit_mask: 1u64 << SELF_TEST_KEY_GPIO,
            mode: gpio_mode_t_GPIO_MODE_INPUT,
            pull_up_en: gpio_pullup_t_GPIO_PULLUP_ENABLE,
            pull_down_en: gpio_pulldown_t_GPIO_PULLDOWN_DISABLE,
            intr_type: gpio_int_type_t_GPIO_INTR_DISABLE,
        };

        // SAFETY: This configures the fixed KEY input after ESP-IDF initialization in the
        // platform layer. The pin mask is derived from the board GPIO constant.
        unsafe { esp!(gpio_config(&config)) }
    }

    fn self_test_key_pressed() -> bool {
        // SAFETY: The fixed KEY GPIO is configured as an input by configure_self_test_key.
        unsafe { gpio_get_level(SELF_TEST_KEY_GPIO) == 0 }
    }

    pub fn restart_now() -> ! {
        // SAFETY: Restart is an intentional terminal hardware-control path for KEY/self-test flow.
        unsafe { esp_restart() }
    }

    pub fn enter_deep_sleep_until(
        next_run_epoch_seconds: u64,
    ) -> Result<(), esp_idf_sys::EspError> {
        let seconds = seconds_until(next_run_epoch_seconds);
        if seconds == 0 {
            // SAFETY: Restart is used instead of returning when the requested wake time is due.
            unsafe { esp_restart() }
        }

        let sleep = DeepSleep::new()?.wakeup_on_timer(Duration::from_secs(seconds))?;
        configure_pmic_irq_deep_sleep_wakeup()?;
        sleep.enter()
    }

    pub fn restart_at(next_run_epoch_seconds: u64) -> ! {
        restart_at_with_poll(next_run_epoch_seconds, || false)
    }

    pub fn restart_at_with_poll(next_run_epoch_seconds: u64, mut poll: impl FnMut() -> bool) -> ! {
        if seconds_until(next_run_epoch_seconds) == 0 {
            // SAFETY: Restart is used instead of returning when the requested wake time is due.
            unsafe { esp_restart() }
        }

        loop {
            for _ in 0..2_000 {
                if seconds_until(next_run_epoch_seconds) == 0 {
                    // SAFETY: Restart is used instead of returning when the requested wake time is due.
                    unsafe { esp_restart() }
                }
                if poll() {
                    // SAFETY: Restart is an intentional terminal path after a platform poll request.
                    unsafe { esp_restart() }
                }
                esp_idf_hal::delay::FreeRtos::delay_ms(30);
            }
        }
    }

    fn seconds_until(next_run_epoch_seconds: u64) -> u64 {
        let now_epoch_seconds = chrono::Utc::now().timestamp().max(0) as u64;
        next_run_epoch_seconds.saturating_sub(now_epoch_seconds)
    }

    fn configure_pmic_irq_deep_sleep_wakeup() -> Result<(), esp_idf_sys::EspError> {
        configure_pmic_irq_gpio_input()?;
        esp_idf_hal::sleep::rtc::configure(PmicIrqWakePin, RtcWakeLevel::AllLow)
    }

    fn configure_pmic_irq_gpio_input() -> Result<(), esp_idf_sys::EspError> {
        let config = gpio_config_t {
            pin_bit_mask: pmic_irq_gpio_mask(),
            mode: gpio_mode_t_GPIO_MODE_INPUT,
            pull_up_en: gpio_pullup_t_GPIO_PULLUP_ENABLE,
            pull_down_en: gpio_pulldown_t_GPIO_PULLDOWN_DISABLE,
            intr_type: gpio_int_type_t_GPIO_INTR_DISABLE,
        };

        // SAFETY: This configures the fixed PMIC IRQ RTC-capable input for deep-sleep wake.
        // The mask is derived from the board GPIO constant.
        unsafe { esp!(gpio_config(&config)) }
    }

    fn external_wakeup_gpio_mask() -> u64 {
        // SAFETY: Reading the ESP-IDF wakeup status is side-effect free and scoped to wake probing.
        unsafe { esp_sleep_get_ext1_wakeup_status() }
    }

    const fn pmic_irq_gpio_mask() -> u64 {
        1u64 << PMIC_IRQ_GPIO
    }
}
