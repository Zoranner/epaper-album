use crate::display::{Color, SCREEN_HEIGHT, SCREEN_WIDTH};
use std::fmt;

pub const EPD_WIDTH: usize = SCREEN_WIDTH;
pub const EPD_HEIGHT: usize = SCREEN_HEIGHT;
pub const EPD_ROW_BYTES: usize = EPD_WIDTH / 2;
pub const EPD_FRAME_BYTES: usize = EPD_ROW_BYTES * EPD_HEIGHT;

const INIT_SEQUENCE: &[(u8, &[u8])] = &[
    (0xAA, &[0x49, 0x55, 0x20, 0x08, 0x09, 0x18]),
    (0x01, &[0x3F]),
    (0x00, &[0x5F, 0x69]),
    (0x03, &[0x00, 0x54, 0x00, 0x44]),
    (0x05, &[0x40, 0x1F, 0x1F, 0x2C]),
    (0x06, &[0x6F, 0x1F, 0x17, 0x49]),
    (0x08, &[0x6F, 0x1F, 0x1F, 0x22]),
    (0x30, &[0x03]),
    (0x50, &[0x3F]),
    (0x60, &[0x02, 0x00]),
    (0x61, &[0x03, 0x20, 0x01, 0xE0]),
    (0x84, &[0x01]),
    (0xE3, &[0x2F]),
];

const BOOSTER_SOFT_START: &[u8] = &[0x6F, 0x1F, 0x17, 0x49];
#[cfg(target_os = "espidf")]
const BUSY_POLL_INTERVAL_MS: u32 = 10;
#[cfg(target_os = "espidf")]
const BUSY_POLL_ATTEMPTS: usize = 18_000;
const TEST_BARS: [Color; 6] = [
    Color::White,
    Color::Black,
    Color::Yellow,
    Color::Red,
    Color::Blue,
    Color::Green,
];

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum EpdError {
    BusyTimeout,
    Transport,
}

impl fmt::Display for EpdError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::BusyTimeout => write!(f, "busy-timeout"),
            Self::Transport => write!(f, "transport"),
        }
    }
}

impl std::error::Error for EpdError {}

pub trait EpdBus {
    fn reset(&mut self) -> Result<(), EpdError>;
    fn wait_until_ready(&mut self) -> Result<(), EpdError>;
    fn delay_ms(&mut self, milliseconds: u32);
    fn command(&mut self, command: u8) -> Result<(), EpdError>;
    fn data(&mut self, data: &[u8]) -> Result<(), EpdError>;
}

pub fn run_epd_hardware_self_test(bus: &mut impl EpdBus) -> Result<(), EpdError> {
    run_epd_frame(bus, |panel_y, row| {
        let source_y = EPD_HEIGHT - 1 - panel_y;
        fill_test_pattern_row(source_y, row);
        Ok(())
    })
}

pub fn epd_color_code(color: Color) -> u8 {
    match color {
        Color::Black => 0,
        Color::White => 1,
        Color::Yellow => 2,
        Color::Red => 3,
        Color::Blue => 5,
        Color::Green => 6,
    }
}

pub fn pack_epd_pixels(left: Color, right: Color) -> u8 {
    (epd_color_code(left) << 4) | epd_color_code(right)
}

pub fn set_packed_frame_pixel(frame: &mut [u8], x: usize, y: usize, color: Color) -> bool {
    if frame.len() != EPD_FRAME_BYTES || x >= EPD_WIDTH || y >= EPD_HEIGHT {
        return false;
    }

    let byte = &mut frame[y * EPD_ROW_BYTES + x / 2];
    let code = epd_color_code(color);
    if x.is_multiple_of(2) {
        *byte = (*byte & 0x0F) | (code << 4);
    } else {
        *byte = (*byte & 0xF0) | code;
    }

    true
}

fn init_panel(bus: &mut impl EpdBus) -> Result<(), EpdError> {
    bus.reset()?;
    bus.wait_until_ready()?;
    bus.delay_ms(50);

    for &(command, data) in INIT_SEQUENCE {
        bus.command(command)?;
        if !data.is_empty() {
            bus.data(data)?;
        }
    }

    bus.command(0x04)?;
    bus.delay_ms(100);
    bus.wait_until_ready()
}

fn run_epd_frame(
    bus: &mut impl EpdBus,
    mut fill_panel_row: impl FnMut(usize, &mut [u8; EPD_ROW_BYTES]) -> Result<(), EpdError>,
) -> Result<(), EpdError> {
    init_panel(bus)?;

    let mut row = [0u8; EPD_ROW_BYTES];
    bus.command(0x10)?;

    for panel_y in 0..EPD_HEIGHT {
        fill_panel_row(panel_y, &mut row)?;
        bus.data(&row)?;
    }

    refresh_and_power_off(bus)
}

