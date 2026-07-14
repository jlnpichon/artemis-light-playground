//! artemis-light-playground: an educational MEV exploration toolkit.
//!
//! Built on top of [`artemis-light`](https://github.com/paradigmxyz/artemis-light),
//! this crate provides protocol decoders (Uniswap, Aave, Lido), pool resolution,
//! price impact computation, and Anvil-based transaction simulation.

pub mod common;
pub mod decoders;
