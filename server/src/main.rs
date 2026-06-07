use epaper_album_server::{
    config::AppConfig,
    db::{self, Store},
    routes::{self, AppState},
};
use sqlx::sqlite::SqlitePoolOptions;
use tower_http::cors::CorsLayer;

#[tokio::main]
async fn main() -> anyhow::Result<()> {
    tracing_subscriber::fmt()
        .with_env_filter(
            tracing_subscriber::EnvFilter::try_from_default_env()
                .unwrap_or_else(|_| "epaper_album_server=info,tower_http=info".into()),
        )
        .init();

    let config = AppConfig::from_env()?;
    std::fs::create_dir_all("data")?;

    let pool = SqlitePoolOptions::new()
        .max_connections(5)
        .connect(&config.database_url)
        .await?;
    db::init_schema(&pool).await?;

    let state = AppState {
        store: Store::new(pool),
        secret_key: config.secret_key,
    };
    let app = routes::router(state).layer(CorsLayer::permissive());

    tracing::info!(
        "Epaper Album Server listening on http://{}",
        config.listen_addr
    );
    tracing::info!("Admin UI is served from ./web/dist when built");

    let listener = tokio::net::TcpListener::bind(config.listen_addr).await?;
    axum::serve(listener, app).await?;

    Ok(())
}
