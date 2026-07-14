# artemis-playground

> Learning Ethereum MEV concepts with [artemis-light](https://github.com/paradigmxyz/artemis-light)

A progressive, educational codebase exploring mempool monitoring,
transaction decoding, DEX pool resolution, price impact analysis,
and Anvil-based transaction simulation. Each binary builds on the previous one,
forming a gentle introduction to MEV tooling with Rust.

**Not production-grade.** This is a learning sandbox.

---

## Architecture

```
          ┌──────────────┐
          │   Ethereum   │
          │  (WSS RPC)   │
          └──────┬───────┘
                 │ pending tx
                 ▼
       ┌────────────────────────┐
       │  MempoolCollector      │  artemis-light
       │  (+ filter_map)        │
       └────────┬───────────────┘
                │ Transaction
                ▼
       ┌────────────────────────┐
       │  ProtocolDecoder       │
       │  (Uniswap / Aave/Lido) │
       └────────┬───────────────┘
                │ DefiEvent
                ▼
       ┌────────────────────────┐
       │  Strategy              │  per-binary logic:
       │  (print / resolve /    │  printer → filter → decode
       │   simulate)            │  → resolve → price → sim
       └────────┬───────────────┘
                │ ActionStream
                ▼
       ┌────────────────────────┐
       │  Telemetry (observer)  │
       └────────────────────────┘
```

The control flow follows the artemis-light `Collector → Strategy → Executor` pipeline.
Optional `filter_map` steps narrow the event stream before it reaches the strategy.

---

## Binaries

| Binary            | Description                                                      | Key concepts                                         |
| ----------------- | ---------------------------------------------------------------- | ---------------------------------------------------- |
| `pending_printer` | Subscribe to pending txs and print basic fields                  | WebSocket provider, MempoolCollector                 |
| `router_filter`   | Filter to only Uniswap router addresses                          | `.filter_map()`, pattern matching                    |
| `defi_printer`    | Decode Uniswap swaps, Aave liquidations, Lido stakes             | ProtocolDecoder trait, ABI decoding                  |
| `pool_resolver`   | Resolve pool addresses & metadata from decoded swaps             | Factory contract calls, DashMap cache                |
| `price_impact`    | Compute spot price, execution price, and price impact            | V2 CPMM formula, V3 QuoterV2 pricing                 |
| `transaction_sim` | Fork mainnet with Anvil and simulate detected transactions       | Anvil fork, snapshot/revert, log extraction          |
| `sim_accuracy`    | Simulate swaps and compare predictions against on-chain outcomes | Accuracy tracking, receipt extraction, diff analysis |

Each binary grows incrementally in complexity — you can follow them in order to understand the full pipeline.

---

## Quick Start

### Prerequisites

- Rust (stable toolchain, edition 2021)
- [Anvil](https://book.getfoundry.sh/anvil/) (included with Foundry)
- An Ethereum RPC endpoint with WebSocket support (e.g. Alchemy, Infura)

### Setup

```bash
cp .env.example .env
# Edit .env with your Ethereum RPC and WSS URLs
```

### Run

```bash
# Start with the simplest binary
cargo run --bin pending_printer

# Then try the more advanced ones
cargo run --bin defi_printer
cargo run --bin transaction_sim
cargo run --bin sim_accuracy
```

### Run tests

```bash
cargo test
```

---

## Known Limitations

- **V2 fee hardcoded to 3000** — the decoder sets `fee: 3000` unconditionally
  for V2 swaps. Not meaningful for V2 (pools use a single fee tier), but could
  confuse downstream consumers.
- **Fork reset per block, not per tx** — `sim.rs` resets the Anvil fork only
  when the block number advances; multiple tx in the same block reuse the same
  fork and rely on `snapshot`/`revert` for isolation.
- **Simulation timeout covers receipt only** — a 30s timeout is applied to
  `pending.get_receipt()`. If the Anvil node itself hangs before that, there
  is no protection.
- **`UniversalRouterCommand` variants unused** — several command enum values
  (`V4Swap`, `ExecuteSubPlan`, `AcrossV4DepositV3`, etc.) are defined but never
  decoded. They exist to mirror the on-chain command set for documentation.

## Project Structure

```
src/
├── lib.rs
├── common/          # shared infrastructure
├── decoders/        # protocol decoders
│   ├── aave.rs
│   ├── lido.rs
│   └── uniswap/
bin/                 # one binary per learning step
├── pending_printer.rs
├── router_filter.rs
├── defi_printer.rs
├── pool_resolver.rs
├── price_impact.rs
├── transaction_sim.rs
└── sim_accuracy.rs
```

---

## Dependencies

- [artemis-light](https://github.com/paradigmxyz/artemis-light) — MEV framework (Collector/Strategy/Executor)
- [alloy](https://github.com/alloy-rs/alloy) — Ethereum client library
- [anvil](https://book.getfoundry.sh/anvil/) — Local test node for forking/simulation
- [tokio](https://tokio.rs/) — Async runtime
- [tracing](https://docs.rs/tracing/) — Structured logging
