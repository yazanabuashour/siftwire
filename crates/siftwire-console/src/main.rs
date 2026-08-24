use anyhow::{Context, Result};
use siftwire_console::AppState;
use tracing_subscriber::EnvFilter;

fn main() -> Result<()> {
    let runtime = tokio::runtime::Builder::new_multi_thread()
        .enable_all()
        .build()
        .context("build tokio runtime")?;
    runtime.block_on(serve())
}

async fn serve() -> Result<()> {
    let filter = EnvFilter::try_from_default_env()
        .unwrap_or_else(|_| EnvFilter::new("siftwire_console=info"));
    tracing_subscriber::fmt()
        .with_env_filter(filter)
        .with_writer(std::io::stderr)
        .without_time()
        .init();
    let settings = siftwire_console::Settings::from_env()?;
    let state = AppState {
        settings: settings.clone(),
    };
    let app = siftwire_console::router(state);
    let listener = tokio::net::TcpListener::bind(&settings.bind)
        .await
        .with_context(|| format!("bind {}", settings.bind))?;
    tracing::info!(address = %settings.bind, "siftwire console listening");
    axum::serve(listener, app)
        .with_graceful_shutdown(shutdown_signal())
        .await
        .context("serve console")
}

async fn shutdown_signal() {
    // An errored signal listener simply ends graceful waiting.
    if tokio::signal::ctrl_c().await.is_err() {
        tracing::warn!("ctrl-c listener failed; shutting down anyway");
    }
}
