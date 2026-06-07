use std::net::SocketAddr;

#[derive(Debug, Clone)]
pub struct AppConfig {
    pub listen_addr: SocketAddr,
    pub database_url: String,
    pub secret_key: String,
    pub admin_username: String,
    pub admin_password: String,
    pub admin_token: String,
}

impl AppConfig {
    pub fn from_env() -> anyhow::Result<Self> {
        let port = std::env::var("LISTEN_PORT")
            .ok()
            .and_then(|value| value.parse().ok())
            .unwrap_or(3000);
        let database_url = std::env::var("DATABASE_URL")
            .unwrap_or_else(|_| "sqlite:data/epaper-album.db?mode=rwc".to_string());
        let secret_key =
            std::env::var("SECRET_KEY").unwrap_or_else(|_| "local-secret-key".to_string());
        let admin_username =
            std::env::var("ADMIN_USERNAME").unwrap_or_else(|_| "admin".to_string());
        let admin_password =
            std::env::var("ADMIN_PASSWORD").unwrap_or_else(|_| "admin".to_string());
        let admin_token = uuid::Uuid::new_v4().to_string();

        Ok(Self {
            listen_addr: SocketAddr::from(([0, 0, 0, 0], port)),
            database_url,
            secret_key,
            admin_username,
            admin_password,
            admin_token,
        })
    }
}
