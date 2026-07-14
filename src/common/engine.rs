use std::fmt::Debug;
use std::sync::atomic::{AtomicU64, Ordering};
use std::sync::Arc;

use anyhow::{Context, Result};
use tracing::{error, info};

use artemis_light::engine::Engine;

/// Initialise tracing and load environment variables from `.env`.
///
/// Must be called once at the start of every binary.
pub fn init_tracing() {
    dotenvy::dotenv().ok();
    tracing_subscriber::fmt()
        .with_env_filter(tracing_subscriber::filter::EnvFilter::from_default_env())
        .init();
}

/// Run the artemis-light engine and wait for Ctrl+C or a fatal error.
///
/// Prints telemetry (total events processed) on shutdown.
pub async fn run_engine<E: Clone + Debug + Send + 'static, A: Clone + Debug + Send + 'static>(
    engine: Engine<E, A>,
    events: Arc<AtomicU64>,
) -> Result<()> {
    let mut handle = engine.run().await.context("failed to run engine")?;
    let fatal = tokio::select! {
        _ = tokio::signal::ctrl_c() => false,
        _ = handle.fatal.cancelled() => true,
    };
    handle.token.cancel();
    while handle.tasks.join_next().await.is_some() {}

    info!(events = events.load(Ordering::Relaxed), "telemetry");

    if fatal {
        error!("An unrecoverable error occurred");
    }

    Ok(())
}
