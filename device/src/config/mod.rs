use serde::{Deserialize, Serialize};

pub const CONFIG_PATH: &str = "/sdcard/config.toml";

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct Config {
    pub wifi_ssid: String,
    pub wifi_password: String,
    pub base_url: String,
    #[serde(rename = "secret-key")]
    pub secret_key: String,
}

impl Config {
    pub fn has_required_values(&self) -> bool {
        !self.wifi_ssid.trim().is_empty()
            && !self.wifi_password.trim().is_empty()
            && !self.base_url.trim().is_empty()
            && !self.secret_key.trim().is_empty()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn deserializes_secret_key_from_toml_field_name() {
        let config: Config = toml::from_str(
            r#"
wifi_ssid = "Home WiFi"
wifi_password = "password"
base_url = "https://example.com/epaper"
secret-key = "local-secret-key"
"#,
        )
        .unwrap();

        assert_eq!(config.secret_key, "local-secret-key");
    }

    #[test]
    fn detects_blank_required_values() {
        let config = Config {
            wifi_ssid: "Home WiFi".to_string(),
            wifi_password: "password".to_string(),
            base_url: " ".to_string(),
            secret_key: "local-secret-key".to_string(),
        };

        assert!(!config.has_required_values());
    }
}
