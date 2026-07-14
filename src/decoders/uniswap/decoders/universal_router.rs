use alloy::{
    dyn_abi::SolType,
    primitives::{Address, Bytes, U256},
    sol_types::SolCall,
};
use tracing::{debug, trace, warn};

use crate::decoders::{
    uniswap::{
        abi::UniversalRouter::{
            self, V2SwapExactIn, V2SwapExactOut, V3SwapExactIn, V3SwapExactOut,
        },
        decoders::v3::decode_v3_path,
        Hop, SwapKind,
    },
    DefiEvent, UniswapSwap, UniswapVersion,
};

/// Commands understood by the Uniswap Universal Router.
///
/// The command byte is laid out as:
///
/// | bit 7 | bit 6 | bits 5-0 |
/// |-------|-------|----------|
/// | allow | rsvd  | opcode   |
/// | revert|       |          |
///
/// Only V2/V3 swap commands are actually decoded.  The remaining variants
/// mirror the on-chain command set for documentation.
#[allow(dead_code)]
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
#[repr(u8)]
enum UniversalRouterCommand {
    // --- masks ---
    FlagAllowRevert = 0x80,
    CommandTypeMask = 0x7f,

    // --- opcodes 0x00-0x07 ---
    V3SwapExactIn = 0x00,
    V3SwapExactOut = 0x01,
    Permit2TransferFrom = 0x02,
    Permit2PermitBatch = 0x03,
    Sweep = 0x04,
    Transfer = 0x05,
    PayPortion = 0x06,
    PayPortionFullPrecision = 0x07,

    // --- opcodes 0x08-0x0e ---
    V2SwapExactIn = 0x08,
    V2SwapExactOut = 0x09,
    Permit2Permit = 0x0a,
    WrapEth = 0x0b,
    UnwrapWeth = 0x0c,
    Permit2TransferFromBatch = 0x0d,
    BalanceCheckERC20 = 0x0e,

    // --- opcodes 0x10-0x14 ---
    V4Swap = 0x10,
    V3PositionManagerPermit = 0x11,
    V3PositionManagerCall = 0x12,
    V4InitializePool = 0x13,
    V4PositionManagerCall = 0x14,

    // --- opcode 0x21 ---
    ExecuteSubPlan = 0x21,

    // --- 0x40-0x5f reserved for third-party integrations ---
    AcrossV4DepositV3 = 0x40,
}

impl TryFrom<u8> for UniversalRouterCommand {
    type Error = u8;

    fn try_from(value: u8) -> Result<Self, Self::Error> {
        match value {
            0x00 => Ok(Self::V3SwapExactIn),
            0x01 => Ok(Self::V3SwapExactOut),
            0x02 => Ok(Self::Permit2TransferFrom),
            0x03 => Ok(Self::Permit2PermitBatch),
            0x04 => Ok(Self::Sweep),
            0x05 => Ok(Self::Transfer),
            0x06 => Ok(Self::PayPortion),
            0x08 => Ok(Self::V2SwapExactIn),
            0x09 => Ok(Self::V2SwapExactOut),
            0x0a => Ok(Self::Permit2Permit),
            0x0b => Ok(Self::WrapEth),
            0x0c => Ok(Self::UnwrapWeth),
            0x0d => Ok(Self::Permit2TransferFromBatch),
            other => Err(other),
        }
    }
}

fn build_swap_event(
    router: Address,
    version: UniswapVersion,
    kind: SwapKind,
    hops: Vec<Hop>,
    amount_specified: U256,
    amount_limit: U256,
) -> Option<DefiEvent> {
    if hops.len() != 1 {
        trace!(
            hops = hops.len(),
            "universal router multi-hop swap, only single-hop supported"
        );
    }
    let [hop] = hops.try_into().ok()?;

    Some(DefiEvent::UniswapSwap(UniswapSwap {
        kind,
        version,
        router,
        token_in: hop.token_in,
        token_out: hop.token_out,
        amount_specified,
        amount_limit,
        fee: hop.fee,
    }))
}

