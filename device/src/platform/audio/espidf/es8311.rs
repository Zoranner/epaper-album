use crate::platform::audio::SELF_TEST_TONE_SAMPLE_RATE_HZ;
use esp_idf_sys::{
    esp, gpio_mode_t_GPIO_MODE_OUTPUT, gpio_num_t_GPIO_NUM_47, gpio_num_t_GPIO_NUM_48,
    gpio_num_t_GPIO_NUM_7, gpio_set_direction, gpio_set_level, i2c_config_t,
    i2c_config_t__bindgen_ty_1, i2c_config_t__bindgen_ty_1__bindgen_ty_1, i2c_driver_delete,
    i2c_driver_install, i2c_master_write_to_device, i2c_mode_t_I2C_MODE_MASTER, i2c_param_config,
    i2c_port_t, i2c_port_t_I2C_NUM_0, EspError, TickType_t,
};

const ES8311_ADDRESS: u8 = 0x18;
const I2C_TIMEOUT_TICKS: TickType_t = 1_000;
const ES8311_MCLK_DIV: u32 = 256;

const ES8311_RESET_REG00: u8 = 0x00;
const ES8311_CLK_MANAGER_REG01: u8 = 0x01;
const ES8311_CLK_MANAGER_REG02: u8 = 0x02;
const ES8311_CLK_MANAGER_REG03: u8 = 0x03;
const ES8311_CLK_MANAGER_REG04: u8 = 0x04;
const ES8311_CLK_MANAGER_REG05: u8 = 0x05;
const ES8311_CLK_MANAGER_REG06: u8 = 0x06;
const ES8311_CLK_MANAGER_REG07: u8 = 0x07;
const ES8311_CLK_MANAGER_REG08: u8 = 0x08;
const ES8311_SDPIN_REG09: u8 = 0x09;
const ES8311_SDPOUT_REG0A: u8 = 0x0A;
const ES8311_SYSTEM_REG0B: u8 = 0x0B;
const ES8311_SYSTEM_REG0C: u8 = 0x0C;
const ES8311_SYSTEM_REG0D: u8 = 0x0D;
const ES8311_SYSTEM_REG0E: u8 = 0x0E;
const ES8311_SYSTEM_REG10: u8 = 0x10;
const ES8311_SYSTEM_REG11: u8 = 0x11;
const ES8311_SYSTEM_REG12: u8 = 0x12;
const ES8311_SYSTEM_REG13: u8 = 0x13;
const ES8311_SYSTEM_REG14: u8 = 0x14;
const ES8311_ADC_REG15: u8 = 0x15;
const ES8311_ADC_REG16: u8 = 0x16;
const ES8311_ADC_REG17: u8 = 0x17;
const ES8311_ADC_REG1B: u8 = 0x1B;
const ES8311_ADC_REG1C: u8 = 0x1C;
const ES8311_DAC_REG31: u8 = 0x31;
const ES8311_DAC_REG32: u8 = 0x32;
const ES8311_DAC_REG37: u8 = 0x37;
const ES8311_GPIO_REG44: u8 = 0x44;
const ES8311_GP_REG45: u8 = 0x45;

pub(super) struct I2cBusGuard {
    port: i2c_port_t,
}

impl I2cBusGuard {
    pub(super) fn install() -> Result<Self, EspError> {
        let config = i2c_config_t {
            mode: i2c_mode_t_I2C_MODE_MASTER,
            sda_io_num: gpio_num_t_GPIO_NUM_47,
            scl_io_num: gpio_num_t_GPIO_NUM_48,
            sda_pullup_en: true,
            scl_pullup_en: true,
            __bindgen_anon_1: i2c_config_t__bindgen_ty_1 {
                master: i2c_config_t__bindgen_ty_1__bindgen_ty_1 { clk_speed: 400_000 },
            },
            clk_flags: 0,
        };

        unsafe {
            esp!(i2c_param_config(i2c_port_t_I2C_NUM_0, &config))?;
            esp!(i2c_driver_install(
                i2c_port_t_I2C_NUM_0,
                i2c_mode_t_I2C_MODE_MASTER,
                0,
                0,
                0,
            ))?;
        }

        Ok(Self {
            port: i2c_port_t_I2C_NUM_0,
        })
    }
}

impl Drop for I2cBusGuard {
    fn drop(&mut self) {
        unsafe {
            let _ = esp!(i2c_driver_delete(self.port));
        }
    }
}

