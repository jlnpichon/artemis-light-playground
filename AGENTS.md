# artemis-playground — Agent instructions

## Project

Educational MEV sandbox built on [artemis-light](https://github.com/paradigmxyz/artemis-light).  
Not production-grade — learning-only code.

## Commands

```bash
cargo run --bin <name>        # run any of the 7 binaries
cargo test                    # all unit tests (no integration tests)
cargo check --all-targets     # CI: check every target
cargo clippy --all-targets -- -D warnings   # CI: lint
```

Binaries (in order of complexity): `pending_printer`, `router_filter`, `defi_printer`, `pool_resolver`, `price_impact`, `transaction_sim`, `sim_accuracy`.  
Each is defined in `Cargo.toml` via `[[bin]]` blocks pointing to `bin/<name>.rs`.

Rust stable, edition 2021. No custom toolchain or rustfmt/clippy config — defaults apply.

## Setup

- Copy `.env.example` → `.env` and set `ETH_RPC_URL` + `ETH_WSS_URL` (Alchemy/Infura with WS support).
- Env vars are loaded once by `dotenvy::dotenv().ok()` at startup — no other mechanism.
- `transaction_sim` and `sim_accuracy` require [Anvil](https://book.getfoundry.sh/anvil/) (Foundry) on `$PATH`.

## Architecture

Control flow: `Collector → Strategy → Executor` (artemis-light pipeline). Optional `.filter_map()` narrows the event stream before the strategy.

```
src/
├── lib.rs                # re-exports common + decoders
├── common/               # shared infra
│   ├── engine.rs         # init_tracing(), run_engine() — every binary calls these
│   ├── provider.rs       # build_provider_ws() from ETH_WSS_URL
│   ├── anvil.rs          # AnvilProcess drop guard for forked node
│   ├── sim.rs            # simulate_raw(), predict_swap() — Anvil fork + snapshot/revert
│   ├── pools/            # pool metadata, reserves, price impact, on-chain resolution
│   ├── telemetry.rs      # event counter observer
│   └── tx.rs             # formatting utilities, TxStatus
└── decoders/             # protocol decoders
    ├── uniswap/          # V2/V3/UniversalRouter — decoder + pricing
    ├── aave.rs
    └── lido.rs
```

Decoders register themselves in a global `OnceLock<HashMap<Address, Box<dyn ProtocolDecoder>>>` keyed by contract address. New decoders add to `src/decoders/mod.rs::decoders()`.

## Key quirks

- **V2 fee hardcoded to 3000** in decoders (not meaningful for V2).
- **Fork reset per block, not per tx** — `sim.rs` resets Anvil fork only when block number advances; multiple tx in same block use snapshot/revert for isolation.
- **Simulation timeout (30s) covers receipt only** — no protection if Anvil hangs before that.
- `UniversalRouterCommand` variants `V4Swap`, `ExecuteSubPlan`, etc. are defined but never decoded — documentation-only.

## Testing

Tests are pure unit tests (no integration tests, no required services). Run `cargo test` directly.
