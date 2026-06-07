use crate::config::{Config, CONFIG_PATH};
use crate::epd::{
    espidf::EspEpdBus, run_epd_hardware_self_test, run_epd_prepacked_frame, EPD_FRAME_BYTES,
    EPD_ROW_BYTES,
};
use crate::pmic::espidf::{chip_id_is_axp2101, init_axp2101_for_photo_painter};
use crate::selftest::{ConfigProbe, RenderProbe, SelfTestReport, StorageProbe};
use crate::storage::{with_mounted_sdcard_parts, StorageBinaryRead, StorageRead};
use std::io::{Read, Seek, SeekFrom};
use std::path::Path;

const TEST_BMP_PATH: &str = "/sdcard/test.bmp";

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum EpdProbe {
    Refreshed,
    PhotoRefreshed,
    ImageFormatError,
    ImageReadError,
    InitError,
    BusyTimeout,
    TransportError,
}

impl EpdProbe {
    pub const fn label(self) -> &'static str {
        match self {
            Self::Refreshed => "refreshed",
            Self::PhotoRefreshed => "photo-refreshed",
            Self::ImageFormatError => "image-format-error",
            Self::ImageReadError => "image-read-error",
            Self::InitError => "init-error",
            Self::BusyTimeout => "busy-timeout",
            Self::TransportError => "transport-error",
        }
    }
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct HardwareSelfTestReport {
    pub base: SelfTestReport,
    pub epd: EpdProbe,
}

pub fn run_espidf_hardware_self_test() -> HardwareSelfTestReport {
    let peripherals = match esp_idf_svc::hal::peripherals::Peripherals::take() {
        Ok(peripherals) => peripherals,
        Err(_) => {
            return HardwareSelfTestReport {
                base: SelfTestReport {
                    storage: StorageProbe::MountError,
                    config: ConfigProbe::ReadError,
                    render: RenderProbe {
                        refresh_count: 0,
                        slept: false,
                    },
                },
                epd: EpdProbe::InitError,
            };
        }
    };

    let pins = peripherals.pins;
    match init_axp2101_for_photo_painter(peripherals.i2c0, pins.gpio47, pins.gpio48) {
        Ok(probe) => {
            log::info!(
                target: "epaper_album",
                "pmic: chip=0x{:02x} axp2101={} dc=0x{:02x} ldo=0x{:02x}",
                probe.chip_id,
                chip_id_is_axp2101(probe),
                probe.dc_onoff,
                probe.ldo_onoff0
            );
        }
        Err(_) => {
            log::warn!(target: "epaper_album", "pmic: init-error");
        }
    }

    let (config_read, image_read) = match with_mounted_sdcard_parts(
        peripherals.sdmmc1,
        pins.gpio41,
        pins.gpio39,
        pins.gpio40,
        pins.gpio1,
        pins.gpio2,
        pins.gpio38,
        || {
            let config_read = read_text_from_mounted_path(Path::new(CONFIG_PATH));
            let image_read = read_epd_frame_from_mounted_bmp(Path::new(TEST_BMP_PATH));
            Ok((config_read, image_read))
        },
    ) {
        Ok(Ok(files)) => files,
        Ok(Err(_)) => (StorageRead::ReadError, StorageBinaryRead::ReadError),
        Err(_) => (StorageRead::MountError, StorageBinaryRead::MountError),
    };

    let storage = probe_storage(&config_read);
    let config = probe_config(config_read);
    let epd = match EspEpdBus::new(
        peripherals.spi3,
        pins.gpio10,
        pins.gpio11,
        pins.gpio9,
        pins.gpio8,
        pins.gpio12,
        pins.gpio13,
    ) {
        Ok(mut bus) => refresh_epd_from_self_test_image(&mut bus, image_read),
        Err(_) => EpdProbe::InitError,
    };

    HardwareSelfTestReport {
        base: SelfTestReport {
            storage,
            config,
            render: RenderProbe {
                refresh_count: 0,
                slept: false,
            },
        },
        epd,
    }
}

fn refresh_epd_from_self_test_image(
    bus: &mut EspEpdBus,
    image_read: StorageBinaryRead,
) -> EpdProbe {
    match image_read {
        StorageBinaryRead::Bytes(frame) => {
            log::info!(
                target: "epaper_album",
                "test.bmp: {} frame bytes, photo-refresh",
                frame.len()
            );

            match run_epd_memory_frame(bus, &frame) {
                Ok(()) => EpdProbe::PhotoRefreshed,
                Err(error) => epd_error_probe(error),
            }
        }
        StorageBinaryRead::Missing => match run_epd_hardware_self_test(bus) {
            Ok(()) => EpdProbe::Refreshed,
            Err(error) => epd_error_probe(error),
        },
        StorageBinaryRead::FormatError => EpdProbe::ImageFormatError,
        StorageBinaryRead::ReadError => EpdProbe::ImageReadError,
        StorageBinaryRead::MountError => EpdProbe::ImageReadError,
    }
}

