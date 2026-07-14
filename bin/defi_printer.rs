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

use artemis_playground::{
    common::{engine::*, provider::build_provider_ws, telemetry::Telemetry},
    decoders::{aave::AaveLiquidation, decode_defi_tx, lido::LidoStake, DefiEvent, UniswapSwap},
};

struct PrintSwaps;

#[derive(Debug, Clone)]
enum Event {
    PendingTx { events: Vec<DefiEvent> },
}

#[async_trait]
impl Strategy<Event, ()> for PrintSwaps {
    async fn sync_state(&mut self) -> Result<()> {
        Ok(())
    }

    async fn process_event(&mut self, event: Event) -> Result<ActionStream<'_, ()>> {
        let Event::PendingTx { events, .. } = event;
        events
            .iter()
            .filter_map(DefiEvent::as_swap)
            .for_each(UniswapSwap::log);
        Ok(Box::pin(futures::stream::empty()))
    }
}

struct PrintLiquidation;

#[async_trait]
impl Strategy<Event, ()> for PrintLiquidation {
    async fn sync_state(&mut self) -> Result<()> {
        Ok(())
    }

    async fn process_event(&mut self, event: Event) -> Result<ActionStream<'_, ()>> {
        let Event::PendingTx { events, .. } = event;
        events
            .iter()
            .filter_map(DefiEvent::as_liquidation)
            .for_each(AaveLiquidation::log);
        Ok(Box::pin(futures::stream::empty()))
    }
}

struct PrintStaking;

#[async_trait]
impl Strategy<Event, ()> for PrintStaking {
    async fn sync_state(&mut self) -> Result<()> {
        Ok(())
    }

    async fn process_event(&mut self, event: Event) -> Result<ActionStream<'_, ()>> {
        let Event::PendingTx { events, .. } = event;
        events
            .iter()
            .filter_map(DefiEvent::as_stake)
            .for_each(LidoStake::log);
        Ok(Box::pin(futures::stream::empty()))
    }
}

#[tokio::main]
async fn main() -> Result<()> {
    init_tracing();

    let telemetry = Telemetry::new();
    let mut engine = Engine::<Event, ()>::default();
    let provider = Arc::new(build_provider_ws().await?);

    let mpool_collector = MempoolCollector::new(provider.clone()).filter_map(|tx: Transaction| {
        let events = decode_defi_tx(&tx);
        if events.is_empty() {
            None
        } else {
            Some(Event::PendingTx { events })
        }
    });

    engine.add_collector(Box::new(mpool_collector));
    engine.add_strategy(Box::new(PrintSwaps));
    engine.add_strategy(Box::new(PrintLiquidation));
    engine.add_strategy(Box::new(PrintStaking));
    engine.add_observer(Box::new(telemetry.clone()));

    run_engine(engine, telemetry.events).await
}
