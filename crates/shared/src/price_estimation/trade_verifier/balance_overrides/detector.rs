use {
    super::Strategy,
    crate::tenderly_api::SimulationError,
    anyhow::Context,
    contracts::alloy::ERC20,
    ethcontract::{Address, H160, H256, U256, state_overrides::StateOverride},
    ethrpc::{
        alloy::conversions::{IntoAlloy},
        extensions::DebugNamespace,
    },
    maplit::hashmap,
    std::{
        collections::HashMap,
        fmt::{self, Debug, Formatter},
        sync::Arc,
    },
    thiserror::Error,
    web3::types::{BlockNumber, CallRequest},
};

/// A heuristic balance override detector based on `eth_call` simulations.
///
/// This has the exact same node requirements as trade verification.
#[derive(Clone)]
pub struct Detector(Arc<Inner>);

pub struct Inner {
    probing_depth: u8,
    web3: ethrpc::Web3,
}

impl std::ops::Deref for Detector {
    type Target = Inner;

    fn deref(&self) -> &Self::Target {
        &self.0
    }
}

fn merge_state_overrides(overrides: Vec<HashMap<H160, StateOverride>>) -> HashMap<H160, StateOverride> {
    let mut merged = HashMap::new();

    for override_map in overrides {
        for (address, state_override) in override_map {
            merged.entry(address)
                .and_modify(|existing: &mut StateOverride| {
                    if let (Some(existing_diff), Some(new_diff)) = (&mut existing.state_diff, &state_override.state_diff) {
                        existing_diff.extend(new_diff.clone());
                    }
                })
                .or_insert(state_override);
        }
    }

    merged
}

impl Detector {
    /// Creates a new balance override detector.
    pub fn new(web3: ethrpc::Web3, probing_depth: u8) -> Self {

        Self(Arc::new(Inner { web3, probing_depth }))
    }

    fn generate_strategies(&self, target_contract: H160) -> Vec<StrategyHelper> {
        // First test storage slots that don't need guesswork.
        let mut strategies = vec![
            Strategy::SoladyMapping { target_contract },
            Strategy::SolidityMapping {
                target_contract,
                map_slot: U256::from(OPEN_ZEPPELIN_ERC20_UPGRADEABLE),
            },
        ];

        // For each entry point probe the first n following slots.
        let entry_points = [
            // solc lays out memory linearly starting at 0 by default
            "0000000000000000000000000000000000000000000000000000000000000000",
        ];
        for start_slot in entry_points {
            let mut map_slot = U256::from(start_slot);
            for _ in 0..self.probing_depth {
                strategies.push(Strategy::SolidityMapping { target_contract: H160::default(), map_slot });
                map_slot += U256::one();
            }
        }

        strategies
            .into_iter()
            .enumerate()
            .map(|(index, strategy)| StrategyHelper::new(strategy, index))
            .collect()
    }

    /// Tries to detect the balance override strategy for the specified token.
    /// Returns an `Err` if it cannot detect the strategy or an internal
    /// simulation fails.
    pub async fn detect(
        &self,
        token: Address,
        holder: Address,
    ) -> Result<Strategy, DetectionError> {

        // First try the trace-based detection if the node supports it
        let trace_strategy = self.detect_with_trace(token, holder).await;
        if let Ok(strategy) = trace_strategy {
            tracing::debug!(
                ?token,
                ?strategy,
                "Trace-based detection succeeded"
            );
            return Ok(strategy);
        } else {
            tracing::debug!(
                ?token,
                ?trace_strategy,
                "Trace-based detection failed, falling back to heuristic detection",
            );
        }

        // Fall back to the original heuristic-based detection
        let strategies = self.generate_strategies(token);
        let token = ERC20::Instance::new(token.into_alloy(), self.web3.alloy.clone());
        let overrides = merge_state_overrides(
            strategies
                .iter()
                .map(|helper| helper.strategy.state_override(&holder, &helper.balance))
                .collect()
        );

        let balance = token
            .balanceOf(holder.into_alloy())
            .state(overrides.into_alloy())
            .call()
            .await
            .context("eth_call with state overrides failed")
            .map_err(|e| DetectionError::Simulation(SimulationError::Other(e)))?;

        strategies
            .iter()
            .find_map(|helper| {
                (helper.balance.into_alloy() == balance).then_some(helper.strategy.clone())
            })
            .ok_or(DetectionError::NotFound)
    }

