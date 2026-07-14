use std::sync::Arc;

use alloy::{providers::Provider, rpc::types::Transaction};
use anyhow::Result;
use async_trait::async_trait;
use tracing::info;

use artemis_light::{
    collector_ext::CollectorExt,
    collectors::MempoolCollector,
    engine::Engine,
    types::{ActionStream, Strategy},
};

use artemis_playground::{
    common::{
        engine::*,
        pools::{compute_price_impact, PoolResolver, Wad, WadPercent},
        provider::build_provider_ws,
        telemetry::Telemetry,
        tx::short_addr,
    },
    decoders::{decode_defi_tx, DefiEvent},
};

#[derive(Debug, Clone)]
pub enum Event {
    PendingTx { events: Vec<DefiEvent> },
}

struct PriceImpactStrategy<P> {
    provider: P,
    resolver: PoolResolver<P>,
}

#[async_trait]
impl<P: Provider + Clone> Strategy<Event, ()> for PriceImpactStrategy<P> {
    async fn sync_state(&mut self) -> Result<()> {
        Ok(())
    }

    async fn process_event(&mut self, event: Event) -> Result<ActionStream<'_, ()>> {
        let Event::PendingTx { events } = event;

        for event in events {
            if let DefiEvent::UniswapSwap(swap) = event {
                let Some(meta) = self.resolver.resolve(&swap).await else {
                    return Ok(Box::pin(futures::stream::empty()));
                };

                if let Some(impact) = compute_price_impact(&self.provider, &swap, &meta).await {
                    info!(
                        pool = %short_addr(meta.address),
                        spot = %Wad(impact.spot_price),
                        execution = %Wad(impact.execution_price),
                        impact = %WadPercent(impact.price_impact),
                        "price impact"
                    );
                }
            }
        }

        Ok(Box::pin(futures::stream::empty()))
    }
}

#[tokio::main]
async fn main() -> Result<()> {
    init_tracing();

    let telemetry = Telemetry::new();
    let mut engine = Engine::<Event, ()>::default();
    let provider = Arc::new(build_provider_ws().await?);
    let pool_resolver = PoolResolver::new(provider.clone());

    let mpool_collector = MempoolCollector::new(provider.clone()).filter_map(|tx: Transaction| {
        let events = decode_defi_tx(&tx);
        if events.is_empty() {
            None
        } else {
            Some(Event::PendingTx { events })
        }
    });

    engine.add_collector(Box::new(mpool_collector));
    engine.add_strategy(Box::new(PriceImpactStrategy {
        provider: provider.clone(),
        resolver: pool_resolver,
    }));
    engine.add_observer(Box::new(telemetry.clone()));

    run_engine(engine, telemetry.events).await
}
