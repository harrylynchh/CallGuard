use anyhow::Result;
use server::{config::Config, routes, state::AppState};
use tracing_subscriber::EnvFilter;

#[tokio::main]
async fn main() -> Result<()> {
    tracing_subscriber::fmt()
        .with_env_filter(EnvFilter::try_from_default_env().unwrap_or_else(|_| "info".into()))
        .init();

    let config = Config::from_env()?;
    if config.skip_twilio_signature {
        tracing::warn!(
            "SKIP_TWILIO_SIGNATURE=1: Twilio signatures are NOT checked (local testing only)"
        );
    }
    tracing::info!(public_url = %config.public_url, "Twilio webhooks are expected at {}/voice", config.public_url);

    let addr = std::net::SocketAddr::from(([0, 0, 0, 0], config.port));
    let app = routes::router(AppState::new(config));
    let listener = tokio::net::TcpListener::bind(addr).await?;
    tracing::info!(%addr, "CallGuard listening");
    axum::serve(listener, app).await?;
    Ok(())
}
