//! Known Ethereum contract addresses used throughout the project.

use alloy::primitives::{address, Address};

pub const UNISWAP_V2_ROUTER02: Address = address!("7a250d5630B4cF539739dF2C5dAcb4c659F2488D");
pub const UNISWAP_V3_ROUTER: Address = address!("E592427A0AEce92De3Edee1F18E0157C05861564");
pub const UNISWAP_V3_ROUTER02: Address = address!("68b3465833fb72A70ecDF485E0e4C7bD8665Fc45");
pub const UNISWAP_UNIVERSAL_ROUTER_LEGACY: Address =
    address!("Ef1c6E67703c7BD7107eed8303Fbe6EC2554BF6B");
pub const UNISWAP_UNIVERSAL_ROUTER: Address = address!("3fC91A3afd70395Cd496C647d5a6CC9D4B2b7FAD");

pub const UNISWAP_V2_FACTORY: Address = address!("5C69bEe701ef814a2B6a3EDD4B1652CB9cc5aA6f");
pub const UNISWAP_V3_FACTORY: Address = address!("1F98431c8aD98523631AE4a59f267346ea31F984");
pub const UNISWAP_V3_QUOTER_V2: Address = address!("61fFE014bA17989E743c5F6cB21bF9697530B21e");

pub const AAVE_V3_POOL: Address = address!("87870Bca3F3fD6335C3F4ce8392d69350B4fA4E2");

pub const LIDO: Address = address!("ae7ab96520DE3A18E5e111B5EaAb095312D7fE84");

pub const WETH9: Address = address!("C02aaA39b223FE8D0A0e5C4F27eAD9083C756Cc2");

pub fn is_uniswap_router(addr: Address) -> bool {
    matches!(
        addr,
        UNISWAP_V2_ROUTER02
            | UNISWAP_V3_ROUTER
            | UNISWAP_V3_ROUTER02
            | UNISWAP_UNIVERSAL_ROUTER
            | UNISWAP_UNIVERSAL_ROUTER_LEGACY
    )
}

pub fn canonical_order(a: Address, b: Address) -> (Address, Address) {
    if a < b {
        (a, b)
    } else {
        (b, a)
    }
}
