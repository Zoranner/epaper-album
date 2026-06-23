use core::time::Duration;

pub use super::WakeProbe;
use esp_idf_hal::gpio::PinId;
use esp_idf_hal::reset::WakeupReason;
use esp_idf_hal::sleep::{DeepSleep, RtcWakeLevel, RtcWakeupPins};
use esp_idf_sys::{
    esp, esp_restart, esp_sleep_get_ext1_wakeup_status, gpio_config, gpio_config_t, gpio_get_level,
    gpio_int_type_t_GPIO_INTR_DISABLE, gpio_mode_t_GPIO_MODE_INPUT, gpio_num_t_GPIO_NUM_21,
    gpio_num_t_GPIO_NUM_4, gpio_pulldown_t_GPIO_PULLDOWN_DISABLE, gpio_pullup_t_GPIO_PULLUP_ENABLE,
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

    self_test_key_pressed()
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

pub fn enter_deep_sleep_until(next_run_epoch_seconds: u64) -> Result<(), esp_idf_sys::EspError> {
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
