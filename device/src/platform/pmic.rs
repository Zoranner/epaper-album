#[cfg(any(target_os = "espidf", test))]
const AXP2101_WAKEUP_IRQ_PIN_TO_LOW: u8 = 1 << 4;
#[cfg(any(target_os = "espidf", test))]
const AXP2101_VBUS_INSERT_IRQ_ENABLE: u8 = 1 << 7;

#[cfg(any(target_os = "espidf", test))]
const fn no_irq_bits(_: u8) -> u8 {
    0
}

#[cfg(any(target_os = "espidf", test))]
const fn irq_enable2_bits_for_policy(policy: crate::power::DeepSleepWakePolicy) -> u8 {
    match policy {
        crate::power::DeepSleepWakePolicy::TimerOnly => 0,
        crate::power::DeepSleepWakePolicy::TimerAndPmicIrq => AXP2101_VBUS_INSERT_IRQ_ENABLE,
    }
}

#[cfg(any(target_os = "espidf", test))]
const fn disable_irq_pin_wakeup_bits(value: u8) -> u8 {
    value & !AXP2101_WAKEUP_IRQ_PIN_TO_LOW
}

#[cfg(any(target_os = "espidf", test))]
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct PmicStatusSummary {
    pub battery_connected: bool,
    pub vbus_good: bool,
    pub battery_current_direction: u8,
    pub charge_step: u8,
}

#[cfg(any(target_os = "espidf", test))]
pub fn status_summary(status1: u8, status2: u8) -> PmicStatusSummary {
    PmicStatusSummary {
        battery_connected: status1 & (1 << 3) != 0,
        vbus_good: status1 & (1 << 5) != 0,
        battery_current_direction: (status2 >> 5) & 0x03,
        charge_step: status2 & 0x07,
    }
}

#[cfg(any(target_os = "espidf", test))]
pub fn battery_status_from_registers(
    status1: u8,
    status2: u8,
    percent: Option<u8>,
) -> crate::power::BatteryStatus {
    let summary = status_summary(status1, status2);
    crate::power::BatteryStatus::with_external_power(
        0,
        percent,
        charge_state_from_summary(summary),
        summary.vbus_good,
        false,
    )
}

#[cfg(any(target_os = "espidf", test))]
fn charge_state_from_summary(summary: PmicStatusSummary) -> crate::power::ChargeState {
    use crate::power::ChargeState;

    match (
        summary.battery_current_direction,
        summary.charge_step,
        summary.vbus_good,
        summary.battery_connected,
    ) {
        (0x01, _, true, true) => ChargeState::Charging,
        (_, 0x04, true, true) => ChargeState::Full,
        (0x02, _, _, true) => ChargeState::Discharging,
        (0x00, _, true, _) => ChargeState::Full,
        (_, _, true, false) => ChargeState::Full,
        _ => ChargeState::Unknown,
    }
}

#[cfg(target_os = "espidf")]
pub mod espidf {
    use super::{
        disable_irq_pin_wakeup_bits, irq_enable2_bits_for_policy, no_irq_bits,
        AXP2101_WAKEUP_IRQ_PIN_TO_LOW,
    };
    use crate::power::BatteryStatus;
    use crate::power::DeepSleepWakePolicy;
    use esp_idf_hal::i2c::{I2cConfig, I2cDriver};
    use esp_idf_hal::units::FromValueType;
    use esp_idf_sys::TickType_t;

