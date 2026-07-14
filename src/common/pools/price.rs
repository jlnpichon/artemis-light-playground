use std::fmt;

use alloy::{
    primitives::{U160, U256, U512},
    providers::Provider,
};

use crate::{
    common::pools::{PoolMetadata, Reserves},
    decoders::{self, UniswapSwap, UniswapVersion},
};

/// Fixed-point multiplier for ratio calculations.
///
/// Values are scaled by 1e18 to avoid floating-point arithmetic while keeping precision.
/// Example: 1% = 1e16, 100% = 1e18.
pub const WAD: U256 = U256::from_limbs([1_000_000_000_000_000_000, 0, 0, 0]);

/// A WAD-scaled value displayed as a decimal (e.g., `1.000000` for 1 WAD).
pub struct Wad(pub U256);

impl fmt::Display for Wad {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        let int = self.0 / WAD;
        let frac = (self.0 % WAD) / U256::from(1_000_000_000_000u64);

        write!(f, "{}.{:06}", int, frac)
    }
}

/// A WAD-scaled percentage displayed as `XX.XX%` (e.g., `50.00%` for 0.5 WAD).
pub struct WadPercent(pub U256);

impl fmt::Display for WadPercent {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        let value = self.0 * U256::from(100);

        let int = value / WAD;
        let frac = (value % WAD) / U256::from(1_000_000_000_000u64);

        write!(f, "{}.{:02}%", int, frac)
    }
}

/// The result of a price impact computation.
///
/// All values are WAD-scaled (1e18) except `amount_out` which is the raw token
/// amount.  `price_impact` is the relative difference between spot and execution
/// prices, as a fraction of WAD (e.g., 0.5 WAD = 50 %).
#[derive(Debug, Clone)]
pub struct PriceImpact {
    pub spot_price: U256,
    pub execution_price: U256,
    pub amount_out: U256,
    pub price_impact: U256,
}

impl PriceImpact {
    /// Compute the price impact from actual swap amounts and the pre-swap spot price.
    ///
    /// Returns `None` if either amount is zero (execution price would be
    /// undefined).
    pub fn from_amounts(amount_in: U256, amount_out: U256, spot_price: U256) -> Option<Self> {
        if amount_in.is_zero() || amount_out.is_zero() {
            return None;
        }

        // amount_out / amount_in
        let execution_price = amount_out.checked_mul(WAD)?.checked_div(amount_in)?;

        // (spot - execution) / spot
        let price_impact = if spot_price > execution_price {
            (spot_price - execution_price)
                .checked_mul(WAD)?
                .checked_div(spot_price)?
        } else {
            U256::ZERO
        };

        Some(Self {
            spot_price,
            execution_price,
            amount_out,
            price_impact,
        })
    }
}

/// Compute price impact for a swap given its on-chain pool metadata.
///
/// Dispatches to the V2 constant-product formula or the V3 QuoterV2 depending
/// on [`UniswapVersion`].
pub async fn compute_price_impact<P: Provider + Clone>(
    provider: &P,
    swap: &UniswapSwap,
    meta: &PoolMetadata,
) -> Option<PriceImpact> {
    match swap.version {
        UniswapVersion::V2 => {
            let Reserves::V2 { reserve0, reserve1 } = meta.reserves else {
                return None;
            };
            let (reserve_in, reserve_out) = if swap.token_in == meta.token0 {
                (reserve0, reserve1)
            } else {
                (reserve1, reserve0)
            };
            decoders::uniswap::v2_price_impact(
                reserve_in.to::<U256>(),
                reserve_out.to::<U256>(),
                swap.amount_specified,
            )
        }
        UniswapVersion::V3 => decoders::uniswap::v3_quote(provider, swap, meta).await,
    }
}

/// Convert a Uniswap V3 `sqrtPriceX96` to a WAD-scaled price of token1 per token0.
///
/// When `token_in_is_token0` is `false` the inverse is returned (price of token0
/// per token1).  Returns `None` if the price is zero.
pub fn sqrt_price_x96_to_wad(sqrt_price_x96: U160, token_in_is_token0: bool) -> Option<U256> {
    let sqrt_price = U256::from(sqrt_price_x96);
    let squared: U512 = sqrt_price.widening_mul(sqrt_price);
    let scaled = squared * U512::from(WAD);
    let price_token1_per_token0 = U256::from(scaled >> 192);

    if price_token1_per_token0.is_zero() {
        return None;
    }

    Some(if token_in_is_token0 {
        price_token1_per_token0
    } else {
        (WAD * WAD) / price_token1_per_token0
    })
}

#[cfg(test)]
mod tests {
    use super::*;
    use alloy::primitives::U160;

    #[test]
    fn test_sqrt_price_x96_to_wad_zero() {
        assert!(sqrt_price_x96_to_wad(U160::ZERO, true).is_none());
    }

    #[test]
    fn test_sqrt_price_x96_to_wad_eth_usdc() {
        let sqrt_price_x96 = U160::from(1945708902234702803409436861u128);
        let price = sqrt_price_x96_to_wad(sqrt_price_x96, true).unwrap();
        assert!(price > U256::ZERO);
    }

    #[test]
    fn test_sqrt_price_x96_to_wad_inverted() {
        let sqrt_price_x96 = U160::from(1945708902234702803409436861u128);
        let price_token0_is_in = sqrt_price_x96_to_wad(sqrt_price_x96, true).unwrap();
        let price_token1_is_in = sqrt_price_x96_to_wad(sqrt_price_x96, false).unwrap();
        let inverted = (WAD * WAD) / price_token0_is_in;
        assert_eq!(price_token1_is_in, inverted);
    }

    #[test]
    fn test_price_impact_from_amounts() {
        let impact = PriceImpact::from_amounts(U256::from(100), U256::from(50), WAD).unwrap();
        assert_eq!(impact.execution_price, WAD / U256::from(2));
        assert_eq!(impact.price_impact, WAD / U256::from(2));
    }

    #[test]
    fn test_price_impact_no_impact() {
        let impact = PriceImpact::from_amounts(U256::from(100), U256::from(100), WAD).unwrap();
        assert_eq!(impact.price_impact, U256::ZERO);
    }

    #[test]
    fn test_price_impact_zero_amounts() {
        assert!(PriceImpact::from_amounts(U256::ZERO, U256::from(50), WAD).is_none());
        assert!(PriceImpact::from_amounts(U256::from(100), U256::ZERO, WAD).is_none());
    }

    #[test]
    fn test_price_impact_negative_slippage() {
        let impact = PriceImpact::from_amounts(U256::from(50), U256::from(100), WAD).unwrap();
        assert_eq!(impact.price_impact, U256::ZERO);
    }

    #[test]
    fn test_wad_display() {
        assert_eq!(format!("{}", Wad(WAD)), "1.000000");
        assert_eq!(format!("{}", Wad(U256::from(1))), "0.000000");
        assert_eq!(
            format!("{}", Wad(U256::from(1_500_000_000_000_000_000u128))),
            "1.500000"
        );
    }

    #[test]
    fn test_wad_percent_display() {
        assert_eq!(format!("{}", WadPercent(WAD / U256::from(2))), "50.00%");
        assert_eq!(format!("{}", WadPercent(WAD)), "100.00%");
        assert_eq!(format!("{}", WadPercent(U256::ZERO)), "0.00%");
    }
}