pub(super) fn init_es8311() -> Result<(), EspError> {
    enable_pa(false)?;

    write_es8311(ES8311_GPIO_REG44, 0x08)?;
    write_es8311(ES8311_GPIO_REG44, 0x08)?;
    write_es8311(ES8311_CLK_MANAGER_REG01, 0x30)?;
    write_es8311(ES8311_CLK_MANAGER_REG02, 0x00)?;
    write_es8311(ES8311_CLK_MANAGER_REG03, 0x10)?;
    write_es8311(ES8311_ADC_REG16, 0x24)?;
    write_es8311(ES8311_CLK_MANAGER_REG04, 0x10)?;
    write_es8311(ES8311_CLK_MANAGER_REG05, 0x00)?;
    write_es8311(ES8311_SYSTEM_REG0B, 0x00)?;
    write_es8311(ES8311_SYSTEM_REG0C, 0x00)?;
    write_es8311(ES8311_SYSTEM_REG10, 0x1F)?;
    write_es8311(ES8311_SYSTEM_REG11, 0x7F)?;
    write_es8311(ES8311_RESET_REG00, 0x80)?;
    write_es8311(ES8311_RESET_REG00, 0x80)?;
    write_es8311(ES8311_CLK_MANAGER_REG01, 0x3F)?;
    write_es8311(ES8311_CLK_MANAGER_REG06, 0x00)?;
    write_es8311(ES8311_SYSTEM_REG13, 0x10)?;
    write_es8311(ES8311_ADC_REG1B, 0x0A)?;
    write_es8311(ES8311_ADC_REG1C, 0x6A)?;
    write_es8311(ES8311_GPIO_REG44, 0x58)?;

    write_es8311(ES8311_SDPIN_REG09, 0x0C)?;
    write_es8311(ES8311_SDPOUT_REG0A, 0x0C)?;
    configure_sample_rate_16k()?;
    write_es8311(ES8311_DAC_REG31, 0x00)?;
    write_es8311(ES8311_DAC_REG32, 0xD0)?;

    write_es8311(ES8311_RESET_REG00, 0x80)?;
    write_es8311(ES8311_CLK_MANAGER_REG01, 0x3F)?;
    write_es8311(ES8311_SDPIN_REG09, 0x0C)?;
    write_es8311(ES8311_SDPOUT_REG0A, 0x4C)?;
    write_es8311(ES8311_ADC_REG17, 0xBF)?;
    write_es8311(ES8311_SYSTEM_REG0E, 0x02)?;
    write_es8311(ES8311_SYSTEM_REG12, 0x00)?;
    write_es8311(ES8311_SYSTEM_REG14, 0x1A)?;
    write_es8311(ES8311_SYSTEM_REG0D, 0x01)?;
    write_es8311(ES8311_ADC_REG15, 0x40)?;
    write_es8311(ES8311_DAC_REG37, 0x08)?;
    write_es8311(ES8311_GP_REG45, 0x00)
}

fn configure_sample_rate_16k() -> Result<(), EspError> {
    let mclk = SELF_TEST_TONE_SAMPLE_RATE_HZ * ES8311_MCLK_DIV;
    if mclk != 4_096_000 {
        unreachable!("self-test tone ES8311 coefficients are fixed for 16 kHz");
    }

    write_es8311(ES8311_CLK_MANAGER_REG02, 0x00)?;
    write_es8311(ES8311_CLK_MANAGER_REG05, 0x00)?;
    write_es8311(ES8311_CLK_MANAGER_REG03, 0x10)?;
    write_es8311(ES8311_CLK_MANAGER_REG04, 0x10)?;
    write_es8311(ES8311_CLK_MANAGER_REG07, 0x00)?;
    write_es8311(ES8311_CLK_MANAGER_REG08, 0xFF)?;
    write_es8311(ES8311_CLK_MANAGER_REG06, 0x03)
}

pub(super) fn enable_pa(enable: bool) -> Result<(), EspError> {
    unsafe {
        esp!(gpio_set_direction(
            gpio_num_t_GPIO_NUM_7,
            gpio_mode_t_GPIO_MODE_OUTPUT,
        ))?;
        esp!(gpio_set_level(gpio_num_t_GPIO_NUM_7, u32::from(enable)))
    }
}

fn write_es8311(register: u8, value: u8) -> Result<(), EspError> {
    let data = [register, value];
    unsafe {
        esp!(i2c_master_write_to_device(
            i2c_port_t_I2C_NUM_0,
            ES8311_ADDRESS,
            data.as_ptr(),
            data.len(),
            I2C_TIMEOUT_TICKS,
        ))
    }
}
