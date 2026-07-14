use alloy::{
    consensus::Transaction as _,
    network::TransactionResponse,
    primitives::{Address, TxHash, B256},
    providers::Provider,
    rpc::types::Transaction,
};
use anyhow::Result;

use tracing::info;

use crate::{common::pools::PoolMetadata, decoders::UniswapSwap};

/// Format a transaction hash as `0x1234…5678`.
pub fn short_hash(hash: &B256) -> String {
    let s = hash.to_string();
    format!("{}…{}", &s[..4], &s[s.len() - 4..])
}

/// Format an address as `0x1234…5678`.
pub fn short_addr(addr: Address) -> String {
    let s = addr.to_string();
    format!("{}…{}", &s[..4], &s[s.len() - 4..])
}

/// Format the first 4 bytes of calldata as a hex selector.
///
/// Returns `0x????` if the input is shorter than 4 bytes.
pub fn short_selector(input: &[u8]) -> String {
    if input.len() < 4 {
        return "0x????".into();
    }

    format!("0x{}", hex::encode(&input[..4]))
}

/// Return the human-readable name of the transaction type.
pub fn tx_type(tx: &Transaction) -> &'static str {
    match tx.transaction_type() {
        Some(0) => "Legacy",
        Some(1) => "EIP2930",
        Some(2) => "EIP1559",
        Some(3) => "EIP4844",
        Some(4) => "EIP7702",
        _ => "Unknown",
    }
}

/// Log basic pending transaction info (hash, sender, recipient, gas, nonce).
pub fn log_pending_tx(tx: &Transaction) {
    info!(
        hash = %short_hash(tx.inner.tx_hash()),
        tx_type = tx_type(tx),
        from = %short_addr(tx.inner.signer()),
        to = tx.to().map(short_addr),
        gas_limit = tx.gas_limit(),
        nonce = tx.nonce(),
        "pending tx"
    );
}

/// Log a decoded swap together with its resolved pool metadata.
pub fn log_swap(swap: &UniswapSwap, meta: &PoolMetadata) {
    info!(
        kind = ?swap.kind,
        pool = %short_addr(meta.address),
        token0 = %short_addr(meta.token0),
        token1 = %short_addr(meta.token1),
        reserves = %meta.reserves,
        amount = %swap.amount_specified,
        limit = %swap.amount_limit,
        fee = swap.fee,
        "uniswap swap"
    );
}

#[derive(Clone, Debug, Copy, PartialEq, Eq)]
pub enum TxStatus {
    Dropped,
    Mined,
    Pending,
}

pub async fn check_tx_status<P: Provider>(provider: &P, tx_hash: TxHash) -> Result<TxStatus> {
    Ok(match provider.get_transaction_by_hash(tx_hash).await? {
        None => TxStatus::Dropped,
        Some(t) if t.block_number.is_some() => TxStatus::Mined,
        Some(_) => TxStatus::Pending,
    })
}
