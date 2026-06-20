use crate::cloud::espidf::EspIdfHttpClient;
use crate::config::Config;
use crate::device_runtime::{DeviceCloudSync, SyncErrorReport, SyncRequest, SyncResult};
use crate::device_sync::{CloudResourceSync, DeviceSyncError};
use crate::storage::MountedSdCardResourceStore;
use crate::wifi::espidf::{connect_wifi, ConnectedWifi, WifiConnectError};
use core::time::Duration;

pub struct EspDeviceCloudSync {
    modem: Option<esp_idf_svc::hal::modem::Modem<'static>>,
    wifi: Option<ConnectedWifi>,
    sntp: Option<esp_idf_svc::sntp::EspSntp<'static>>,
    time_synced: bool,
    inner: CloudResourceSync<EspIdfHttpClient, MountedSdCardResourceStore>,
}

impl EspDeviceCloudSync {
    pub fn new(modem: esp_idf_svc::hal::modem::Modem<'static>) -> Self {
        Self {
            modem: Some(modem),
            wifi: None,
            sntp: None,
            time_synced: false,
            inner: CloudResourceSync::new(EspIdfHttpClient, MountedSdCardResourceStore),
        }
    }

    pub const fn time_synced(&self) -> bool {
        self.time_synced
    }

    pub fn prepare_network(&mut self, config: &Config) {
        if self.wifi.is_none() {
            let Some(modem) = self.modem.take() else {
                log::warn!(target: "epaper_album", "wifi: modem-unavailable");
                return;
            };

            match connect_wifi(modem, config) {
                Ok(wifi) => {
                    self.wifi = Some(wifi);
                }
                Err(error) => {
                    log::warn!(target: "epaper_album", "wifi: {error:?}");
                    return;
                }
            }
        }

        self.sync_time();
    }

    fn sync_time(&mut self) {
        if self.time_synced {
            return;
        }

        if self.sntp.is_none() {
            match esp_idf_svc::sntp::EspSntp::new_default() {
                Ok(sntp) => {
                    self.sntp = Some(sntp);
                    log::info!(target: "epaper_album", "sntp: started");
                }
                Err(error) => {
                    log::warn!(target: "epaper_album", "sntp: init-error: {error:?}");
                    return;
                }
            }
        }

        let Some(sntp) = self.sntp.as_ref() else {
            return;
        };

        for _ in 0..20 {
            if sntp.get_sync_status() == esp_idf_svc::sntp::SyncStatus::Completed {
                log::info!(target: "epaper_album", "sntp: completed");
                self.time_synced = true;
                return;
            }
            std::thread::sleep(Duration::from_millis(500));
        }

        log::warn!(
            target: "epaper_album",
            "sntp: timeout status={:?}",
            sntp.get_sync_status()
        );
    }
}

#[derive(Debug)]
pub enum EspDeviceSyncError {
    Wifi(WifiConnectError),
    Sync(DeviceSyncError),
}

impl core::fmt::Display for EspDeviceSyncError {
    fn fmt(&self, formatter: &mut core::fmt::Formatter<'_>) -> core::fmt::Result {
        match self {
            Self::Wifi(error) => write!(formatter, "wifi: {error:?}"),
            Self::Sync(error) => write!(formatter, "{error}"),
        }
    }
}

impl EspDeviceSyncError {
    fn code(&self) -> String {
        match self {
            Self::Wifi(error) => format!("wifi.{}", wifi_error_code(*error)),
            Self::Sync(error) => error.code(),
        }
    }

    const fn category(&self) -> &'static str {
        match self {
            Self::Wifi(_) => "wifi",
            Self::Sync(error) => error.category(),
        }
    }

    fn stage(&self) -> Option<String> {
        match self {
            Self::Wifi(_) => Some("wifi".to_string()),
            Self::Sync(error) => Some(error.stage().to_string()),
        }
    }

    fn message(&self) -> String {
        match self {
            Self::Wifi(_) => "wifi connection failed".to_string(),
            Self::Sync(error) => error.message(),
        }
    }

    fn detail(&self) -> String {
        match self {
            Self::Wifi(error) => format!("{error:?}"),
            Self::Sync(error) => error.detail().unwrap_or_else(|| error.to_string()),
        }
    }
}

const fn wifi_error_code(error: WifiConnectError) -> &'static str {
    match error {
        WifiConnectError::InitError => "init",
        WifiConnectError::ConfigError => "config",
        WifiConnectError::StartError => "start",
        WifiConnectError::ScanError => "scan",
        WifiConnectError::TargetNotFound => "target-not-found",
        WifiConnectError::ConnectError => "connect",
        WifiConnectError::NetifError => "netif",
    }
}

impl DeviceCloudSync for EspDeviceCloudSync {
    type Error = EspDeviceSyncError;

    fn sync_resources(&mut self, request: SyncRequest) -> Result<SyncResult, Self::Error> {
        self.prepare_network(&request.config);
        if self.wifi.is_none() {
            return Err(EspDeviceSyncError::Wifi(WifiConnectError::InitError));
        }
        log::info!(
            target: "epaper_album",
            "sync: network ready date={}",
            request.date
        );
        if let Some(wifi) = self.wifi.as_ref() {
            match wifi.ip_info() {
                Ok(ip_info) => log::info!(
                    target: "epaper_album",
                    "sync: ip={} netmask={} dns={:?} secondary-dns={:?}",
                    ip_info.ip,
                    ip_info.subnet.mask,
                    ip_info.dns,
                    ip_info.secondary_dns
                ),
                Err(error) => log::warn!(target: "epaper_album", "sync: ip-info error: {error:?}"),
            }
        }

        self.inner
            .sync_resources(request)
            .map_err(EspDeviceSyncError::Sync)
    }

    fn describe_error(&self, error: &Self::Error) -> SyncErrorReport {
        SyncErrorReport::new(
            error.code(),
            error.category(),
            error.stage(),
            error.message(),
            error.detail(),
        )
    }
}