    const AXP2101_ADDRESS: u8 = 0x34;
    const AXP2101_CHIP_ID: u8 = 0x4A;
    const REG_STATUS1: u8 = 0x00;
    const REG_STATUS2: u8 = 0x01;
    const REG_CHIP_ID: u8 = 0x03;
    const REG_SLEEP_WAKEUP_CTRL: u8 = 0x26;
    const REG_IRQ_ENABLE1: u8 = 0x40;
    const REG_IRQ_ENABLE2: u8 = 0x41;
    const REG_IRQ_ENABLE3: u8 = 0x42;
    const REG_IRQ_STATUS1: u8 = 0x48;
    const REG_IRQ_STATUS2: u8 = 0x49;
    const REG_IRQ_STATUS3: u8 = 0x4A;
    const REG_DC_ONOFF: u8 = 0x80;
    const REG_DC1_VOLTAGE: u8 = 0x82;
    const REG_LDO_ONOFF0: u8 = 0x90;
    const REG_ALDO1_VOLTAGE: u8 = 0x92;
    const REG_ALDO2_VOLTAGE: u8 = 0x93;
    const REG_ALDO3_VOLTAGE: u8 = 0x94;
    const REG_ALDO4_VOLTAGE: u8 = 0x95;
    const REG_BAT_PERCENT_DATA: u8 = 0xA4;
    const DC1_3300MV: u8 = ((3300u16 - 1500) / 100) as u8;
    const ALDO_3300MV: u8 = ((3300u16 - 500) / 100) as u8;
    const I2C_TIMEOUT_TICKS: TickType_t = 1000;

    #[derive(Clone, Copy, Debug, Eq, PartialEq)]
    pub struct PmicProbe {
        pub chip_id: u8,
        pub status1: u8,
        pub status2: u8,
        pub irq: PmicIrqSnapshot,
        pub dc_onoff: u8,
        pub ldo_onoff0: u8,
        pub battery: BatteryStatus,
    }

    #[derive(Clone, Copy, Debug, Eq, PartialEq)]
    pub struct PmicIrqSnapshot {
        pub enable1_before: u8,
        pub enable2_before: u8,
        pub enable3_before: u8,
        pub status1_before_clear: u8,
        pub status2_before_clear: u8,
        pub status3_before_clear: u8,
        pub enable1_after: u8,
        pub enable2_after: u8,
        pub enable3_after: u8,
        pub sleep_wakeup_ctrl_before: u8,
        pub sleep_wakeup_ctrl_after: u8,
    }

    pub struct PmicController {
        i2c: I2cDriver<'static>,
    }

    impl PmicController {
        pub fn new(
            i2c0: esp_idf_hal::i2c::I2C0<'static>,
            sda: esp_idf_hal::gpio::Gpio47<'static>,
            scl: esp_idf_hal::gpio::Gpio48<'static>,
        ) -> Result<Self, esp_idf_sys::EspError> {
            let config = I2cConfig::new().baudrate(100.kHz().into());
            Ok(Self {
                i2c: I2cDriver::new(i2c0, sda, scl, &config)?,
            })
        }

        pub fn init_for_photo_painter(&mut self) -> Result<PmicProbe, esp_idf_sys::EspError> {
            let chip_id = read_register(&mut self.i2c, REG_CHIP_ID)?;

            write_register(&mut self.i2c, REG_DC1_VOLTAGE, DC1_3300MV)?;
            write_register(&mut self.i2c, REG_ALDO1_VOLTAGE, ALDO_3300MV)?;
            write_register(&mut self.i2c, REG_ALDO2_VOLTAGE, ALDO_3300MV)?;
            write_register(&mut self.i2c, REG_ALDO3_VOLTAGE, ALDO_3300MV)?;
            write_register(&mut self.i2c, REG_ALDO4_VOLTAGE, ALDO_3300MV)?;

            let dc_onoff = read_register(&mut self.i2c, REG_DC_ONOFF)? | 0x01;
            write_register(&mut self.i2c, REG_DC_ONOFF, dc_onoff)?;

            let ldo_onoff0 = read_register(&mut self.i2c, REG_LDO_ONOFF0)? | 0x0F;
            write_register(&mut self.i2c, REG_LDO_ONOFF0, ldo_onoff0)?;

            let (battery, status1, status2) = read_battery_status(&mut self.i2c)?;
            let irq = configure_pmic_irq_state(&mut self.i2c, DeepSleepWakePolicy::TimerOnly)?;

            Ok(PmicProbe {
                chip_id,
                status1,
                status2,
                irq,
                dc_onoff: read_register(&mut self.i2c, REG_DC_ONOFF)?,
                ldo_onoff0: read_register(&mut self.i2c, REG_LDO_ONOFF0)?,
                battery,
            })
        }

