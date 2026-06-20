use thiserror::Error;

pub type AlbumResult<T> = Result<T, AlbumError>;

#[derive(Debug, Error)]
pub enum AlbumError {
    #[error("configuration error: {0}")]
    Config(String),

    #[error("schedule error: {0}")]
    Schedule(String),

    #[error("cache error: {0}")]
    Cache(String),

    #[error("render error: {0}")]
    Render(String),

    #[error("display error: {0}")]
    Display(String),

    #[error("power error: {0}")]
    Power(String),

    #[error("io error: {0}")]
    Io(#[from] std::io::Error),

    #[error("toml parse error: {0}")]
    Toml(#[from] toml::de::Error),

    #[error("json parse error: {0}")]
    Json(#[from] serde_json::Error),
}
