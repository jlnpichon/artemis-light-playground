use std::sync::Arc;

use alloy::{
    primitives::{aliases::U24, Address},
    providers::Provider,
    sol,
};
use dashmap::DashMap;
use tracing::{debug, trace};

use crate::{
    common::{
        addresses::{canonical_order, UNISWAP_V2_FACTORY, UNISWAP_V3_FACTORY},
        pools::{PoolMetadata, Reserves},
        tx::short_addr,
    },
    decoders::{UniswapSwap, UniswapVersion},
};

sol! {
    #[sol(rpc)]
    #[derive(Debug)]
    interface IUniswapV2Factory {
        function getPair(address tokenA, address tokenB) external view returns (address pair);
    }
    #[derive(Debug)]
    #[sol(rpc)]
    interface IUniswapV2Pair {
        event Swap(
            address indexed sender,
            uint256 amount0In,
            uint256 amount1In,
            uint256 amount0Out,
            uint256 amount1Out,
            address indexed to
        );

        function getReserves() external view returns (uint112 reserve0, uint112 reserve1, uint32 blockTimestampLast);
    }
    #[sol(rpc)]
    interface IUniswapV3Factory {
        function getPool(address tokenA, address tokenB, uint24 fee) external view returns (address pool);
    }
    #[sol(rpc)]
    interface IUniswapV3Pool {
        event Swap(
            address indexed sender,
            address indexed recipient,
            int256 amount0,
            int256 amount1,
            uint160 sqrtPriceX96,
            uint128 liquidity,
            int24 tick
        );

        function slot0() external view returns (uint160 sqrtPriceX96, int24 tick, uint16 observationIndex, uint16 observationCardinality, uint16 observationCardinalityNext, uint8 feeProtocol, bool unlocked);
        function liquidity() external view returns (uint128);
    }
}

/// On-chain DEX pool address and reserve resolver with an in-memory cache.
///
/// Given a [`UniswapSwap`] it:
/// 1. Looks up the pool address from the factory (cached in a [`DashMap`]).
/// 2. Fetches current reserves from the pool contract.
/// 3. Returns a [`PoolMetadata`] struct.
pub struct PoolResolver<P> {
    provider: P,
    pool_cache: Arc<DashMap<(Address, Address, UniswapVersion, u32), Address>>,
}

impl<P: Provider + Clone> PoolResolver<P> {
    pub fn new(provider: P) -> Self {
        Self {
            provider,
            pool_cache: Arc::new(DashMap::new()),
        }
    }

    async fn find_pool(
        &self,
        token0: Address,
        token1: Address,
        swap: &UniswapSwap,
    ) -> Option<Address> {
        let key = (token0, token1, swap.version, swap.fee);

        if let Some(pool) = self.pool_cache.get(&key) {
            trace!(pool = short_addr(*pool), "pool cache hit");
            return Some(*pool);
        }

        let pool = match swap.version {
            UniswapVersion::V2 => {
                let factory = IUniswapV2Factory::new(UNISWAP_V2_FACTORY, self.provider.clone());
                factory.getPair(token0, token1).call().await.ok()?
            }
            UniswapVersion::V3 => {
                let factory = IUniswapV3Factory::new(UNISWAP_V3_FACTORY, self.provider.clone());
                factory
                    .getPool(token0, token1, U24::from(swap.fee))
                    .call()
                    .await
                    .ok()?
            }
        };

        if pool.is_zero() {
            return None; // pool not found, do not cache it because pool might be created later
        }

        trace!(pool = short_addr(pool), "pool cache insert");
        self.pool_cache.insert(key, pool);

        Some(pool)
    }

    /// Resolve the pool address and fetch current reserves for the given swap.
    ///
    /// Returns `None` if the pool does not exist on chain or if any RPC call fails.
    pub async fn resolve(&self, swap: &UniswapSwap) -> Option<PoolMetadata> {
        trace!(
            token_in = short_addr(swap.token_in),
            token_out = short_addr(swap.token_out),
            "resolving"
        );

        let (token0, token1) = canonical_order(swap.token_in, swap.token_out);
        let pool = self.find_pool(token0, token1, swap).await?;

        let reserves = match swap.version {
            UniswapVersion::V2 => {
                let pair = IUniswapV2Pair::new(pool, self.provider.clone());
                let r = pair.getReserves().call().await.ok()?;
                Reserves::V2 {
                    reserve0: r.reserve0,
                    reserve1: r.reserve1,
                }
            }
            UniswapVersion::V3 => {
                let p = IUniswapV3Pool::new(pool, self.provider.clone());
                let slot0 = p.slot0().call().await.ok()?;
                let liquidity = p.liquidity().call().await.ok()?;
                Reserves::V3 {
                    sqrt_price_x96: slot0.sqrtPriceX96,
                    liquidity,
                }
            }
        };

        let metadata = PoolMetadata {
            address: pool,
            token0,
            token1,
            reserves,
        };

        debug!(metadata = ?metadata, "pool metadata fetched");

        Some(metadata)
    }
}
