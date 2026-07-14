use alloy::{
    primitives::{aliases::U24, U160, U256},
    providers::Provider,
};

use crate::{
    common::{
        addresses::UNISWAP_V3_QUOTER_V2,
        pools::{sqrt_price_x96_to_wad, PoolMetadata, PriceImpact, Reserves, WAD},
    },
    decoders::uniswap::abi::IQuoterV2,
};

use super::decoder::{SwapKind, UniswapSwap};

/// Compute the price impact for a V2 swap using the constant-product formula.
///
/// Returns `None` if reserves are zero or if the computed `amount_out` is zero
/// (which would make the execution price undefined).
pub fn v2_price_impact(
    reserve_in: U256,
    reserve_out: U256,
    amount_in: U256,
) -> Option<PriceImpact> {
    // reserve_out / reserve_in
    let spot_price = reserve_out.checked_mul(WAD)?.checked_div(reserve_in)?;

    // Uniswap V2 amountOut
    let amount_in_with_fee = amount_in.checked_mul(U256::from(997))?;

    let numerator = amount_in_with_fee.checked_mul(reserve_out)?;
    let denominator = reserve_in
        .checked_mul(U256::from(1000))?
        .checked_add(amount_in_with_fee)?;

    let amount_out = numerator.checked_div(denominator)?;

    PriceImpact::from_amounts(amount_in, amount_out, spot_price)
}

/// Quote a V3 swap via the on-chain QuoterV2 contract.
///
/// Returns the predicted `PriceImpact` including spot price, execution price,
/// and percentage impact.  Requires a provider with access to the QuoterV2 at
/// [`UNISWAP_V3_QUOTER_V2`].
pub async fn v3_quote<P: Provider + Clone>(
    provider: &P,
    swap: &UniswapSwap,
    meta: &PoolMetadata,
) -> Option<PriceImpact> {
    let quoter = IQuoterV2::new(UNISWAP_V3_QUOTER_V2, provider.clone());

    let (amount_in, amount_out) = match swap.kind {
        SwapKind::ExactInput => {
            let params = IQuoterV2::QuoteExactInputSingleParams {
                tokenIn: swap.token_in,
                tokenOut: swap.token_out,
                amountIn: swap.amount_specified,
                fee: U24::from(swap.fee),
                sqrtPriceLimitX96: U160::ZERO,
            };
            let r = quoter.quoteExactInputSingle(params).call().await.ok()?;
            (swap.amount_specified, r.amountOut)
        }
        SwapKind::ExactOutput => {
            let params = IQuoterV2::QuoteExactOutputSingleParams {
                tokenIn: swap.token_in,
                tokenOut: swap.token_out,
                amount: swap.amount_specified,
                fee: U24::from(swap.fee),
                sqrtPriceLimitX96: U160::ZERO,
            };
            let r = quoter.quoteExactOutputSingle(params).call().await.ok()?;
            (r.amountIn, swap.amount_specified)
        }
    };

    let Reserves::V3 { sqrt_price_x96, .. } = meta.reserves else {
        return None;
    };
    let spot_price = sqrt_price_x96_to_wad(sqrt_price_x96, swap.token_in == meta.token0)?;

    PriceImpact::from_amounts(amount_in, amount_out, spot_price)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_v2_price_impact_basic() {
        let result = v2_price_impact(U256::from(1000), U256::from(2000), U256::from(100));
        assert!(result.is_some());
        let impact = result.unwrap();
        assert_eq!(impact.spot_price, U256::from(2000) * WAD / U256::from(1000));
        assert!(impact.price_impact > U256::ZERO);
        assert!(impact.price_impact < WAD);
    }

    #[test]
    fn test_v2_price_impact_no_reserves() {
        assert!(v2_price_impact(U256::ZERO, U256::from(2000), U256::from(100)).is_none());
        assert!(v2_price_impact(U256::from(1000), U256::ZERO, U256::from(100)).is_none());
    }

    #[test]
    fn test_v2_price_impact_small_swap() {
        let result = v2_price_impact(
            U256::from(1_000_000),
            U256::from(1_000_000),
            U256::from(100),
        );
        assert!(result.is_some());
        let impact = result.unwrap();
        assert_eq!(impact.spot_price, WAD);
        assert!(impact.price_impact > U256::ZERO);
        assert!(impact.price_impact <= WAD / U256::from(100));
    }

    #[test]
    fn test_v2_price_impact_large_swap() {
        let result = v2_price_impact(U256::from(1000), U256::from(1000), U256::from(900));
        assert!(result.is_some());
        let impact = result.unwrap();
        assert!(impact.price_impact > U256::ZERO);
        assert!(impact.price_impact < WAD);
    }
}
