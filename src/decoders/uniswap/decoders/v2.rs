use alloy::{
    primitives::{Address, Bytes, U256},
    sol_types::SolCall,
};

use crate::{
    common::addresses::WETH9,
    decoders::{
        uniswap::{abi::UniswapV2Router02, SwapKind},
        DefiEvent, UniswapSwap, UniswapVersion,
    },
};

/// Decode a call to the Uniswap V2 router (Router02).
///
/// Supports all six swap functions: `swapExactTokensForTokens`,
/// `swapTokensForExactTokens`, `swapExactETHForTokens`, `swapTokensForExactETH`,
/// `swapExactTokensForETH`, and `swapETHForExactTokens`.
///
/// ETH-involving swaps substitute [`WETH9`] as the token address.  The fee is
/// hardcoded to 3000 (not meaningful for V2).
pub fn decode_v2(input: &Bytes, router: Address, value: U256) -> Option<DefiEvent> {
    let version = UniswapVersion::V2;
    let fee = 3000;
    if let Ok(call) = UniswapV2Router02::swapExactTokensForTokensCall::abi_decode(input) {
        return Some(DefiEvent::UniswapSwap(UniswapSwap {
            kind: SwapKind::ExactInput,
            version,
            router,
            token_in: *call.path.first()?,
            token_out: *call.path.last()?,
            amount_specified: call.amountIn,
            amount_limit: call.amountOutMin,
            fee,
        }));
    };
    if let Ok(call) = UniswapV2Router02::swapTokensForExactTokensCall::abi_decode(input) {
        return Some(DefiEvent::UniswapSwap(UniswapSwap {
            kind: SwapKind::ExactOutput,
            version,
            router,
            token_in: *call.path.first()?,
            token_out: *call.path.last()?,
            amount_specified: call.amountOut,
            amount_limit: call.amountInMax,
            fee,
        }));
    };
    if let Ok(call) = UniswapV2Router02::swapExactETHForTokensCall::abi_decode(input) {
        return Some(DefiEvent::UniswapSwap(UniswapSwap {
            kind: SwapKind::ExactInput,
            version,
            router,
            token_in: WETH9,
            token_out: *call.path.last()?,
            amount_specified: value,
            amount_limit: call.amountOutMin,
            fee,
        }));
    };
    if let Ok(call) = UniswapV2Router02::swapTokensForExactETHCall::abi_decode(input) {
        return Some(DefiEvent::UniswapSwap(UniswapSwap {
            kind: SwapKind::ExactOutput,
            version,
            router,
            token_in: *call.path.first()?,
            token_out: WETH9,
            amount_specified: call.amountOut,
            amount_limit: call.amountInMax,
            fee,
        }));
    };
    if let Ok(call) = UniswapV2Router02::swapExactTokensForETHCall::abi_decode(input) {
        return Some(DefiEvent::UniswapSwap(UniswapSwap {
            kind: SwapKind::ExactInput,
            version,
            router,
            token_in: *call.path.first()?,
            token_out: WETH9,
            amount_specified: call.amountIn,
            amount_limit: call.amountOutMin,
            fee,
        }));
    };
    if let Ok(call) = UniswapV2Router02::swapETHForExactTokensCall::abi_decode(input) {
        return Some(DefiEvent::UniswapSwap(UniswapSwap {
            kind: SwapKind::ExactOutput,
            version,
            router,
            token_in: WETH9,
            token_out: *call.path.last()?,
            amount_specified: call.amountOut,
            amount_limit: value,
            fee,
        }));
    };
    None
}