        pub fn prepare_for_deep_sleep(
            &mut self,
            wake_policy: DeepSleepWakePolicy,
        ) -> Result<PmicSleepProbe, esp_idf_sys::EspError> {
            let irq = configure_pmic_irq_state(&mut self.i2c, wake_policy)?;
            Ok(PmicSleepProbe {
                wake_policy,
                irq,
                dc_onoff: read_register(&mut self.i2c, REG_DC_ONOFF)?,
                ldo_onoff0: read_register(&mut self.i2c, REG_LDO_ONOFF0)?,
            })
        }
    }

    pub fn init_axp2101_for_photo_painter(
        i2c0: esp_idf_hal::i2c::I2C0<'static>,
        sda: esp_idf_hal::gpio::Gpio47<'static>,
        scl: esp_idf_hal::gpio::Gpio48<'static>,
    ) -> Result<PmicProbe, esp_idf_sys::EspError> {
        let mut controller = PmicController::new(i2c0, sda, scl)?;
        controller.init_for_photo_painter()
    }

    pub fn chip_id_is_axp2101(probe: PmicProbe) -> bool {
        probe.chip_id == AXP2101_CHIP_ID
    }

    pub fn prepare_axp2101_for_deep_sleep(
        i2c0: esp_idf_hal::i2c::I2C0<'static>,
        sda: esp_idf_hal::gpio::Gpio47<'static>,
        scl: esp_idf_hal::gpio::Gpio48<'static>,
    ) -> Result<PmicSleepProbe, esp_idf_sys::EspError> {
        let mut controller = PmicController::new(i2c0, sda, scl)?;
        controller.prepare_for_deep_sleep(DeepSleepWakePolicy::TimerOnly)
    }

    #[derive(Clone, Copy, Debug, Eq, PartialEq)]
    pub struct PmicSleepProbe {
        pub wake_policy: DeepSleepWakePolicy,
        pub irq: PmicIrqSnapshot,
        pub dc_onoff: u8,
        pub ldo_onoff0: u8,
    }

    fn configure_pmic_irq_state(
        i2c: &mut I2cDriver<'_>,
        wake_policy: DeepSleepWakePolicy,
    ) -> Result<PmicIrqSnapshot, esp_idf_sys::EspError> {
        let enable1_before = read_register(i2c, REG_IRQ_ENABLE1)?;
        let enable2_before = read_register(i2c, REG_IRQ_ENABLE2)?;
        let enable3_before = read_register(i2c, REG_IRQ_ENABLE3)?;
        let status1_before_clear = read_register(i2c, REG_IRQ_STATUS1)?;
        let status2_before_clear = read_register(i2c, REG_IRQ_STATUS2)?;
        let status3_before_clear = read_register(i2c, REG_IRQ_STATUS3)?;
        let sleep_wakeup_ctrl_before = read_register(i2c, REG_SLEEP_WAKEUP_CTRL)?;

        clear_irq_status(i2c)?;

        write_register(i2c, REG_IRQ_ENABLE1, no_irq_bits(enable1_before))?;
        write_register(
            i2c,
            REG_IRQ_ENABLE2,
            irq_enable2_bits_for_policy(wake_policy),
        )?;
        write_register(i2c, REG_IRQ_ENABLE3, no_irq_bits(enable3_before))?;
        if wake_policy.uses_pmic_irq() {
            update_register(i2c, REG_SLEEP_WAKEUP_CTRL, |value| {
                value | AXP2101_WAKEUP_IRQ_PIN_TO_LOW
            })?;
        } else {
            update_register(i2c, REG_SLEEP_WAKEUP_CTRL, disable_irq_pin_wakeup_bits)?;
        }

        Ok(PmicIrqSnapshot {
            enable1_before,
            enable2_before,
            enable3_before,
            status1_before_clear,
            status2_before_clear,
            status3_before_clear,
            enable1_after: read_register(i2c, REG_IRQ_ENABLE1)?,
            enable2_after: read_register(i2c, REG_IRQ_ENABLE2)?,
            enable3_after: read_register(i2c, REG_IRQ_ENABLE3)?,
            sleep_wakeup_ctrl_before,
            sleep_wakeup_ctrl_after: read_register(i2c, REG_SLEEP_WAKEUP_CTRL)?,
        })
    }