    /// Detects the balance storage slot using debug_traceCall, similar to
    /// Foundry's `deal`. This traces a balanceOf call and finds which
    /// storage slot is accessed.
    async fn detect_with_trace(
        &self,
        token: Address,
        holder: Address,
    ) -> Result<Strategy, DetectionError> {
        use {alloy::sol_types::SolCall, web3::types::Bytes};

        let balance_of_call = ERC20::ERC20::balanceOfCall {
            account: holder.into_alloy(),
        };
        let calldata = balance_of_call.abi_encode();

        let call_request = CallRequest {
            to: Some(token),
            data: Some(Bytes(calldata)),
            ..Default::default()
        };

        let trace = self
            .web3
            .legacy
            .debug()
            .trace_call(call_request, BlockNumber::Latest.into())
            .await
            .map_err(|e| {
                tracing::debug!(?token, error = ?e, "debug_traceCall not supported for token");
                DetectionError::Simulation(SimulationError::Other(anyhow::anyhow!(
                    "debug_traceCall failed: {e}"
                )))
            })?;

        // Extract storage slots accessed via SLOAD operations
        let storage_slots = self.extract_sload_slots(&trace, token);

        if storage_slots.is_empty() {
            tracing::debug!("no SLOAD operations found in trace for token {:?}", token);
            return Err(DetectionError::NotFound);
        }

        if storage_slots.len() == 1 {
            let slot = storage_slots[0];
            tracing::debug!(
                storage_context = ?slot.0,
                slot = ?slot.1,
                ?slot,
                iterations = 0,
                "detected balance slot via trace (single SLOAD) for token",
            );
            return Ok(Strategy::DirectSlot { target_contract: slot.0, slot: slot.1 });
        }

        // Multiple storage slots accessed - test each one to find the balance slot
        tracing::debug!(
            ?token,
            total = storage_slots.len(),
            "multiple SLOAD operations, testing each one",
        );

        // Iterate through slots in reverse order (last accessed is most likely the
        // balance)
        // We check slots individually/one at a time instead of all at once because
        // changing unnecessary storage slots could negatively affect the execution (ex. overriding an upgradable proxy contract target)
        for (i, slot) in storage_slots.iter().rev().enumerate() {
            if let Ok(strategy) = self.verify_slot_is_balance(token, holder, *slot).await {
                tracing::debug!(
                    ?token,
                    ?holder,
                    ?slot,
                    iterations = i + 1,
                    total = storage_slots.len(),
                    "verified balance SLOAD slot via testing",
                );
                return Ok(strategy);
            }
        }

        tracing::debug!(
            "none of the SLOAD slots appear to be the balance slot for token {:?}",
            token
        );

        Err(DetectionError::NotFound)
    }

    /// Extracts storage slots accessed via SLOAD operations from struct logs.
    /// Returns slots in the order they were accessed.
    fn extract_sload_slots(&self, trace: &ethrpc::extensions::StructLogTrace, initial_storage_context: H160) -> Vec<(H160, H256)> {
        let mut storage_context: H160 = initial_storage_context;
        let mut slots = Vec::new();
        let mut seen = std::collections::HashSet::new();

        for log in &trace.struct_logs {
            if log.op == "CALL" || log.op == "STATICCALL" && !log.stack.len() >= 2 {
                // CALL opcode takes the address of the contract to call as second element
                storage_context = H160::from(log.stack[log.stack.len() - 2]);
            }
            // SLOAD opcode reads from storage
            // The storage key is on top of the stack (last element)
            if log.op == "SLOAD" && !log.stack.is_empty() {
                // Stack grows upward, so the top is the last element
                let slot = *log.stack.last().unwrap();

                // Only add unique slots, preserving order
                if seen.insert(slot) {
                    slots.push((storage_context, slot));
                }
            }
        }

        slots
    }

    /// Verifies that a storage slot controls the balance by setting it and
    /// checking balanceOf.
    async fn verify_slot_is_balance(
        &self,
        token: Address,
        holder: Address,
        slot: (H160, H256),
    ) -> Result<Strategy, DetectionError> {
        // Use a unique test value to verify this is the balance slot
        let test_balance = U256::from(0x1337_1337_1337_1337_u64);

        // Create state override with the test value in this slot
        let mut slot_value = [0u8; 32];
        test_balance.to_big_endian(&mut slot_value);

        let overrides = hashmap! {
            slot.0 => StateOverride {
                state_diff: Some(hashmap! {
                    slot.1 => H256(slot_value),
                }),
                ..Default::default()
            },
        };

        // Call balanceOf with the override
        let token_contract = ERC20::Instance::new(token.into_alloy(), self.web3.alloy.clone());
        let balance = token_contract
            .balanceOf(holder.into_alloy())
            .state(overrides.into_alloy())
            .call()
            .await
            .map_err(|e| {
                DetectionError::Simulation(SimulationError::Other(anyhow::anyhow!(
                    "balanceOf call failed during slot verification: {e}"
                )))
            })?;

        // If the balance matches our test value, we found the right slot
        if balance == test_balance.into_alloy() {
            Ok(Strategy::DirectSlot { target_contract: slot.0, slot: slot.1 })
        } else {
            Err(DetectionError::NotFound)
        }
    }
}

