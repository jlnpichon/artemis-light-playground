use std::{collections::HashMap, sync::Arc};

use alloy::{
    network::TransactionResponse,
    node_bindings::Anvil,
    primitives::{TxHash, U256},
    providers::{Provider, ProviderBuilder},
    rpc::types::{Transaction, TransactionReceipt},
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
        self,
        anvil::AnvilProcess,
        engine::*,
        pools::{PoolMetadata, PoolResolver},
        provider::build_provider_ws,
        sim::{extract_swap_amounts, SimResult},
        telemetry::Telemetry,
        tx::TxStatus,
    },
    decoders::{decode_defi_tx, DefiEvent, UniswapSwap},
};

const TIMEOUT_BLOCKS: u64 = 10;

#[derive(Debug, Clone)]
struct PendingSim {
    swap: UniswapSwap,
    meta: PoolMetadata,
    submitted_block: u64,
    predicted: SimResult,
}

struct SimAccuracyStrategy<P> {
    provider: P,
    current_block: u64,
    rpc_url: String,
    anvil_provider: P,
    resolver: PoolResolver<P>,
    pending: HashMap<TxHash, PendingSim>,
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
impl<P: Provider + Clone> Strategy<Event, ()> for SimAccuracyStrategy<P> {
    async fn sync_state(&mut self) -> Result<()> {
        Ok(())
    }

    async fn process_event(&mut self, event: Event) -> Result<ActionStream<'_, ()>> {
        match event {
            Event::Block(block) => {
                self.current_block = block.number;
                debug!(block_number = block.number, "block advanced");

                let mut mined = Vec::new();
                for (tx_hash, sim) in self.pending.iter() {
                    match self.provider.get_transaction_receipt(*tx_hash).await {
                        Ok(Some(receipt)) if receipt.block_number.is_some() => {
                            mined.push(*tx_hash);

                            if let Some((_, actual_out)) = extract_actual_swap(&receipt, sim) {
                                log_diff(*tx_hash, &sim.swap, &sim.predicted, actual_out);
                            } else {
                                warn!(hash = %tx_hash, "failed to extract actual swap");
                            }
                        }
                        Ok(_) => {} // still pending
                        Err(e) => warn!(?e, "failed to fetch receipt"),
                    }
                }

                for h in mined {
                    self.pending.remove(&h);
                }

                // Evict txns that never got mined, to avoid memory leak.
                self.pending
                    .retain(|_, sim| block.number - sim.submitted_block < TIMEOUT_BLOCKS);
            }
            Event::PendingTx { tx, events } => {
                // Do we have an interesting event in this tx ?
                if !events
                    .iter()
                    .any(|e| matches!(e, DefiEvent::UniswapSwap(_)))
                {
                    return Ok(Box::pin(futures::stream::empty()));
                }

                let tx_hash = tx.tx_hash();
                match common::tx::check_tx_status(&self.provider, tx_hash).await {
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

                    let Some((meta, predicted)) = common::sim::predict_swap(
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
                        "tracking - awaiting for block inclusion"
                    );

                    self.pending.insert(
                        tx_hash,
                        PendingSim {
                            swap,
                            meta,
                            submitted_block: self.current_block,
                            predicted,
                        },
                    );
                }
            }
        };

        Ok(Box::pin(futures::stream::empty()))
    }
}

fn extract_actual_swap(receipt: &TransactionReceipt, pending: &PendingSim) -> Option<(U256, U256)> {
    extract_swap_amounts(
        receipt,
        pending.meta.address,
        pending.meta.token0,
        pending.swap.token_in,
        pending.swap.version,
    )
}

fn log_diff(tx_hash: TxHash, swap: &UniswapSwap, predicted: &SimResult, actual_out: U256) {
    let Some(price_impact) = predicted.price_impact.as_ref() else {
        warn!(hash = %tx_hash, "no predicted price impact available");
        return;
    };
    let predicted_out = price_impact.amount_out;

    // U256 -> i128 : swapped amount should fits in i128
    let (Ok(predicted_i), Ok(actual_i)) =
        (i128::try_from(predicted_out), i128::try_from(actual_out))
    else {
        warn!(hash = %tx_hash, "amount too large for i128 diff, skipping");
        return;
    };

    if predicted_i == 0 {
        warn!(hash = %tx_hash, "predicted_out is zero, cannot compute delta");
        return;
    }

    let delta_pct = (actual_i - predicted_i) as f64 / predicted_i as f64 * 100.0;

    info!(
        hash = %tx_hash,
        token_in = %swap.token_in,
        token_out = %swap.token_out,
        predicted_out = %predicted_out,
        actual_out = %actual_out,
        delta_pct = format!("{delta_pct:+.4}%"),
        "sim accuracy"
    );
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
    engine.add_strategy(Box::new(SimAccuracyStrategy {
        rpc_url,
        anvil_provider,
        provider,
        current_block: 0,
        pending: HashMap::new(),
        resolver: pool_resolver,
    }));
    engine.add_observer(Box::new(telemetry.clone()));

    run_engine(engine, telemetry.events).await
}
