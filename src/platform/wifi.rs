#[cfg(target_os = "espidf")]
pub mod espidf {
    use core::convert::TryInto;
    use core::time::Duration;

    use crate::config::Config;
    use esp_idf_svc::eventloop::EspSystemEventLoop;
    use esp_idf_svc::hal::modem::Modem;
    use esp_idf_svc::http::client::{Configuration as HttpConfiguration, EspHttpConnection};
    use esp_idf_svc::http::Method;
    use esp_idf_svc::nvs::EspDefaultNvsPartition;
    use esp_idf_svc::wifi::{
        AccessPointInfo, AuthMethod, BlockingWifi, ClientConfiguration, Configuration, EspWifi,
    };

    const TEST_HTTP_URL: &str = "http://example.com/";

    #[derive(Clone, Copy, Debug, Eq, PartialEq)]
    pub enum WifiProbe {
        Connected,
        Skipped,
        InitError,
        ConfigError,
        StartError,
        ScanError,
        TargetNotFound,
        ConnectError,
        NetifError,
    }

    impl WifiProbe {
        pub const fn label(self) -> &'static str {
            match self {
                Self::Connected => "connected",
                Self::Skipped => "skipped",
                Self::InitError => "init-error",
                Self::ConfigError => "config-error",
                Self::StartError => "start-error",
                Self::ScanError => "scan-error",
                Self::TargetNotFound => "target-not-found",
                Self::ConnectError => "connect-error",
                Self::NetifError => "netif-error",
            }
        }
    }

    #[derive(Clone, Copy, Debug, Eq, PartialEq)]
    pub enum WifiConnectError {
        InitError,
        ConfigError,
        StartError,
        ScanError,
        TargetNotFound,
        ConnectError,
        NetifError,
    }

    pub struct ConnectedWifi {
        wifi: BlockingWifi<EspWifi<'static>>,
    }

    impl ConnectedWifi {
        pub fn ip_info(&self) -> Result<esp_idf_svc::ipv4::IpInfo, esp_idf_svc::sys::EspError> {
            self.wifi.wifi().sta_netif().get_ip_info()
        }
    }

    #[derive(Clone, Copy, Debug, Eq, PartialEq)]
    pub enum HttpProbe {
        Skipped,
        Fetched,
        ClientError,
        RequestError,
        ResponseError,
        ReadError,
        StatusError,
    }

    impl HttpProbe {
        pub const fn label(self) -> &'static str {
            match self {
                Self::Skipped => "skipped",
                Self::Fetched => "fetched",
                Self::ClientError => "client-error",
                Self::RequestError => "request-error",
                Self::ResponseError => "response-error",
                Self::ReadError => "read-error",
                Self::StatusError => "status-error",
            }
        }
    }

    #[derive(Clone, Copy, Debug, Eq, PartialEq)]
    pub struct NetworkProbe {
        pub wifi: WifiProbe,
        pub http: HttpProbe,
    }

    pub fn probe_test_network(modem: Modem<'static>, config: Option<&Config>) -> NetworkProbe {
        let Some(config) = config else {
            return network_probe(WifiProbe::Skipped, HttpProbe::Skipped);
        };

        let wifi_ssid = config.wifi_ssid.trim();
        let wifi_password = config.wifi_password.trim();
        if wifi_ssid.is_empty() || wifi_password.is_empty() {
            return network_probe(WifiProbe::Skipped, HttpProbe::Skipped);
        }

        let connected = match connect_wifi(modem, config) {
            Ok(connected) => connected,
            Err(error) => return network_probe(wifi_probe_from_error(error), HttpProbe::Skipped),
        };

        let wifi_probe = match connected.ip_info() {
            Ok(ip_info) => {
                log::info!(target: "epaper_album", "wifi ip: {:?}", ip_info);
                WifiProbe::Connected
            }
            Err(_) => return network_probe(WifiProbe::NetifError, HttpProbe::Skipped),
        };
        let http_probe = probe_test_http();

        network_probe(wifi_probe, http_probe)
    }

    pub fn connect_wifi(
        modem: Modem<'static>,
        config: &Config,
    ) -> Result<ConnectedWifi, WifiConnectError> {
        let wifi_ssid = config.wifi_ssid.trim();
        let wifi_password = config.wifi_password.trim();
        if wifi_ssid.is_empty() || wifi_password.is_empty() {
            return Err(WifiConnectError::ConfigError);
        }

        let sys_loop = EspSystemEventLoop::take().map_err(|_| WifiConnectError::InitError)?;
        let nvs = EspDefaultNvsPartition::take().map_err(|_| WifiConnectError::InitError)?;
        let esp_wifi = EspWifi::new(modem, sys_loop.clone(), Some(nvs))
            .map_err(|_| WifiConnectError::InitError)?;
        let mut wifi =
            BlockingWifi::wrap(esp_wifi, sys_loop).map_err(|_| WifiConnectError::InitError)?;

        let configuration = Configuration::Client(ClientConfiguration {
            ssid: wifi_ssid
                .try_into()
                .map_err(|_| WifiConnectError::ConfigError)?,
            bssid: None,
            auth_method: AuthMethod::WPA2Personal,
            password: wifi_password
                .try_into()
                .map_err(|_| WifiConnectError::ConfigError)?,
            channel: None,
            ..Default::default()
        });

        wifi.set_configuration(&configuration)
            .map_err(|_| WifiConnectError::ConfigError)?;
        wifi.start().map_err(|_| WifiConnectError::StartError)?;

        match find_test_access_point(&mut wifi, wifi_ssid) {
            Ok(Some(ap)) => {
                log::info!(
                    target: "epaper_album",
                    "wifi target: found channel={} rssi={} auth={:?}",
                    ap.channel,
                    ap.signal_strength,
                    ap.auth_method
                );
            }
            Ok(None) => {
                log::warn!(target: "epaper_album", "wifi target: not-found");
                return Err(WifiConnectError::TargetNotFound);
            }
            Err(_) => return Err(WifiConnectError::ScanError),
        }

        wifi.connect().map_err(|_| WifiConnectError::ConnectError)?;
        wifi.wait_netif_up()
            .map_err(|_| WifiConnectError::NetifError)?;

        Ok(ConnectedWifi { wifi })
    }

    const fn wifi_probe_from_error(error: WifiConnectError) -> WifiProbe {
        match error {
            WifiConnectError::InitError => WifiProbe::InitError,
            WifiConnectError::ConfigError => WifiProbe::ConfigError,
            WifiConnectError::StartError => WifiProbe::StartError,
            WifiConnectError::ScanError => WifiProbe::ScanError,
            WifiConnectError::TargetNotFound => WifiProbe::TargetNotFound,
            WifiConnectError::ConnectError => WifiProbe::ConnectError,
            WifiConnectError::NetifError => WifiProbe::NetifError,
        }
    }

    fn find_test_access_point(
        wifi: &mut BlockingWifi<EspWifi<'static>>,
        wifi_ssid: &str,
    ) -> Result<Option<AccessPointInfo>, esp_idf_svc::sys::EspError> {
        let access_points = wifi.scan()?;
        log::info!(
            target: "epaper_album",
            "wifi scan: {} access points",
            access_points.len()
        );

        Ok(access_points
            .into_iter()
            .find(|access_point| access_point.ssid.as_str() == wifi_ssid))
    }

    fn probe_test_http() -> HttpProbe {
        let connection = match EspHttpConnection::new(&HttpConfiguration {
            timeout: Some(Duration::from_secs(10)),
            ..Default::default()
        }) {
            Ok(connection) => connection,
            Err(_) => return HttpProbe::ClientError,
        };
        let mut connection = connection;
        if connection
            .initiate_request(Method::Get, TEST_HTTP_URL, &[])
            .is_err()
        {
            return HttpProbe::RequestError;
        }
        if connection.initiate_response().is_err() {
            return HttpProbe::ResponseError;
        }

        let status = connection.status();
        let mut total_bytes = 0usize;
        let mut preview = [0u8; 64];
        let mut preview_len = 0usize;
        let mut buffer = [0u8; 128];

        loop {
            match connection.read(&mut buffer) {
                Ok(0) => break,
                Ok(read_len) => {
                    let copy_len = read_len.min(preview.len().saturating_sub(preview_len));
                    if copy_len > 0 {
                        preview[preview_len..preview_len + copy_len]
                            .copy_from_slice(&buffer[..copy_len]);
                        preview_len += copy_len;
                    }
                    total_bytes = total_bytes.saturating_add(read_len);
                }
                Err(_) => return HttpProbe::ReadError,
            }
        }

        log::info!(
            target: "epaper_album",
            "http get: status={} bytes={} url={}",
            status,
            total_bytes,
            TEST_HTTP_URL
        );
        log::info!(
            target: "epaper_album",
            "http preview: {}",
            core::str::from_utf8(&preview[..preview_len]).unwrap_or("<non-utf8>")
        );

        if status == 200 && total_bytes > 0 {
            HttpProbe::Fetched
        } else {
            HttpProbe::StatusError
        }
    }

    const fn network_probe(wifi: WifiProbe, http: HttpProbe) -> NetworkProbe {
        NetworkProbe { wifi, http }
    }
}
