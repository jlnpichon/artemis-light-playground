use std::sync::Arc;

use alloy::{
    network::TransactionResponse,
    node_bindings::Anvil,
    providers::{Provider, ProviderBuilder},
    rpc::types::Transaction,
};
use anyhow::{Context, Result};
use artemis_light::{
    collector_ext::CollectorExt,
    collectors::{BlockCollector, MempoolCollector, NewBlock},
    engine::Engine,
    types::{ActionStream, Strategy},
};
use async_trait::async_trait;
use tracing::{debug, info, warn};

use artemis_playground::{
    common::{
        self, anvil::AnvilProcess, engine::*, pools::PoolResolver, provider::build_provider_ws,
        telemetry::Telemetry, tx::TxStatus,
    },
    decoders::{decode_defi_tx, DefiEvent},
};

struct TxSimStrategy<P> {
    provider: P,
    current_block: u64,
    rpc_url: String,
    anvil_provider: P,
    resolver: PoolResolver<P>,
}

#[derive(Debug, Clone)]
pub enum Event {
    PendingTx {
        tx: Box<Transaction>,
        events: Vec<DefiEvent>,
    },
    Block(NewBlock),
}

#[async_trait]
impl<P: Provider + Clone> Strategy<Event, ()> for TxSimStrategy<P> {
    async fn sync_state(&mut self) -> Result<()> {
        Ok(())
    }

    async fn process_event(&mut self, event: Event) -> Result<ActionStream<'_, ()>> {
        match event {
            Event::Block(block) => {
                self.current_block = block.number;
                debug!(block_number = block.number, "block advanced");
            }
            Event::PendingTx { tx, events } => {
                // Do we have an interesting event in this tx ?
                if !events
                    .iter()
                    .any(|e| matches!(e, DefiEvent::UniswapSwap(_)))
                {
                    return Ok(Box::pin(futures::stream::empty()));
                }

                match common::tx::check_tx_status(&self.provider, tx.tx_hash()).await {
                    Ok(TxStatus::Dropped) => {
                        info!("tx dropped from mempool");
                        return Ok(Box::pin(futures::stream::empty()));
                    }
                    Ok(TxStatus::Mined) => {
                        info!("tx already mined");
                        return Ok(Box::pin(futures::stream::empty()));
                    }
                    Ok(TxStatus::Pending) => {}
                    Err(e) => {
                        warn!(?e, "failed to fetch transaction");
                        return Ok(Box::pin(futures::stream::empty()));
                    }
                };

                for event in events {
                    let DefiEvent::UniswapSwap(swap) = event else {
                        continue;
                    };

                    swap.log();

                    let Some((_, predicted)) = common::sim::predict_swap(
                        &self.resolver,
                        &self.anvil_provider,
                        &self.rpc_url,
                        self.current_block,
                        &tx,
                        &swap,
                    )
                    .await
                    else {
                        continue;
                    };

                    info!(
                        status = %predicted.status_str(),
                        gas = %predicted.gas_summary(),
                        reserves = %predicted.delta_summary(),
                        impact = %predicted.impact_str(),
                        error = ?predicted.error,
                        "simulated swap"
                    );
                }
            }
        };

        Ok(Box::pin(futures::stream::empty()))
    }
}

#[tokio::main]
async fn main() -> Result<()> {
    init_tracing();

    let rpc_url = std::env::var("ETH_RPC_URL").context("failed to read ETH_RPC_URL env var")?;
    let anvil = AnvilProcess::new(
        Anvil::new()
            .fork(rpc_url.clone())
            .try_spawn()
            .context("failed to spawn anvil node")?,
    );
    let anvil_provider = ProviderBuilder::new().connect_http(anvil.endpoint_url());

    let telemetry = Telemetry::new();
    let mut engine = Engine::<Event, ()>::default();
    let provider = build_provider_ws().await?;
    let pool_resolver = PoolResolver::new(provider.clone());

    let arc_provider = Arc::new(provider.clone());
    let block_collector = BlockCollector::new(arc_provider.clone()).map(Event::Block);
    let tx_collector = MempoolCollector::new(arc_provider.clone()).filter_map(|tx: Transaction| {
        let events = decode_defi_tx(&tx);
        if events.is_empty() {
            None
        } else {
            Some(Event::PendingTx {
                tx: Box::new(tx),
                events,
            })
        }
    });
    let merged = tx_collector.merge(block_collector);

    engine.add_collector(Box::new(merged));
    engine.add_strategy(Box::new(TxSimStrategy {
        rpc_url,
        anvil_provider,
        provider,
        current_block: 0,
        resolver: pool_resolver,
    }));
    engine.add_observer(Box::new(telemetry.clone()));

    run_engine(engine, telemetry.events).await
}
