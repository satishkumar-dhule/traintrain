use std::net::SocketAddr;

use railway_rs::config::Config;
use railway_rs::state::AppState;
use railway_rs::web;

fn init_tracing() {
    use tracing_subscriber::EnvFilter;
    let filter = EnvFilter::try_from_default_env()
        .unwrap_or_else(|_| EnvFilter::new("info,tower_http=info"));
    tracing_subscriber::fmt().with_env_filter(filter).init();
}

#[tokio::main]
async fn main() -> Result<(), Box<dyn std::error::Error>> {
    init_tracing();

    let config = Config::from_env();
    let state = AppState::from_config(config.clone())?;

    let stations = state.datasets.stations.len();
    let trains = state.datasets.trains.len();
    let app = web::router(state, config.static_dir.clone());

    let addr: SocketAddr = format!("0.0.0.0:{}", config.port).parse()?;
    tracing::info!(
        "RailCompanion (railway-rs) serving on {addr} | live sources: railyatri, etrain, NTES | data: {stations} stations, {trains} trains"
    );

    let listener = tokio::net::TcpListener::bind(addr).await?;
    axum::serve(listener, app).await?;
    Ok(())
}