fn process_commands(commands: &[u8], inputs: &[Bytes], router: Address) -> Vec<DefiEvent> {
    let mut events = Vec::new();
    for (i, &cmd) in commands.iter().enumerate() {
        let opcode = cmd & UniversalRouterCommand::CommandTypeMask as u8;
        let Some(cmd_input) = inputs.get(i) else {
            debug!(index = i, "missing input for command");
            continue;
        };

        match UniversalRouterCommand::try_from(opcode) {
            Ok(UniversalRouterCommand::V2SwapExactIn) => {
                let swap = match V2SwapExactIn::abi_decode(cmd_input) {
                    Ok(swap) => swap,
                    Err(e) => {
                        warn!(?e, "failed to decode abi");
                        continue;
                    }
                };
                let hops = decode_v2_path(&swap.path);
                if let Some(event) = build_swap_event(
                    router,
                    UniswapVersion::V2,
                    SwapKind::ExactInput,
                    hops,
                    swap.amountIn,
                    swap.amountOutMinimum,
                ) {
                    events.push(event);
                }
            }
            Ok(UniversalRouterCommand::V2SwapExactOut) => {
                let swap = match V2SwapExactOut::abi_decode(cmd_input) {
                    Ok(swap) => swap,
                    Err(e) => {
                        warn!(?e, "failed to decode abi");
                        continue;
                    }
                };
                let hops = decode_v2_path(&swap.path);
                if let Some(event) = build_swap_event(
                    router,
                    UniswapVersion::V2,
                    SwapKind::ExactOutput,
                    hops,
                    swap.amountOut,
                    swap.amountInMax,
                ) {
                    events.push(event);
                }
            }
            Ok(UniversalRouterCommand::V3SwapExactIn) => {
                let swap = match V3SwapExactIn::abi_decode(cmd_input) {
                    Ok(swap) => swap,
                    Err(e) => {
                        warn!(?e, "failed to decode abi");
                        continue;
                    }
                };
                let hops = decode_v3_path(&swap.path);
                if let Some(event) = build_swap_event(
                    router,
                    UniswapVersion::V3,
                    SwapKind::ExactInput,
                    hops,
                    swap.amountIn,
                    swap.amountOutMinimum,
                ) {
                    events.push(event);
                }
            }
            Ok(UniversalRouterCommand::V3SwapExactOut) => {
                let swap = match V3SwapExactOut::abi_decode(cmd_input) {
                    Ok(swap) => swap,
                    Err(e) => {
                        warn!(?e, "failed to decode abi");
                        continue;
                    }
                };
                let hops = decode_v3_path(&swap.path);
                if let Some(event) = build_swap_event(
                    router,
                    UniswapVersion::V3,
                    SwapKind::ExactOutput,
                    hops,
                    swap.amountOut,
                    swap.amountInMax,
                ) {
                    events.push(event);
                }
            }
            Ok(other) => {
                debug!("unknown command {other:?}");
            }
            Err(raw) => {
                warn!(opcode = raw, "unknown universal router command");
            }
        };
    }

    events
}

/// Decode a call to the Uniswap Universal Router.
///
/// Handles both `execute(bytes,bytes[],uint256)` and
/// `execute(bytes,bytes[])` entrypoints.  Only V2 and V3 swap commands are
/// decoded; multi-hop paths are silently dropped.
pub fn decode_universal_router(input: &Bytes, router: Address) -> Vec<DefiEvent> {
    if let Ok(call) = UniversalRouter::execute_0Call::abi_decode(input) {
        return process_commands(&call.commands, &call.inputs, router);
    }
    if let Ok(call) = UniversalRouter::execute_1Call::abi_decode(input) {
        return process_commands(&call.commands, &call.inputs, router);
    }
    vec![]
}

/// Decode a contiguous V2 address-path into a list of hops.
///
/// Unlike V3, V2 paths do not encode fees; the fee field is set to 3000 as a
/// placeholder.
fn decode_v2_path(path: &[Address]) -> Vec<Hop> {
    path.windows(2)
        .map(|pair| Hop {
            token_in: pair[0],
            token_out: pair[1],
            fee: 3000,
        })
        .collect()
}
