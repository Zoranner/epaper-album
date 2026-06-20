pub const SELF_TEST_TONE_SAMPLE_RATE_HZ: u32 = 16_000;
pub const SELF_TEST_TONE_CHANNELS: usize = 2;
pub const SELF_TEST_TONE_BITS_PER_SAMPLE: usize = 16;

const SELF_TEST_TONE_AMPLITUDE: i16 = 8_000;
const SELF_TEST_TONE_SEGMENTS: [ToneSegment; 3] = [
    ToneSegment {
        frequency_hz: 880,
        duration_ms: 80,
    },
    ToneSegment {
        frequency_hz: 0,
        duration_ms: 40,
    },
    ToneSegment {
        frequency_hz: 1_320,
        duration_ms: 80,
    },
];

#[derive(Clone, Copy)]
struct ToneSegment {
    frequency_hz: u32,
    duration_ms: u32,
}

pub fn self_test_tone_pcm() -> Vec<u8> {
    let mut pcm = Vec::with_capacity(self_test_tone_pcm_len());
    for segment in SELF_TEST_TONE_SEGMENTS {
        append_tone_segment(&mut pcm, segment);
    }
    pcm
}

pub const fn self_test_tone_pcm_len() -> usize {
    let mut samples = 0usize;
    let mut index = 0usize;
    while index < SELF_TEST_TONE_SEGMENTS.len() {
        samples += segment_sample_count(SELF_TEST_TONE_SEGMENTS[index].duration_ms);
        index += 1;
    }

    samples * SELF_TEST_TONE_CHANNELS * core::mem::size_of::<i16>()
}

const fn segment_sample_count(duration_ms: u32) -> usize {
    (SELF_TEST_TONE_SAMPLE_RATE_HZ as usize * duration_ms as usize) / 1_000
}

fn append_tone_segment(pcm: &mut Vec<u8>, segment: ToneSegment) {
    let samples = segment_sample_count(segment.duration_ms);
    for sample_index in 0..samples {
        let sample = if segment.frequency_hz == 0 {
            0
        } else {
            square_wave_sample(sample_index, segment.frequency_hz)
        };
        append_stereo_sample(pcm, sample);
    }
}

fn square_wave_sample(sample_index: usize, frequency_hz: u32) -> i16 {
    let period = (SELF_TEST_TONE_SAMPLE_RATE_HZ / frequency_hz).max(1) as usize;
    if sample_index % period < period / 2 {
        SELF_TEST_TONE_AMPLITUDE
    } else {
        -SELF_TEST_TONE_AMPLITUDE
    }
}

fn append_stereo_sample(pcm: &mut Vec<u8>, sample: i16) {
    let bytes = sample.to_le_bytes();
    for _ in 0..SELF_TEST_TONE_CHANNELS {
        pcm.extend_from_slice(&bytes);
    }
}

#[cfg(target_os = "espidf")]
pub mod espidf {
    use super::{
        self_test_tone_pcm, SELF_TEST_TONE_BITS_PER_SAMPLE, SELF_TEST_TONE_SAMPLE_RATE_HZ,
    };
    use core::ffi::c_void;
    use core::ptr;
    use esp_idf_sys::{
        esp, gpio_mode_t_GPIO_MODE_OUTPUT, gpio_num_t_GPIO_NUM_14, gpio_num_t_GPIO_NUM_15,
        gpio_num_t_GPIO_NUM_16, gpio_num_t_GPIO_NUM_17, gpio_num_t_GPIO_NUM_47,
        gpio_num_t_GPIO_NUM_48, gpio_num_t_GPIO_NUM_7, gpio_num_t_GPIO_NUM_NC, gpio_set_direction,
        gpio_set_level, i2c_config_t, i2c_config_t__bindgen_ty_1,
        i2c_config_t__bindgen_ty_1__bindgen_ty_1, i2c_driver_delete, i2c_driver_install,
        i2c_master_write_to_device, i2c_mode_t_I2C_MODE_MASTER, i2c_param_config, i2c_port_t,
        i2c_port_t_I2C_NUM_0, i2s_chan_config_t, i2s_chan_config_t__bindgen_ty_1,
        i2s_chan_handle_t, i2s_channel_disable, i2s_channel_enable, i2s_channel_init_std_mode,
        i2s_channel_write, i2s_data_bit_width_t_I2S_DATA_BIT_WIDTH_16BIT, i2s_del_channel,
        i2s_mclk_multiple_t_I2S_MCLK_MULTIPLE_256, i2s_port_t_I2S_NUM_0,
        i2s_role_t_I2S_ROLE_MASTER, i2s_slot_bit_width_t_I2S_SLOT_BIT_WIDTH_AUTO,
        i2s_slot_mode_t_I2S_SLOT_MODE_STEREO, i2s_std_clk_config_t, i2s_std_config_t,
        i2s_std_gpio_config_t, i2s_std_gpio_config_t__bindgen_ty_1, i2s_std_slot_config_t,
        i2s_std_slot_mask_t_I2S_STD_SLOT_BOTH, soc_periph_i2s_clk_src_t_I2S_CLK_SRC_DEFAULT,
        EspError, TickType_t, ESP_ERR_INVALID_STATE, ESP_INTR_FLAG_LEVEL1,
    };

    const ES8311_ADDRESS: u8 = 0x18;
    const I2C_TIMEOUT_TICKS: TickType_t = 1_000;
    const I2S_WRITE_TIMEOUT_MS: u32 = 500;
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