    fn clear_irq_status(i2c: &mut I2cDriver<'_>) -> Result<(), esp_idf_sys::EspError> {
        for register in [REG_IRQ_STATUS1, REG_IRQ_STATUS2, REG_IRQ_STATUS3] {
            write_register(i2c, register, 0xFF)?;
        }

        Ok(())
    }

    fn read_battery_status(
        i2c: &mut I2cDriver<'_>,
    ) -> Result<(BatteryStatus, u8, u8), esp_idf_sys::EspError> {
        let status1 = read_register(i2c, REG_STATUS1)?;
        let status2 = read_register(i2c, REG_STATUS2)?;
        let percent = read_battery_percent(i2c)?;
        Ok(read_battery_status_from_registers(
            status1, status2, percent,
        ))
    }

    fn read_battery_status_from_registers(
        status1: u8,
        status2: u8,
        percent: Option<u8>,
    ) -> (BatteryStatus, u8, u8) {
        (
            super::battery_status_from_registers(status1, status2, percent),
            status1,
            status2,
        )
    }

    fn read_battery_percent(i2c: &mut I2cDriver<'_>) -> Result<Option<u8>, esp_idf_sys::EspError> {
        let raw = read_register(i2c, REG_BAT_PERCENT_DATA)?;
        Ok((raw <= 100).then_some(raw))
    }

    fn read_register(i2c: &mut I2cDriver<'_>, register: u8) -> Result<u8, esp_idf_sys::EspError> {
        let mut value = [0u8; 1];
        i2c.write_read(AXP2101_ADDRESS, &[register], &mut value, I2C_TIMEOUT_TICKS)?;
        Ok(value[0])
    }

    fn write_register(
        i2c: &mut I2cDriver<'_>,
        register: u8,
        value: u8,
    ) -> Result<(), esp_idf_sys::EspError> {
        i2c.write(AXP2101_ADDRESS, &[register, value], I2C_TIMEOUT_TICKS)
    }

    fn update_register(
        i2c: &mut I2cDriver<'_>,
        register: u8,
        update: impl FnOnce(u8) -> u8,
    ) -> Result<(), esp_idf_sys::EspError> {
        let value = read_register(i2c, register)?;
        write_register(i2c, register, update(value))
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::power::ChargeState;

    #[test]
    fn battery_sleep_masks_pmic_irqs() {
        assert_eq!(no_irq_bits(0xFF), 0x00);
        assert_eq!(
            irq_enable2_bits_for_policy(crate::power::DeepSleepWakePolicy::TimerOnly),
            0x00
        );
    }

    #[test]
    fn disables_irq_pin_low_wakeup_without_clearing_existing_wakeup_bits() {
        assert_eq!(disable_irq_pin_wakeup_bits(0x14), 0x04);
    }

    #[test]
    fn pmic_irq_policy_can_only_wake_on_vbus_insert_when_enabled() {
        assert_eq!(
            irq_enable2_bits_for_policy(crate::power::DeepSleepWakePolicy::TimerAndPmicIrq),
            AXP2101_VBUS_INSERT_IRQ_ENABLE
        );
    }

    #[test]
    fn maps_axp2101_status_to_charge_state() {
        let battery_connected = 1 << 3;
        let vbus_good = 1 << 5;

        assert_eq!(
            charge_state_from_summary(status_summary(battery_connected | vbus_good, 0x01 << 5)),
            ChargeState::Charging
        );
        assert_eq!(
            charge_state_from_summary(status_summary(battery_connected | vbus_good, 0x04)),
            ChargeState::Full
        );
        assert_eq!(
            charge_state_from_summary(status_summary(battery_connected, 0x02 << 5)),
            ChargeState::Discharging
        );
        assert_eq!(
            charge_state_from_summary(status_summary(vbus_good, 0x00)),
            ChargeState::Full
        );
    }

    #[test]
    fn maps_full_battery_without_vbus_as_not_externally_powered() {
        let battery_connected = 1 << 3;
        let battery = battery_status_from_registers(battery_connected, 0x04, Some(100));

        assert_eq!(battery.charge_state, ChargeState::Unknown);
        assert!(!battery.externally_powered());
    }
}
