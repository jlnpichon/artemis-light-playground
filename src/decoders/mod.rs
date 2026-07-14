//! Protocol-specific transaction decoders.
//!
//! Each supported protocol registers itself in a global [`OnceLock`] map keyed by
//! contract address.  Call [`decode_defi_tx`] on a pending [`Transaction`] to
//! attempt decoding against all registered decoders.
//!
//! Currently supported: Uniswap V2/V3/UniversalRouter, Aave V3, Lido.

use std::{collections::HashMap, sync::OnceLock};

use alloy::{consensus::Transaction as _, primitives::Address, rpc::types::Transaction};

pub use crate::decoders::uniswap::{UniswapSwap, UniswapVersion};
use crate::{
    common::addresses::{
        AAVE_V3_POOL, LIDO, UNISWAP_UNIVERSAL_ROUTER, UNISWAP_UNIVERSAL_ROUTER_LEGACY,
        UNISWAP_V2_ROUTER02, UNISWAP_V3_ROUTER, UNISWAP_V3_ROUTER02,
    },
    decoders::{
        aave::{AaveDecoder, AaveLiquidation},
        lido::{LidoDecoder, LidoStake},
        uniswap::UniswapDecoder,
    },
};

pub mod aave;
pub mod lido;
pub mod uniswap;

static DECODERS: OnceLock<HashMap<Address, Box<dyn ProtocolDecoder>>> = OnceLock::new();

fn decoders() -> &'static HashMap<Address, Box<dyn ProtocolDecoder>> {
    DECODERS.get_or_init(|| {
        let mut map = HashMap::new();

        map.insert(UNISWAP_V2_ROUTER02, UniswapDecoder::new_boxed());
        map.insert(UNISWAP_V3_ROUTER, UniswapDecoder::new_boxed());
        map.insert(UNISWAP_V3_ROUTER02, UniswapDecoder::new_boxed());
        map.insert(UNISWAP_UNIVERSAL_ROUTER, UniswapDecoder::new_boxed());
        map.insert(UNISWAP_UNIVERSAL_ROUTER_LEGACY, UniswapDecoder::new_boxed());

        map.insert(AAVE_V3_POOL, AaveDecoder::new_boxed());

        map.insert(LIDO, LidoDecoder::new_boxed());

        map
    })
}

/// A DeFi event decoded from a pending transaction.
///
/// Variants correspond to the supported protocols.  Use the `as_*` accessors for
/// type-safe pattern matching without importing the inner types.
#[derive(Debug, Clone)]
pub enum DefiEvent {
    UniswapSwap(UniswapSwap),
    AaveLiquidation(AaveLiquidation),
    LidoStake(LidoStake),
}

impl DefiEvent {
    /// Returns a reference to the inner [`UniswapSwap`] if this is a swap event.
    pub fn as_swap(&self) -> Option<&UniswapSwap> {
        match self {
            DefiEvent::UniswapSwap(swap) => Some(swap),
            _ => None,
        }
    }

    /// Returns a reference to the inner [`AaveLiquidation`] if this is a liquidation event.
    pub fn as_liquidation(&self) -> Option<&AaveLiquidation> {
        match self {
            DefiEvent::AaveLiquidation(liq) => Some(liq),
            _ => None,
        }
    }

    /// Returns a reference to the inner [`LidoStake`] if this is a staking event.
    pub fn as_stake(&self) -> Option<&LidoStake> {
        match self {
            DefiEvent::LidoStake(stake) => Some(stake),
            _ => None,
        }
    }
}

/// Protocol-specific transaction decoder.
///
/// Implementations inspect a raw [`Transaction`] and return zero or more
/// [`DefiEvent`]s.  Decoders are stored in a global registry keyed by the
/// contract address that the transaction is sent to.
pub trait ProtocolDecoder: Send + Sync + 'static {
    /// Attempt to decode the transaction, returning any recognized events.
    fn decode(&self, tx: &Transaction) -> Vec<DefiEvent>;
}

/// Attempt to decode a transaction against all registered protocols.
///
/// Looks up the recipient address in the global decoder registry and returns
/// all decoded events.  Returns an empty vec if the address is unknown or
/// no decoder recognises the calldata.
pub fn decode_defi_tx(tx: &Transaction) -> Vec<DefiEvent> {
    let Some(to) = tx.to() else {
        return vec![];
    };
    let Some(decoder) = decoders().get(&to) else {
        return vec![];
    };
    decoder.decode(tx)
}
