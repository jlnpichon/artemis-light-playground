use alloy::providers::{
    fillers::{BlobGasFiller, ChainIdFiller, FillProvider, GasFiller, JoinFill, NonceFiller},
    ProviderBuilder, RootProvider, WsConnect,
};
use anyhow::{Context, Result};

/// A fully-fleshed alloy WebSocket provider with gas estimation, nonce
/// management, and blob-gas fillers.
pub type WsProvider = FillProvider<
    JoinFill<
        alloy::providers::Identity,
        JoinFill<GasFiller, JoinFill<BlobGasFiller, JoinFill<NonceFiller, ChainIdFiller>>>,
    >,
    RootProvider,
>;

/// Build a WebSocket provider from the `ETH_WSS_URL` environment variable.
pub async fn build_provider_ws() -> Result<WsProvider> {
    let ws_url = std::env::var("ETH_WSS_URL")?;
    let ws = WsConnect::new(ws_url);
    let provider = ProviderBuilder::new()
        .connect_ws(ws)
        .await
        .context("failed to establish WebSocket")?;

    Ok(provider)
}
