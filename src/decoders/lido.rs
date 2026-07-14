use alloy::{
    consensus::Transaction as _,
    network::TransactionResponse,
    primitives::{Address, U256},
    rpc::types::Transaction,
    sol,
    sol_types::SolCall,
};
use tracing::{debug, info, trace};

use crate::{
    common::tx::{short_addr, short_selector},
    decoders::{DefiEvent, ProtocolDecoder},
};

sol! {
    #[derive(Debug)]
    contract Lido {
        function submit(address _referral)
            external
            payable
            returns (uint256);
    }
}

/// A decoded Lido staking (`submit`) event.
#[derive(Debug, Clone)]
pub struct LidoStake {
    pub user: Address,
    pub amount: U256,
}

impl LidoStake {
    pub fn log(&self) {
        info!(
            user = short_addr(self.user),
            amount = %self.amount,
            "lido stake"
        );
    }
}

/// Decoder for Lido `submit` transactions.
pub struct LidoDecoder;

impl LidoDecoder {
    pub fn new_boxed() -> Box<dyn ProtocolDecoder> {
        Box::new(Self {})
    }
}

impl ProtocolDecoder for LidoDecoder {
    fn decode(&self, tx: &Transaction) -> Vec<DefiEvent> {
        let input = tx.input();
        let Some(pool) = tx.to() else {
            return vec![];
        };
        let from = tx.from();

        trace!(
            pool = %pool,
            selector = ?short_selector(input),
            "trying lido decode"
        );

        if Lido::submitCall::abi_decode(input).is_ok() {
            let event = LidoStake {
                user: from,
                amount: tx.value(),
            };

            debug!(
                pool = %pool,
                ?event,
                "decoded lido event"
            );

            return vec![DefiEvent::LidoStake(event)];
        }

        trace!(
            pool = %short_addr(pool),
            selector = %short_selector(input),
            "lido contract call not supported"
        );

        vec![]
    }
}