/// Contains all the information we need to determine which state override
/// was successful.
struct StrategyHelper {
    /// strategy that was used to compute the state override
    strategy: Strategy,
    /// balance amount the strategy wrote into the storage
    balance: U256,
}

impl StrategyHelper {
    fn new(strategy: Strategy, index: usize) -> Self {
        let index = u8::try_from(index).expect("unreasonable amount of strategies used");
        Self {
            strategy,
            // Use an exact value which isn't too large or too small. This helps
            // not have false positives for cases where the token balances in
            // some other denomination from the actual token balance (such as
            // stETH for example) and not run into issues with overflows.
            // We also make sure that we avoid 0 because `balanceOf()` returns
            // 0 by default so we can't use it to detect successful state overrides.
            balance: U256::from(u64::from_be_bytes([index + 1; 8])),
        }
    }
}

// <https://github.com/OpenZeppelin/openzeppelin-contracts-upgradeable/blob/master/contracts/token/ERC20/ERC20Upgradeable.sol#L43-L44>
const OPEN_ZEPPELIN_ERC20_UPGRADEABLE: &str =
    "52c63247e1f47db19d5ce0460030c497f067ca4cebf71ba98eeadabe20bace00";

impl Debug for Detector {
    fn fmt(&self, f: &mut Formatter) -> fmt::Result {
        f.debug_struct("Detector")
            .field("simulator", &format_args!("Arc<dyn CodeSimulating>"))
            .finish()
    }
}

/// An error detecting the balance override strategy for a token.
#[derive(Debug, Error)]
pub enum DetectionError {
    #[error("could not detect a balance override strategy")]
    NotFound,
    #[error(transparent)]
    Simulation(#[from] SimulationError),
}

#[cfg(test)]
mod tests {
    use {super::*, ethrpc::Web3};

    /// Tests that we can detect storage slots by probing the first
    /// n slots or by checking hardcoded known slots.
    /// Set `NODE_URL` environment to a mainnet RPC URL.
    #[ignore]
    #[tokio::test]
    async fn detects_storage_slots_mainnet() {
        let detector = Detector::new(Web3::new_from_env(), 60);

        let storage = detector
            .detect(
                addr!("c02aaa39b223fe8d0a0e5c4f27ead9083c756cc2"),
                addr!("0000000000000000000000000000000000000001"),
            )
            .await
            .unwrap();
        assert_eq!(storage, Strategy::SolidityMapping { 
            target_contract: addr!("c02aaa39b223fe8d0a0e5c4f27ead9083c756cc2"),
            map_slot: 3.into()
        });

        let storage = detector
            .detect(
                addr!("4956b52ae2ff65d74ca2d61207523288e4528f96"),
                addr!("0000000000000000000000000000000000000001"),
            )
            .await
            .unwrap();
        assert_eq!(
            storage,
            Strategy::SolidityMapping {
                target_contract: addr!("4956b52ae2ff65d74ca2d61207523288e4528f96"),
                map_slot: U256::from(OPEN_ZEPPELIN_ERC20_UPGRADEABLE),
            }
        );

        let storage = detector
            .detect(
                addr!("0000000000c5dc95539589fbd24be07c6c14eca4"),
                addr!("0000000000000000000000000000000000000001"),
            )
            .await
            .unwrap();
        assert_eq!(storage, Strategy::SoladyMapping { target_contract: addr!("0000000000c5dc95539589fbd24be07c6c14eca4") });
    }

    /// Tests that we can detect storage slots by probing the first
    /// n slots or by checking hardcoded known slots.
    /// Set `NODE_URL` environment to an arbitrum RPC URL.
    #[ignore]
    #[tokio::test]
    async fn detects_storage_slots_arbitrum() {
        let detector = Detector::new(Web3::new_from_env(), 60);

        // all bridged tokens on arbitrum require a ton of probing
        let storage = detector
            .detect(
                addr!("ff970a61a04b1ca14834a43f5de4533ebddb5cc8"),
                addr!("0000000000000000000000000000000000000001"),
            )
            .await
            .unwrap();
        assert_eq!(storage, Strategy::SolidityMapping { target_contract: addr!("ff970a61a04b1ca14834a43f5de4533ebddb5cc8"), map_slot: 51.into() });
    }
}
