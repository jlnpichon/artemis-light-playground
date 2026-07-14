use std::sync::Arc;

use alloy::rpc::types::Transaction;
use anyhow::Result;
use async_trait::async_trait;

use artemis_light::{
    collector_ext::CollectorExt,
    collectors::MempoolCollector,
    engine::Engine,
    types::{ActionStream, Strategy},
};

use artemis_light_playground::common::{
    engine::{init_tracing, run_engine},
    provider::build_provider_ws,
    telemetry::Telemetry,
    tx::log_pending_tx,
};

#[derive(Debug, Clone)]
enum Event {
    Tx(Transaction),
}

struct PrinterStrategy;

#[async_trait]
impl Strategy<Event, ()> for PrinterStrategy {
    async fn sync_state(&mut self) -> Result<()> {
        Ok(())
    }

    async fn process_event(&mut self, event: Event) -> Result<ActionStream<'_, ()>> {
        let Event::Tx(transaction) = event;
        log_pending_tx(&transaction);

        Ok(Box::pin(futures::stream::empty()))
    }
}

#[tokio::main]
async fn main() -> Result<()> {
    init_tracing();

    let telemetry = Telemetry::new();
    let mut engine = Engine::<Event, ()>::default();
    let provider = Arc::new(build_provider_ws().await?);

    let mpool_collector = MempoolCollector::new(provider.clone()).map(Event::Tx);

    engine.add_collector(Box::new(mpool_collector));
    engine.add_strategy(Box::new(PrinterStrategy));
    engine.add_observer(Box::new(telemetry.clone()));

    run_engine(engine, telemetry.events).await
}
