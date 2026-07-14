use std::time::Duration;

use alloy::{
    eips::Encodable2718,
    primitives::{aliases::U112, Address, I256, U256},
    providers::{ext::AnvilApi, Provider},
    rpc::types::{anvil::Forking, Transaction, TransactionReceipt},
    sol_types::SolEvent,
};
use anyhow::Result;
use tracing::{debug, error, info, trace, warn};

use crate::{
    common::{
        format::format_si,
        pools::{
            sqrt_price_x96_to_wad, IUniswapV2Pair, IUniswapV3Pool, PoolMetadata, PoolResolver,
            PriceImpact, Reserves, WadPercent, WAD,
        },
    },
    decoders::{UniswapSwap, UniswapVersion},
};

/// The result of simulating a single transaction on a forked Anvil node.
#[derive(Debug, Clone)]
pub struct SimResult {
    pub success: bool,
    pub gas_used: u64,
    pub reserves_before: Reserves,
    pub reserves_after: Reserves,
    pub price_impact: Option<PriceImpact>,
    pub error: Option<String>,
}

impl Default for SimResult {
    fn default() -> Self {
        Self {
            success: false,
            gas_used: 0,
            reserves_before: Reserves::V2 {
                reserve0: U112::ZERO,
                reserve1: U112::ZERO,
            },
            reserves_after: Reserves::V2 {
                reserve0: U112::ZERO,
                reserve1: U112::ZERO,
            },
            price_impact: None,
            error: None,
        }
    }
}

impl SimResult {
    pub fn status_str(&self) -> &str {
        if self.success {
            "OK"
        } else {
            "FAIL"
        }
    }

    pub fn gas_summary(&self) -> String {
        format_si(U256::from(self.gas_used))
    }

    pub fn impact_str(&self) -> String {
        match &self.price_impact {
            Some(pi) => WadPercent(pi.price_impact).to_string(),
            None => "N/A".into(),
        }
    }

    pub fn delta_summary(&self) -> String {
        match (&self.reserves_before, &self.reserves_after) {
            (
                Reserves::V2 {
                    reserve0: b0,
                    reserve1: b1,
                },
                Reserves::V2 {
                    reserve0: a0,
                    reserve1: a1,
                },
            ) => format!("r0:{}->{}  r1:{}->{}", b0, a0, b1, a1,),
            (
                Reserves::V3 {
                    sqrt_price_x96: sp_b,
                    liquidity: l_b,
                },
                Reserves::V3 {
                    sqrt_price_x96: sp_a,
                    liquidity: l_a,
                },
            ) => format!("sqrtP:{}->{}  liq:{}->{}", sp_b, sp_a, l_b, l_a,),
            _ => String::new(),
        }
    }
}

async fn fetch_reserves(
    anvil_provider: &impl Provider,
    meta: &PoolMetadata,
    version: UniswapVersion,
) -> Result<Reserves> {
    match version {
        UniswapVersion::V2 => {
            let pair = IUniswapV2Pair::new(meta.address, anvil_provider);
            let r = pair.getReserves().call().await?;
            Ok(Reserves::V2 {
                reserve0: r.reserve0,
                reserve1: r.reserve1,
            })
        }
        UniswapVersion::V3 => {
            let pool = IUniswapV3Pool::new(meta.address, anvil_provider);
            let slot0 = pool.slot0().call().await?;
            let liquidity = pool.liquidity().call().await?;
            Ok(Reserves::V3 {
                sqrt_price_x96: slot0.sqrtPriceX96,
                liquidity,
            })
        }
    }
}

pub fn extract_swap_amounts(
    receipt: &TransactionReceipt,
    pool: Address,
    token0: Address,
    token_in: Address,
    version: UniswapVersion,
) -> Option<(U256, U256)> {
    let sig_hashes = match version {
        UniswapVersion::V2 => IUniswapV2Pair::Swap::SIGNATURE_HASH,
        UniswapVersion::V3 => IUniswapV3Pool::Swap::SIGNATURE_HASH,
    };

    let logs = receipt.inner.logs();
    let rpc_log = logs
        .iter()
        .find(|log| log.address() == pool && log.topic0() == Some(&sig_hashes))?;

    match version {
        UniswapVersion::V2 => {
            let decoded = IUniswapV2Pair::Swap::decode_log(&rpc_log.inner);

            match decoded {
                Ok(event) => {
                    let (in_, out) = if token_in == token0 {
                        (event.amount0In, event.amount1Out)
                    } else {
                        (event.amount1In, event.amount0Out)
                    };
                    trace!("extract_swap v2: in={in_} out={out}");
                    Some((in_, out))
                }
                Err(e) => {
                    trace!("extract_swap v2 decode failed: {e}");
                    None
                }
            }
        }
        UniswapVersion::V3 => {
            let decoded = IUniswapV3Pool::Swap::decode_log(&rpc_log.inner);

            match decoded {
                Ok(event) => {
                    let (in_, out) = if event.amount0 > I256::ZERO {
                        (event.amount0.unsigned_abs(), event.amount1.unsigned_abs())
                    } else {
                        (event.amount1.unsigned_abs(), event.amount0.unsigned_abs())
                    };
                    trace!("extract_swap v3: in={in_} out={out}");
                    Some((in_, out))
                }
                Err(e) => {
                    debug!("extract_swap v3 decode failed: {e}");
                    None
                }
            }
        }
    }
}

