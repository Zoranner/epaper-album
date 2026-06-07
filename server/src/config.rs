use std::net::SocketAddr;

#[derive(Debug, Clone)]
pub struct AppConfig {
    pub listen_addr: SocketAddr,
    pub database_url: String,
    pub secret_key: String,
}

impl AppConfig {
    pub fn from_env() -> anyhow::Result<Self> {
        let port = std::env::var("LISTEN_PORT")
            .ok()
            .and_then(|value| value.parse().ok())
            .unwrap_or(3000);
        let database_url = std::env::var("DATABASE_URL")
            .unwrap_or_else(|_| "sqlite:data/epaper-album.db?mode=rwc".to_string());
        let secret_key = std::env::var("EPAPER_ALBUM_SECRET_KEY")
            .unwrap_or_else(|_| "local-secret-key".to_string());

        Ok(Self {
            listen_addr: SocketAddr::from(([0, 0, 0, 0], port)),
            database_url,
            secret_key,
        })
    }
}
