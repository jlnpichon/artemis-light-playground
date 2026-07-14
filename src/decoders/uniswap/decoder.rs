use alloy::{
    consensus::Transaction as _,
    primitives::{Address, U256},
    rpc::types::Transaction,
};
use tracing::{debug, info, trace};

use crate::{
    common::{
        self,
        tx::{short_addr, short_selector},
    },
    decoders::{
        uniswap::decoders::{
            universal_router::decode_universal_router, v2::decode_v2, v3::decode_v3,
        },
        DefiEvent, ProtocolDecoder,
    },
};

/// Whether the user specified the exact input amount or the exact output amount.
#[derive(Debug, Clone)]
pub enum SwapKind {
    ExactInput,
    ExactOutput,
}

/// Uniswap protocol version.
#[derive(Debug, Copy, Clone, Hash, PartialEq, Eq)]
pub enum UniswapVersion {
    V2,
    V3,
}

/// A single swap decoded from a Uniswap-family router call.
///
/// For multi-hop paths only the first and last tokens are captured; intermediate
/// hops are ignored (with a trace log).
#[derive(Debug, Clone)]
pub struct UniswapSwap {
    pub kind: SwapKind,
    pub router: Address,
    pub token_in: Address,
    pub token_out: Address,
    pub version: UniswapVersion,

    pub amount_specified: U256,
    pub amount_limit: U256,
    /// Pool fee tier in basis points (e.g. 3000 = 0.3 %).
    /// For V2 this is a placeholder and always set to 3000.
    pub fee: u32,
}

impl UniswapSwap {
    pub fn log(&self) {
        info!(
            version = ?self.version,
            kind = ?self.kind,
            router = %short_addr(self.router),
            token_in = %short_addr(self.token_in),
            token_out = %short_addr(self.token_out),
            amount_specified = %self.amount_specified,
            amount_limit = %self.amount_limit,
            "uniswap swap"
        );
    }
}

/// Decoder for Uniswap V2, V3, and Universal Router transactions.
///
/// Dispatches to [`decode_v2`], [`decode_v3`], or [`decode_universal_router`]
/// based on the recipient address.
pub struct UniswapDecoder;

impl UniswapDecoder {
    pub fn new_boxed() -> Box<dyn ProtocolDecoder> {
        Box::new(Self {})
    }
}

impl ProtocolDecoder for UniswapDecoder {
    fn decode(&self, tx: &Transaction) -> Vec<DefiEvent> {
        let input = tx.input();
        let Some(router) = tx.to() else {
            return vec![];
        };

        trace!(
            router = %short_addr(router),
            selector = %short_selector(input),
            "trying uniswap decode"
        );

        let events = match router {
            common::addresses::UNISWAP_V2_ROUTER02 => match decode_v2(input, router, tx.value()) {
                Some(e) => vec![e],
                None => vec![],
            },
            common::addresses::UNISWAP_V3_ROUTER | common::addresses::UNISWAP_V3_ROUTER02 => {
                match decode_v3(input, router) {
                    Some(e) => vec![e],
                    None => vec![],
                }
            }
            common::addresses::UNISWAP_UNIVERSAL_ROUTER => decode_universal_router(input, router),
            _ => vec![],
        };

        if !events.is_empty() {
            debug!(
                router = %short_addr(router),
                ?events,
                "decoded uniswap event"
            );
        } else {
            trace!(
                router = %short_addr(router),
                selector = %short_selector(input),
                "uniswap router call not supported"
            );
        }

        events
    }
}

/// A single hop in a multi-hop swap path.
#[derive(Debug, Clone)]
pub struct Hop {
    pub token_in: Address,
    pub token_out: Address,
    pub fee: u32,
}
