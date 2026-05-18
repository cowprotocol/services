use {
    alloy_primitives::{Address, B256, keccak256},
    alloy_rpc_types::trace::geth::GethTrace,
    alloy_transport::RpcError,
    std::{collections::HashSet, time::Duration},
    thiserror::Error,
    tracing::instrument,
};

pub(crate) const DEFAULT_VERIFICATION_TIMEOUT: Duration = Duration::from_secs(1);

#[derive(Debug, Error)]
pub enum SimulationError {
    #[error("simulation reverted {0:?}")]
    Revert(Option<String>),
    #[error(transparent)]
    Other(#[from] anyhow::Error),
}

#[derive(Debug, Error)]
pub enum DetectionError<E> {
    #[error("could not detect a balance override strategy")]
    NotFound,
    #[error("error returned by the RPC server")]
    Rpc(#[from] RpcError<E>),
    #[error(transparent)]
    Simulation(#[from] SimulationError),
}

/// Extracts storage slots accessed via SLOAD operations from a geth trace.
/// Returns slots in first-access order, keyed by the storage context
/// (contract).
#[instrument(skip_all, level = "trace")]
pub(crate) fn extract_sload_slots(
    trace: GethTrace,
    initial_storage_context: Address,
) -> Vec<(Address, B256)> {
    let mut storage_context = vec![initial_storage_context];
    let mut slots = Vec::new();
    let mut seen = HashSet::new();

    for log in &trace
        .try_into_default_frame()
        .unwrap_or_default()
        .struct_logs
    {
        let stack = log.stack.clone().unwrap_or_default();
        match log.op.as_ref() {
            "CALL" | "STATICCALL" if stack.len() >= 2 => {
                tracing::trace!("Detected CALL into nested contract");
                storage_context.push(Address::from_word(stack[stack.len() - 2].into()));
            }
            "DELEGATECALL" if stack.len() >= 2 => {
                storage_context.push(*storage_context.last().unwrap());
            }
            "RETURN" => {
                tracing::trace!("Detected RETURN from nested contract");
                if storage_context.is_empty() {
                    tracing::debug!(
                        "Too many RETURN opcodes (is there something wrong with the struct log?)"
                    );
                    break;
                }
                storage_context.pop();
            }
            "SLOAD" if !stack.is_empty() => {
                if let Some(current_storage) = storage_context.last() {
                    tracing::trace!(?stack, "Detected SLOAD");
                    let slot = *stack.last().unwrap();
                    if seen.insert((*current_storage, slot)) {
                        slots.push((*current_storage, slot.into()));
                    }
                } else {
                    tracing::debug!(
                        ?stack,
                        "SLOAD called when not in a call context (is something wrong with the \
                         struct log?)"
                    );
                    break;
                }
            }
            _ => {}
        }
    }

    slots
}

/// `keccak256(pad32(holder) ++ map_slot)` — the storage slot of
/// `mapping(address => _)` entries in Solidity.
pub(crate) fn mapping_slot_hash(holder: &Address, map_slot: &[u8; 32]) -> B256 {
    let mut buf = [0u8; 64];
    buf[12..32].copy_from_slice(holder.as_slice());
    buf[32..64].copy_from_slice(map_slot);
    keccak256(buf)
}