pub fn run_epd_prepacked_frame(
    bus: &mut impl EpdBus,
    mut fill_panel_row: impl FnMut(usize, &mut [u8; EPD_ROW_BYTES]) -> Result<(), EpdError>,
) -> Result<(), EpdError> {
    run_epd_frame(bus, |panel_y, row| fill_panel_row(panel_y, row))
}

fn refresh_and_power_off(bus: &mut impl EpdBus) -> Result<(), EpdError> {
    bus.command(0x04)?;
    bus.delay_ms(100);
    bus.wait_until_ready()?;

    bus.command(0x06)?;
    bus.data(BOOSTER_SOFT_START)?;

    bus.command(0x12)?;
    bus.data(&[0x00])?;
    bus.delay_ms(100);
    bus.wait_until_ready()?;

    bus.command(0x02)?;
    bus.data(&[0x00])?;
    bus.delay_ms(100);
    bus.wait_until_ready()
}

fn fill_test_pattern_row(y: usize, row: &mut [u8; EPD_ROW_BYTES]) {
    let band_height = EPD_HEIGHT / TEST_BARS.len();
    let color = TEST_BARS[(y / band_height).min(TEST_BARS.len() - 1)];
    let packed = pack_epd_pixels(color, color);

    row.fill(packed);
}

#[cfg(target_os = "espidf")]
pub mod espidf {
    use super::{EpdBus, EpdError, BUSY_POLL_ATTEMPTS, BUSY_POLL_INTERVAL_MS};
    use esp_idf_hal::delay::FreeRtos;
    use esp_idf_hal::gpio::{
        AnyOutputPin, Gpio12, Gpio13, Gpio8, Gpio9, Input, Output, PinDriver, Pull,
    };
    use esp_idf_hal::spi::{config, SpiDeviceDriver, SpiDriverConfig, SPI3};
    use esp_idf_hal::units::FromValueType;

    type EpdSpiDevice = SpiDeviceDriver<'static, esp_idf_hal::spi::SpiDriver<'static>>;

    pub struct EspEpdBus {
        spi: EpdSpiDevice,
        cs: PinDriver<'static, Output>,
        dc: PinDriver<'static, Output>,
        rst: PinDriver<'static, Output>,
        busy: PinDriver<'static, Input>,
    }

    impl EspEpdBus {
        #[allow(clippy::too_many_arguments)]
        pub fn new(
            spi: SPI3<'static>,
            sclk: esp_idf_hal::gpio::Gpio10<'static>,
            mosi: esp_idf_hal::gpio::Gpio11<'static>,
            cs: Gpio9<'static>,
            dc: Gpio8<'static>,
            rst: Gpio12<'static>,
            busy: Gpio13<'static>,
        ) -> Result<Self, esp_idf_sys::EspError> {
            let spi_config = config::Config::new()
                .baudrate(10.MHz().into())
                .data_mode(config::MODE_0)
                .write_only(true)
                .duplex(config::Duplex::Half);
            let spi = SpiDeviceDriver::new_single(
                spi,
                sclk,
                mosi,
                None::<esp_idf_hal::gpio::AnyIOPin>,
                Option::<AnyOutputPin>::None,
                &SpiDriverConfig::new(),
                &spi_config,
            )?;
            let mut cs = PinDriver::output(cs)?;
            cs.set_high()?;

            Ok(Self {
                spi,
                cs,
                dc: PinDriver::output(dc)?,
                rst: PinDriver::output(rst)?,
                busy: PinDriver::input(busy, Pull::Up)?,
            })
        }
    }

    impl EpdBus for EspEpdBus {
        fn reset(&mut self) -> Result<(), EpdError> {
            self.rst.set_high().map_err(|_| EpdError::Transport)?;
            FreeRtos::delay_ms(50);
            self.rst.set_low().map_err(|_| EpdError::Transport)?;
            FreeRtos::delay_ms(20);
            self.rst.set_high().map_err(|_| EpdError::Transport)?;
            FreeRtos::delay_ms(50);
            Ok(())
        }

        fn wait_until_ready(&mut self) -> Result<(), EpdError> {
            for _ in 0..BUSY_POLL_ATTEMPTS {
                if self.busy.is_high() {
                    return Ok(());
                }

                FreeRtos::delay_ms(BUSY_POLL_INTERVAL_MS);
            }

            Err(EpdError::BusyTimeout)
        }

        fn delay_ms(&mut self, milliseconds: u32) {
            FreeRtos::delay_ms(milliseconds);
        }

        fn command(&mut self, command: u8) -> Result<(), EpdError> {
            self.dc.set_low().map_err(|_| EpdError::Transport)?;
            self.cs.set_low().map_err(|_| EpdError::Transport)?;
            let result = self.spi.write(&[command]).map_err(|_| EpdError::Transport);
            self.cs.set_high().map_err(|_| EpdError::Transport)?;
            result
        }