    pub fn play_self_test_request_tone() -> Result<(), EspError> {
        let _i2c = I2cBusGuard::install()?;
        init_es8311()?;
        let mut i2s = I2sTxGuard::install()?;
        enable_pa(true)?;

        let pcm = self_test_tone_pcm();
        i2s.write_all(&pcm)?;
        esp_idf_hal::delay::FreeRtos::delay_ms(30);

        enable_pa(false)?;
        Ok(())
    }

    struct I2cBusGuard {
        port: i2c_port_t,
    }

    impl I2cBusGuard {
        fn install() -> Result<Self, EspError> {
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

    struct I2sTxGuard {
        handle: i2s_chan_handle_t,
        enabled: bool,
    }

    impl I2sTxGuard {
        fn install() -> Result<Self, EspError> {
            let chan_cfg = i2s_chan_config_t {
                id: i2s_port_t_I2S_NUM_0,
                role: i2s_role_t_I2S_ROLE_MASTER,
                dma_desc_num: 6,
                dma_frame_num: 240,
                __bindgen_anon_1: i2s_chan_config_t__bindgen_ty_1 { auto_clear: true },
                auto_clear_before_cb: true,
                allow_pd: false,
                intr_priority: ESP_INTR_FLAG_LEVEL1 as i32,
            };
            let mut tx_handle = ptr::null_mut();

            unsafe {
                esp!(esp_idf_sys::i2s_new_channel(
                    &chan_cfg,
                    &mut tx_handle,
                    ptr::null_mut(),
                ))?;
                esp!(i2s_channel_init_std_mode(tx_handle, &std_i2s_config()))?;
                esp!(i2s_channel_enable(tx_handle))?;
            }

            Ok(Self {
                handle: tx_handle,
                enabled: true,
            })
        }

        fn write_all(&mut self, data: &[u8]) -> Result<(), EspError> {
            let mut offset = 0;
            while offset < data.len() {
                let mut written = 0usize;
                unsafe {
                    esp!(i2s_channel_write(
                        self.handle,
                        data[offset..].as_ptr() as *const c_void,
                        data.len() - offset,
                        &mut written,
                        I2S_WRITE_TIMEOUT_MS,
                    ))?;
                }
                offset += written;
            }

            Ok(())
        }
    }

    impl Drop for I2sTxGuard {
        fn drop(&mut self) {
            unsafe {
                if self.enabled {
                    let result = i2s_channel_disable(self.handle);
                    if result == ESP_ERR_INVALID_STATE {
                        self.enabled = false;
                    } else {
                        let _ = esp!(result);
                    }
                }
                let _ = esp!(i2s_del_channel(self.handle));
            }
        }
    }

    fn std_i2s_config() -> i2s_std_config_t {
        i2s_std_config_t {
            clk_cfg: i2s_std_clk_config_t {
                sample_rate_hz: SELF_TEST_TONE_SAMPLE_RATE_HZ,
                clk_src: soc_periph_i2s_clk_src_t_I2S_CLK_SRC_DEFAULT,
                ext_clk_freq_hz: 0,
                mclk_multiple: i2s_mclk_multiple_t_I2S_MCLK_MULTIPLE_256,
                bclk_div: 0,
            },
            slot_cfg: i2s_std_slot_config_t {
                data_bit_width: i2s_data_bit_width_t_I2S_DATA_BIT_WIDTH_16BIT,
                slot_bit_width: i2s_slot_bit_width_t_I2S_SLOT_BIT_WIDTH_AUTO,
                slot_mode: i2s_slot_mode_t_I2S_SLOT_MODE_STEREO,
                slot_mask: i2s_std_slot_mask_t_I2S_STD_SLOT_BOTH,
                ws_width: SELF_TEST_TONE_BITS_PER_SAMPLE as u32,
                ws_pol: false,
                bit_shift: true,
                left_align: false,
                big_endian: false,
                bit_order_lsb: false,
            },
            gpio_cfg: i2s_std_gpio_config_t {
                mclk: gpio_num_t_GPIO_NUM_14,
                bclk: gpio_num_t_GPIO_NUM_15,
                ws: gpio_num_t_GPIO_NUM_16,
                dout: gpio_num_t_GPIO_NUM_17,
                din: gpio_num_t_GPIO_NUM_NC,
                invert_flags: i2s_std_gpio_config_t__bindgen_ty_1::default(),
            },
        }
    }

    fn init_es8311() -> Result<(), EspError> {
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

    fn enable_pa(enable: bool) -> Result<(), EspError> {
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
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn self_test_tone_pcm_has_expected_stereo_16bit_length() {
        let pcm = self_test_tone_pcm();

        assert_eq!(pcm.len(), self_test_tone_pcm_len());
        assert_eq!(pcm.len(), 12_800);
        assert_eq!(pcm.len() % (SELF_TEST_TONE_CHANNELS * 2), 0);
    }

    #[test]
    fn self_test_tone_pcm_contains_two_tones_with_silence_between() {
        let pcm = self_test_tone_pcm();
        let first_sample = i16::from_le_bytes([pcm[0], pcm[1]]);
        let pause_start = 80 * SELF_TEST_TONE_SAMPLE_RATE_HZ as usize / 1_000;
        let pause_byte = pause_start * SELF_TEST_TONE_CHANNELS * 2;
        let second_start = 120 * SELF_TEST_TONE_SAMPLE_RATE_HZ as usize / 1_000;
        let second_byte = second_start * SELF_TEST_TONE_CHANNELS * 2;

        assert_ne!(first_sample, 0);
        assert!(pcm[pause_byte..pause_byte + 160]
            .iter()
            .all(|byte| *byte == 0));
        assert_ne!(
            i16::from_le_bytes([pcm[second_byte], pcm[second_byte + 1]]),
            0
        );
    }
}
