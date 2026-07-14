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
    contract Pool {
        function liquidationCall(
            address collateralAsset,
            address debtAsset,
            address user,
            uint256 debtToCover,
            bool receiveAToken
        ) public virtual override;
    }
}

/// A decoded Aave V3 liquidation event.
#[derive(Debug, Clone)]
pub struct AaveLiquidation {
    pub liquidator: Address,
    pub collateral_token: Address,
    pub debt_token: Address,
    pub user: Address,
    pub debt_to_cover: U256,
    pub receive_a_token: bool,
}

impl AaveLiquidation {
    pub fn log(&self) {
        info!(
            liquidator = short_addr(self.liquidator),
            user = short_addr(self.user),
            collateral = short_addr(self.collateral_token),
            debt = short_addr(self.debt_token),
            amount = %self.debt_to_cover,
            "aave liquidation"
        );
    }
}

/// Decoder for Aave V3 `liquidationCall` transactions.
pub struct AaveDecoder;

impl AaveDecoder {
    pub fn new_boxed() -> Box<dyn ProtocolDecoder> {
        Box::new(Self {})
    }
}

impl ProtocolDecoder for AaveDecoder {
    fn decode(&self, tx: &Transaction) -> Vec<DefiEvent> {
        let input = tx.input();
        let Some(pool) = tx.to() else {
            return vec![];
        };
        let from = tx.from();

        trace!(
            pool = %pool,
            selector = ?short_selector(input),
            "trying aave decode"
        );

        if let Ok(call) = Pool::liquidationCallCall::abi_decode(input) {
            let event = AaveLiquidation {
                liquidator: from,
                collateral_token: call.collateralAsset,
                debt_token: call.debtAsset,
                debt_to_cover: call.debtToCover,
                user: call.user,
                receive_a_token: call.receiveAToken,
            };

            debug!(
                pool = %pool,
                ?event,
                "decoded aave event"
            );

            return vec![DefiEvent::AaveLiquidation(event)];
        }
        trace!(
            pool = %short_addr(pool),
            selector = %short_selector(input),
            "aave pool contract call not supported"
        );

        vec![]
    }
}