        fn data(&mut self, data: &[u8]) -> Result<(), EpdError> {
            self.dc.set_high().map_err(|_| EpdError::Transport)?;
            self.cs.set_low().map_err(|_| EpdError::Transport)?;
            let result = self.spi.write(data).map_err(|_| EpdError::Transport);
            self.cs.set_high().map_err(|_| EpdError::Transport)?;
            result
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[derive(Default)]
    struct MockEpdBus {
        events: Vec<MockEvent>,
    }

    #[derive(Debug, Eq, PartialEq)]
    enum MockEvent {
        Reset,
        Wait,
        Delay(u32),
        Command(u8),
        Data(Vec<u8>),
    }

    impl EpdBus for MockEpdBus {
        fn reset(&mut self) -> Result<(), EpdError> {
            self.events.push(MockEvent::Reset);
            Ok(())
        }

        fn wait_until_ready(&mut self) -> Result<(), EpdError> {
            self.events.push(MockEvent::Wait);
            Ok(())
        }

        fn delay_ms(&mut self, milliseconds: u32) {
            self.events.push(MockEvent::Delay(milliseconds));
        }

        fn command(&mut self, command: u8) -> Result<(), EpdError> {
            self.events.push(MockEvent::Command(command));
            Ok(())
        }

        fn data(&mut self, data: &[u8]) -> Result<(), EpdError> {
            self.events.push(MockEvent::Data(data.to_vec()));
            Ok(())
        }
    }

    #[test]
    fn maps_album_colors_to_panel_nibbles() {
        assert_eq!(epd_color_code(Color::Black), 0);
        assert_eq!(epd_color_code(Color::White), 1);
        assert_eq!(epd_color_code(Color::Yellow), 2);
        assert_eq!(epd_color_code(Color::Red), 3);
        assert_eq!(epd_color_code(Color::Blue), 5);
        assert_eq!(epd_color_code(Color::Green), 6);
    }

    #[test]
    fn packs_two_pixels_into_high_and_low_nibbles() {
        assert_eq!(pack_epd_pixels(Color::White, Color::Black), 0x10);
        assert_eq!(pack_epd_pixels(Color::Red, Color::Green), 0x36);
    }

    #[test]
    fn sets_pixel_in_prepacked_frame() {
        let mut frame = vec![0x11; EPD_FRAME_BYTES];

        assert!(set_packed_frame_pixel(&mut frame, 0, 0, Color::Black));
        assert!(set_packed_frame_pixel(&mut frame, 1, 0, Color::Red));
        assert!(!set_packed_frame_pixel(
            &mut frame,
            EPD_WIDTH,
            0,
            Color::Black
        ));

        assert_eq!(frame[0], 0x03);
    }

    #[test]
    fn self_test_sends_init_frame_and_refresh_sequence() {
        let mut bus = MockEpdBus::default();

        run_epd_hardware_self_test(&mut bus).unwrap();

        assert_eq!(bus.events[0], MockEvent::Reset);
        assert_eq!(bus.events[1], MockEvent::Wait);
        assert_eq!(bus.events[2], MockEvent::Delay(50));
        assert!(bus.events.contains(&MockEvent::Command(0xAA)));
        assert!(bus.events.contains(&MockEvent::Command(0x10)));
        assert_eq!(
            bus.events
                .iter()
                .filter(
                    |event| matches!(event, MockEvent::Data(data) if data.len() == EPD_ROW_BYTES)
                )
                .count(),
            EPD_HEIGHT
        );
        assert!(bus.events.ends_with(&[
            MockEvent::Command(0x12),
            MockEvent::Data(vec![0x00]),
            MockEvent::Delay(100),
            MockEvent::Wait,
            MockEvent::Command(0x02),
            MockEvent::Data(vec![0x00]),
            MockEvent::Delay(100),
            MockEvent::Wait,
        ]));
    }

    #[test]
    fn test_pattern_rows_are_plain_horizontal_color_bars() {
        let mut row = [0u8; EPD_ROW_BYTES];

        fill_test_pattern_row(0, &mut row);
        assert!(row.iter().all(|byte| *byte == 0x11));

        fill_test_pattern_row(EPD_HEIGHT / 6, &mut row);
        assert!(row.iter().all(|byte| *byte == 0x00));

        fill_test_pattern_row(EPD_HEIGHT - 1, &mut row);
        assert!(row.iter().all(|byte| *byte == 0x66));
    }

    #[test]
    fn self_test_transmits_rows_in_panel_vertical_order() {
        let mut bus = MockEpdBus::default();

        run_epd_hardware_self_test(&mut bus).unwrap();

        let rows = bus.events.iter().filter_map(|event| match event {
            MockEvent::Data(data) if data.len() == EPD_ROW_BYTES => Some(data),
            _ => None,
        });
        let rows = rows.collect::<Vec<_>>();

        assert!(rows[0].iter().all(|byte| *byte == 0x66));
        assert!(rows[EPD_HEIGHT - 1].iter().all(|byte| *byte == 0x11));
    }
}
