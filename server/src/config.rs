use std::net::SocketAddr;

use anyhow::{anyhow, Context};

#[derive(Debug, Clone)]
pub struct AppConfig {
    pub listen_addr: SocketAddr,
    pub database_url: String,
    pub secret_key: String,
    pub admin_username: String,
    pub admin_password: String,
    pub admin_token: String,
    pub admin_token_expires_at: chrono::DateTime<chrono::Utc>,
}

impl AppConfig {
    pub fn from_env() -> anyhow::Result<Self> {
        Self::from_vars(|name| std::env::var(name))
    }

    fn from_vars(
        mut var: impl FnMut(&str) -> Result<String, std::env::VarError>,
    ) -> anyhow::Result<Self> {
        let production = env_bool("EPAPER_ALBUM_PRODUCTION", &mut var)?;
        let host = match var("LISTEN_HOST") {
            Ok(value) if !value.trim().is_empty() => value,
            Ok(_) | Err(std::env::VarError::NotPresent) => {
                if production {
                    "0.0.0.0".to_string()
                } else {
                    "127.0.0.1".to_string()
                }
            }
            Err(error) => return Err(error).context("failed to read LISTEN_HOST"),
        };
        let port = match var("LISTEN_PORT") {
            Ok(value) => value,
            Err(std::env::VarError::NotPresent) => "3000".to_string(),
            Err(error) => return Err(error).context("failed to read LISTEN_PORT"),
        }
        .parse::<u16>()
        .context("LISTEN_PORT must be a valid TCP port")?;
        let database_url = match var("DATABASE_URL") {
            Ok(value) if !value.trim().is_empty() => value,
            Ok(_) | Err(std::env::VarError::NotPresent) => {
                "sqlite:data/epaper-album.db?mode=rwc".to_string()
            }
            Err(error) => return Err(error).context("failed to read DATABASE_URL"),
        };
        let secret_key = env_required_or_dev_default(
            "SECRET_KEY",
            "local-secret-key",
            production,
            "device secret key",
            &mut var,
        )?;
        let admin_username = env_required_or_dev_default(
            "ADMIN_USERNAME",
            "admin",
            production,
            "admin username",
            &mut var,
        )?;
        let admin_password = env_required_or_dev_default(
            "ADMIN_PASSWORD",
            "admin",
            production,
            "admin password",
            &mut var,
        )?;
        let admin_token = uuid::Uuid::new_v4().to_string();
        let admin_token_expires_at = chrono::Utc::now() + chrono::Duration::hours(24);

        Ok(Self {
            listen_addr: format!("{host}:{port}").parse().with_context(|| {
                format!("LISTEN_HOST/LISTEN_PORT must form a socket address: {host}:{port}")
            })?,
            database_url,
            secret_key,
            admin_username,
            admin_password,
            admin_token,
            admin_token_expires_at,
        })
    }
}

fn env_bool(
    name: &str,
    var: &mut impl FnMut(&str) -> Result<String, std::env::VarError>,
) -> anyhow::Result<bool> {
    match var(name) {
        Ok(value) => match value.trim().to_ascii_lowercase().as_str() {
            "1" | "true" | "yes" | "on" => Ok(true),
            "0" | "false" | "no" | "off" | "" => Ok(false),
            _ => Err(anyhow!("{name} must be true or false")),
        },
        Err(std::env::VarError::NotPresent) => Ok(false),
        Err(error) => Err(error).with_context(|| format!("failed to read {name}")),
    }
}

fn env_required_or_dev_default(
    name: &str,
    dev_default: &str,
    production: bool,
    label: &str,
    var: &mut impl FnMut(&str) -> Result<String, std::env::VarError>,
) -> anyhow::Result<String> {
    match var(name) {
        Ok(value) if production && is_weak_production_value(value.trim(), dev_default) => Err(
            anyhow!("{name} must not use the development default or placeholder in production"),
        ),
        Ok(value) if !value.trim().is_empty() => Ok(value),
        Ok(_) | Err(std::env::VarError::NotPresent) if production => {
            Err(anyhow!("{name} must be set in production for {label}"))
        }
        Ok(_) | Err(std::env::VarError::NotPresent) => Ok(dev_default.to_string()),
        Err(error) => Err(error).with_context(|| format!("failed to read {name}")),
    }
}

fn is_weak_production_value(value: &str, dev_default: &str) -> bool {
    value == dev_default || value.starts_with("change-me") || value.starts_with("replace-")
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::collections::HashMap;

    fn config_from(values: &[(&str, &str)]) -> anyhow::Result<AppConfig> {
        let values = values
            .iter()
            .map(|(key, value)| ((*key).to_string(), (*value).to_string()))
            .collect::<HashMap<_, _>>();
        AppConfig::from_vars(|name| {
            values
                .get(name)
                .cloned()
                .ok_or(std::env::VarError::NotPresent)
        })
    }

    #[test]
    fn development_defaults_bind_loopback() {
        let config = config_from(&[]).expect("development config");

        assert_eq!(config.listen_addr.to_string(), "127.0.0.1:3000");
        assert_eq!(config.secret_key, "local-secret-key");
        assert_eq!(config.admin_username, "admin");
        assert_eq!(config.admin_password, "admin");
    }

    #[test]
    fn production_requires_credentials() {
        let error = config_from(&[("EPAPER_ALBUM_PRODUCTION", "true")])
            .expect_err("production credentials should be required");

        assert!(error.to_string().contains("SECRET_KEY must be set"));
    }

    #[test]
    fn production_rejects_development_defaults_and_placeholders() {
        let default_error = config_from(&[
            ("EPAPER_ALBUM_PRODUCTION", "true"),
            ("SECRET_KEY", "local-secret-key"),
            ("ADMIN_USERNAME", "operator"),
            ("ADMIN_PASSWORD", "strong-password"),
        ])
        .expect_err("production secret should reject development default");
        assert!(default_error
            .to_string()
            .contains("must not use the development default"));

        let placeholder_error = config_from(&[
            ("EPAPER_ALBUM_PRODUCTION", "true"),
            ("SECRET_KEY", "change-me-device-secret"),
            ("ADMIN_USERNAME", "operator"),
            ("ADMIN_PASSWORD", "strong-password"),
        ])
        .expect_err("production secret should reject placeholder");
        assert!(placeholder_error
            .to_string()
            .contains("must not use the development default"));
    }

    #[test]
    fn production_uses_explicit_public_binding_and_credentials() {
        let config = config_from(&[
            ("EPAPER_ALBUM_PRODUCTION", "true"),
            ("SECRET_KEY", "device-secret"),
            ("ADMIN_USERNAME", "operator"),
            ("ADMIN_PASSWORD", "strong-password"),
        ])
        .expect("production config");

        assert_eq!(config.listen_addr.to_string(), "0.0.0.0:3000");
        assert_eq!(config.secret_key, "device-secret");
        assert_eq!(config.admin_username, "operator");
        assert_eq!(config.admin_password, "strong-password");
    }
}
