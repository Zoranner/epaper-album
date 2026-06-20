use crate::platform::audio::{SELF_TEST_TONE_BITS_PER_SAMPLE, SELF_TEST_TONE_SAMPLE_RATE_HZ};
use core::ffi::c_void;
use core::ptr;
use esp_idf_sys::{
    esp, gpio_num_t_GPIO_NUM_14, gpio_num_t_GPIO_NUM_15, gpio_num_t_GPIO_NUM_16,
    gpio_num_t_GPIO_NUM_17, gpio_num_t_GPIO_NUM_NC, i2s_chan_config_t,
    i2s_chan_config_t__bindgen_ty_1, i2s_chan_handle_t, i2s_channel_disable, i2s_channel_enable,
    i2s_channel_init_std_mode, i2s_channel_write, i2s_data_bit_width_t_I2S_DATA_BIT_WIDTH_16BIT,
    i2s_del_channel, i2s_mclk_multiple_t_I2S_MCLK_MULTIPLE_256, i2s_port_t_I2S_NUM_0,
    i2s_role_t_I2S_ROLE_MASTER, i2s_slot_bit_width_t_I2S_SLOT_BIT_WIDTH_AUTO,
    i2s_slot_mode_t_I2S_SLOT_MODE_STEREO, i2s_std_clk_config_t, i2s_std_config_t,
    i2s_std_gpio_config_t, i2s_std_gpio_config_t__bindgen_ty_1, i2s_std_slot_config_t,
    i2s_std_slot_mask_t_I2S_STD_SLOT_BOTH, soc_periph_i2s_clk_src_t_I2S_CLK_SRC_DEFAULT, EspError,
    ESP_ERR_INVALID_STATE, ESP_INTR_FLAG_LEVEL1,
};

const I2S_WRITE_TIMEOUT_MS: u32 = 500;

pub(super) struct I2sTxGuard {
    handle: i2s_chan_handle_t,
    enabled: bool,
}

impl I2sTxGuard {
    pub(super) fn install() -> Result<Self, EspError> {
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

    pub(super) fn write_all(&mut self, data: &[u8]) -> Result<(), EspError> {
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