fn read_epd_frame_from_mounted_bmp(image_path: &Path) -> StorageBinaryRead {
    match read_epd_frame_from_mounted_bmp_result(image_path) {
        Ok(frame) => StorageBinaryRead::Bytes(frame),
        Err(ReadImageError::Missing) => StorageBinaryRead::Missing,
        Err(ReadImageError::Format) => StorageBinaryRead::FormatError,
        Err(ReadImageError::Read) => StorageBinaryRead::ReadError,
    }
}

fn read_epd_frame_from_mounted_bmp_result(image_path: &Path) -> Result<Vec<u8>, ReadImageError> {
    let mut file = match std::fs::File::open(image_path) {
        Ok(file) => file,
        Err(error) if error.kind() == std::io::ErrorKind::NotFound => {
            return Err(ReadImageError::Missing);
        }
        Err(_) => return Err(ReadImageError::Read),
    };
    let total_len = file.metadata().map_err(|_| ReadImageError::Read)?.len();
    let mut header_bytes = [0u8; 54];
    file.read_exact(&mut header_bytes)
        .map_err(|_| ReadImageError::Read)?;
    let header = crate::bmp::parse_bmp_header(&header_bytes, total_len)
        .map_err(|_| ReadImageError::Format)?;
    let row_stride = crate::bmp::bmp_24bit_row_stride();
    let mut row_bgr = vec![0u8; row_stride];
    let mut frame = vec![0u8; EPD_FRAME_BYTES];

    for panel_y in 0..crate::epd::EPD_HEIGHT {
        let source_y = crate::epd::EPD_HEIGHT - 1 - panel_y;
        let bmp_file_y = if header.top_down {
            source_y
        } else {
            crate::epd::EPD_HEIGHT - 1 - source_y
        };
        let offset = header.pixel_offset + (bmp_file_y * row_stride) as u64;
        file.seek(SeekFrom::Start(offset))
            .map_err(|_| ReadImageError::Read)?;
        file.read_exact(&mut row_bgr)
            .map_err(|_| ReadImageError::Read)?;
        let row_start = panel_y * EPD_ROW_BYTES;
        let row = (&mut frame[row_start..row_start + EPD_ROW_BYTES])
            .try_into()
            .map_err(|_| ReadImageError::Read)?;
        crate::bmp::fill_epd_row_from_bgr_mirrored(&row_bgr, row)
            .map_err(|_| ReadImageError::Format)?;
    }

    Ok(frame)
}

fn run_epd_memory_frame(bus: &mut EspEpdBus, frame: &[u8]) -> Result<(), crate::epd::EpdError> {
    if frame.len() != EPD_FRAME_BYTES {
        return Err(crate::epd::EpdError::Transport);
    }

    run_epd_prepacked_frame(bus, |panel_y, row| {
        let row_start = panel_y * EPD_ROW_BYTES;
        row.copy_from_slice(&frame[row_start..row_start + EPD_ROW_BYTES]);
        Ok(())
    })
}

enum ReadImageError {
    Missing,
    Format,
    Read,
}

fn read_text_from_mounted_path(path: &Path) -> StorageRead {
    match std::fs::read_to_string(path) {
        Ok(content) => StorageRead::Text(content),
        Err(error) if error.kind() == std::io::ErrorKind::NotFound => StorageRead::Missing,
        Err(_) => StorageRead::ReadError,
    }
}

fn epd_error_probe(error: crate::epd::EpdError) -> EpdProbe {
    match error {
        crate::epd::EpdError::BusyTimeout => EpdProbe::BusyTimeout,
        crate::epd::EpdError::Transport => EpdProbe::TransportError,
    }
}

pub fn print_hardware_self_test_report(report: &HardwareSelfTestReport) {
    log::info!(target: "epaper_album", "epaper-album self-test");
    log::info!(target: "epaper_album", "storage: {}", report.base.storage.label());
    log::info!(target: "epaper_album", "config: {}", report.base.config.label());
    log::info!(
        target: "epaper_album",
        "epd: {}",
        report.epd.label()
    );
    log::info!(
        target: "epaper_album",
        "render refresh count: {}",
        report.base.render.refresh_count
    );
    log::info!(
        target: "epaper_album",
        "render sleep: {}",
        report.base.render.slept
    );
}

fn probe_storage(config_read: &StorageRead) -> StorageProbe {
    match config_read {
        StorageRead::MountError => StorageProbe::MountError,
        StorageRead::Text(_) | StorageRead::Missing | StorageRead::ReadError => {
            StorageProbe::Available
        }
    }
}

fn probe_config(config_read: StorageRead) -> ConfigProbe {
    match config_read {
        StorageRead::Text(content) => match toml::from_str::<Config>(&content) {
            Ok(config) if config.has_required_values() => ConfigProbe::Valid,
            Ok(_) => ConfigProbe::Incomplete,
            Err(_) => ConfigProbe::ParseError,
        },
        StorageRead::Missing => ConfigProbe::Missing,
        StorageRead::ReadError => ConfigProbe::ReadError,
        StorageRead::MountError => ConfigProbe::ReadError,
    }
}
