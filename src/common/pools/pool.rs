use alloy::primitives::{aliases::U112, Address, U160};
use std::fmt;

/// On-chain reserves for a DEX pool.
///
/// V2 stores token balances as `U112`.  V3 stores the sqrt-price and available
/// liquidity from the pool's `slot0` and `liquidity()`.
#[derive(Debug, Clone)]
pub enum Reserves {
    V2 {
        reserve0: U112,
        reserve1: U112,
    },
    V3 {
        sqrt_price_x96: U160,
        liquidity: u128,
    },
}

impl fmt::Display for Reserves {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Reserves::V2 { reserve0, reserve1 } => {
                write!(f, "r0={reserve0} r1={reserve1}")
            }
            Reserves::V3 {
                sqrt_price_x96,
                liquidity,
            } => {
                write!(f, "sqrtP={sqrt_price_x96} liq={liquidity}")
            }
        }
    }
}

/// Metadata for a DEX pool including its address, token pair, and current reserves.
#[derive(Debug, Clone)]
pub struct PoolMetadata {
    pub address: Address,
    pub token0: Address,
    pub token1: Address,
    pub reserves: Reserves,
}
