use alloy::{
    primitives::{Address, Bytes},
    sol_types::SolCall,
};
use tracing::trace;

use crate::decoders::{
    uniswap::{abi::SwapRouter, Hop, SwapKind},
    DefiEvent, UniswapSwap, UniswapVersion,
};

/// Decode a call to the Uniswap V3 SwapRouter.
///
/// Supports `exactInputSingle`, `exactInput`, `exactOutputSingle`, and
/// `exactOutput`.  Multi-hop paths are silently ignored (with a `trace!` log).
pub fn decode_v3(input: &Bytes, router: Address) -> Option<DefiEvent> {
    let version = UniswapVersion::V3;
    if let Ok(call) = SwapRouter::exactInputSingleCall::abi_decode(input) {
        return Some(DefiEvent::UniswapSwap(UniswapSwap {
            kind: SwapKind::ExactInput,
            version,
            router,
            token_in: call.params.tokenIn,
            token_out: call.params.tokenOut,
            amount_specified: call.params.amountIn,
            amount_limit: call.params.amountOutMinimum,
            fee: call.params.fee.to::<u32>(),
        }));
    }
    if let Ok(call) = SwapRouter::exactInputCall::abi_decode(input) {
        let hops = decode_v3_path(&call.params.path);

        if hops.len() != 1 {
            trace!(
                hops = hops.len(),
                "v3 multi-hop exactInput, only single-hop supported"
            );
            return None;
        };
        let hop = &hops[0];

        return Some(DefiEvent::UniswapSwap(UniswapSwap {
            kind: SwapKind::ExactInput,
            version,
            router,
            token_in: hop.token_in,
            token_out: hop.token_out,
            amount_specified: call.params.amountIn,
            amount_limit: call.params.amountOutMinimum,
            fee: hop.fee,
        }));
    }
    if let Ok(call) = SwapRouter::exactOutputSingleCall::abi_decode(input) {
        return Some(DefiEvent::UniswapSwap(UniswapSwap {
            kind: SwapKind::ExactOutput,
            version,
            router,
            token_in: call.params.tokenIn,
            token_out: call.params.tokenOut,
            amount_specified: call.params.amountOut,
            amount_limit: call.params.amountInMaximum,
            fee: call.params.fee.to::<u32>(),
        }));
    }
    if let Ok(call) = SwapRouter::exactOutputCall::abi_decode(input) {
        let hops = decode_v3_path(&call.params.path);

        if hops.len() != 1 {
            trace!(
                hops = hops.len(),
                "v3 multi-hop exactOutput, only single-hop supported"
            );
            return None;
        }
        let hop = &hops[0];

        // exactOutput path is encoded backward : (tokenOut, fee, tokenIn).
        return Some(DefiEvent::UniswapSwap(UniswapSwap {
            kind: SwapKind::ExactOutput,
            version,
            router,
            token_in: hop.token_out,
            token_out: hop.token_in,
            amount_specified: call.params.amountOut,
            amount_limit: call.params.amountInMaximum,
            fee: hop.fee,
        }));
    }
    None
}

/// Decode a packed V3 path into a list of hops.
///
/// The path format is `(tokenIn, fee, tokenOut)+` where each hop is 43 bytes
/// (20 + 3 + 20).  Intermediate tokens serve as both the output of one hop
/// and the input of the next.
pub fn decode_v3_path(path: &[u8]) -> Vec<Hop> {
    const ADDR_LEN: usize = 20;
    const FEE_LEN: usize = 3;

    let mut hops = Vec::new();
    let mut offset = 0;

    while offset + ADDR_LEN + FEE_LEN + ADDR_LEN <= path.len() {
        let token_in = Address::from_slice(&path[offset..offset + ADDR_LEN]);
        offset += ADDR_LEN;

        let fee = u32::from_be_bytes([0, path[offset], path[offset + 1], path[offset + 2]]);
        offset += FEE_LEN;

        let token_out = Address::from_slice(&path[offset..offset + ADDR_LEN]);

        hops.push(Hop {
            token_in,
            fee,
            token_out,
        });
        // No offset += ADDR_LEN : token_out of this hop is token_in of the next
    }

    hops
}

#[cfg(test)]
mod tests {
    use super::*;
    use alloy::primitives::address;

    const WETH: Address = address!("C02aaA39b223FE8D0A0e5C4F27eAD9083C756Cc2");
    const USDC: Address = address!("A0b86991c6218b36c1d19D4a2e9Eb0cE3606eB48");
    const DAI: Address = address!("6B175474E89094C44Da98b954EedeAC495271d0F");

    fn build_single_hop_path() -> Vec<u8> {
        let mut path = Vec::new();
        path.extend_from_slice(WETH.as_ref());
        path.extend_from_slice(&[0x00, 0x0b, 0xb8]); // fee = 3000
        path.extend_from_slice(USDC.as_ref());
        path
    }

    fn build_multi_hop_path() -> Vec<u8> {
        let mut path = Vec::new();
        path.extend_from_slice(WETH.as_ref());
        path.extend_from_slice(&[0x00, 0x0b, 0xb8]);
        path.extend_from_slice(USDC.as_ref());
        path.extend_from_slice(&[0x00, 0x01, 0xf4]); // fee = 500
        path.extend_from_slice(DAI.as_ref());
        path
    }

    #[test]
    fn test_decode_v3_path_single_hop() {
        let path = build_single_hop_path();
        let hops = decode_v3_path(&path);
        assert_eq!(hops.len(), 1);
        assert_eq!(hops[0].token_in, WETH);
        assert_eq!(hops[0].token_out, USDC);
        assert_eq!(hops[0].fee, 3000);
    }

    #[test]
    fn test_decode_v3_path_multi_hop() {
        let path = build_multi_hop_path();
        let hops = decode_v3_path(&path);
        assert_eq!(hops.len(), 2);
        assert_eq!(hops[0].token_in, WETH);
        assert_eq!(hops[0].token_out, USDC);
        assert_eq!(hops[0].fee, 3000);
        assert_eq!(hops[1].token_in, USDC);
        assert_eq!(hops[1].token_out, DAI);
        assert_eq!(hops[1].fee, 500);
    }

    #[test]
    fn test_decode_v3_path_too_short() {
        let hops = decode_v3_path(&[0u8; 10]);
        assert!(hops.is_empty());
    }

    #[test]
    fn test_decode_v3_path_exact_boundary() {
        let path = vec![0u8; 43];
        let hops = decode_v3_path(&path);
        assert_eq!(hops.len(), 1);
    }

    #[test]
    fn test_decode_v3_path_empty() {
        let hops = decode_v3_path(&[]);
        assert!(hops.is_empty());
    }
}