fn compute_price_impact_from_swap(
    before: &Reserves,
    meta: &PoolMetadata,
    token_in: Address,
    version: UniswapVersion,
    actual_in: U256,
    actual_out: U256,
) -> Option<PriceImpact> {
    let spot = match version {
        UniswapVersion::V2 => {
            let Reserves::V2 { reserve0, reserve1 } = before else {
                return None;
            };
            if token_in == meta.token0 {
                U256::from(*reserve1)
                    .checked_mul(WAD)?
                    .checked_div(U256::from(*reserve0))?
            } else {
                U256::from(*reserve0)
                    .checked_mul(WAD)?
                    .checked_div(U256::from(*reserve1))?
            }
        }
        UniswapVersion::V3 => {
            let Reserves::V3 { sqrt_price_x96, .. } = before else {
                return None;
            };
            sqrt_price_x96_to_wad(*sqrt_price_x96, token_in == meta.token0)?
        }
    };

    PriceImpact::from_amounts(actual_in, actual_out, spot)
}

async fn try_simulate_raw(
    provider: &impl Provider,
    tx: &Transaction,
) -> Result<TransactionReceipt> {
    let mut bytes = Vec::new();
    tx.inner.encode_2718(&mut bytes);
    let pending = provider.send_raw_transaction(&bytes).await.map_err(|e| {
        error!("send_raw_transaction failed: {e}");
        e
    })?;
    let receipt = tokio::time::timeout(Duration::from_secs(30), pending.get_receipt())
        .await
        .map_err(|_| anyhow::anyhow!("simulation timed out after 30s"))??;
    Ok(receipt)
}

/// Simulate a transaction on a forked Anvil node and return the result.
///
/// Steps:
/// 1. Reset the Anvil fork to the current block height if needed.
/// 2. Snapshot the state.
/// 3. Fetch pre-simulation reserves.
/// 4. Submit and mine the transaction.
/// 5. Fetch post-simulation reserves and extract swap amounts from logs.
/// 6. Compute price impact.
/// 7. Revert to the snapshot (discarding the simulated tx).
async fn simulate_raw(
    current_block: u64,
    anvil_provider: &impl Provider,
    rpc_url: &str,
    tx: &Transaction,
    meta: &PoolMetadata,
    token_in: Address,
    version: UniswapVersion,
) -> Result<SimResult> {
    // Sync anvil to the most recent block if needed.
    if current_block > anvil_provider.get_block_number().await? {
        anvil_provider
            .anvil_reset(Some(Forking {
                json_rpc_url: Some(rpc_url.to_string()),
                block_number: Some(current_block),
            }))
            .await?;
    }

    // Take a snapshot before THIS transaction.
    let snap_id = anvil_provider.anvil_snapshot().await?;

    let before = fetch_reserves(anvil_provider, meta, version).await?;

    let result = match try_simulate_raw(anvil_provider, tx).await {
        Ok(receipt) => {
            let after = fetch_reserves(anvil_provider, meta, version).await?;

            let success = receipt.status();
            let gas_used = receipt.gas_used;
            let price_impact = if success {
                extract_swap_amounts(&receipt, meta.address, meta.token0, token_in, version)
                    .and_then(|(actual_in, actual_out)| {
                        compute_price_impact_from_swap(
                            &before, meta, token_in, version, actual_in, actual_out,
                        )
                    })
            } else {
                None
            };

            SimResult {
                success,
                gas_used,
                reserves_before: before,
                reserves_after: after,
                price_impact,
                error: if success { None } else { Some("revert".into()) },
            }
        }
        Err(e) => SimResult {
            error: Some(e.to_string()),
            reserves_before: before,
            ..Default::default()
        },
    };

    // "Cancel" the transaction as it never happened
    anvil_provider.anvil_revert(snap_id).await?;

    Ok(result)
}

pub async fn predict_swap<P: Provider + Clone>(
    resolver: &PoolResolver<P>,
    anvil_provider: &P,
    rpc_url: &str,
    current_block: u64,
    tx: &Transaction,
    swap: &UniswapSwap,
) -> Option<(PoolMetadata, SimResult)> {
    let Some(meta) = resolver.resolve(swap).await else {
        info!(token_in = %swap.token_in, token_out = %swap.token_out, "unknown pool");
        return None;
    };
    let predicted = simulate_raw(
        current_block,
        anvil_provider,
        rpc_url,
        tx,
        &meta,
        swap.token_in,
        swap.version,
    )
    .await
    .inspect_err(|e| warn!(?e, "failed to simulate tx"))
    .ok()?;
    Some((meta, predicted))
}
