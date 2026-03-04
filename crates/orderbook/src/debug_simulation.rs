use {
    alloy::{
        primitives::{Address, Bytes, U256, address, map::AddressMap},
        providers::Provider,
        rpc::types::state::{AccountOverride, StateOverride},
        sol_types::SolCall,
    },
    anyhow::{Context, Result},
    app_data::Root as AppDataRoot,
    balance_overrides::{BalanceOverrideRequest, BalanceOverriding},
    contracts::alloy::{GPv2Settlement, ICowWrapper, support::AnyoneAuthenticator},
    ethrpc::Web3,
    model::order::Order,
    shared::{
        encoded_settlement::{EncodedSettlement, encode_trade},
        interaction::{EncodedInteraction, Interaction},
        price_estimation::trade_verifier::{
            legacy_settlement_to_alloy,
            tenderly_api::{SimulationRequest, StateObject, TenderlyApi},
        },
    },
    std::sync::Arc,
};

/// Fixed fake solver address used for simulations.
const FAKE_SOLVER: Address = address!("0101010101010101010101010101010101010101");

/// A wrapper call: target address + calldata.
type WrapperCalls = Vec<(Address, Vec<u8>)>;

pub struct OrderSimulator {
    web3: Web3,
    settlement_contract: GPv2Settlement::Instance,
    balance_overrides: Arc<dyn BalanceOverriding>,
    tenderly: Option<Arc<dyn TenderlyApi>>,
    chain_id: u64,
}

pub struct SimulationResult {
    pub succeeded: bool,
    pub gas_estimate: Option<u64>,
    pub revert_reason: Option<String>,
    pub tenderly_url: Option<String>,
    pub from: Address,
    pub call_target: Address,
    pub used_wrapper: bool,
    pub block_number: u64,
    pub calldata: Vec<u8>,
    pub state_overrides: StateOverride,
}

impl OrderSimulator {
    pub fn new(
        web3: Web3,
        settlement_contract: GPv2Settlement::Instance,
        balance_overrides: Arc<dyn BalanceOverriding>,
        tenderly: Option<Arc<dyn TenderlyApi>>,
        chain_id: u64,
    ) -> Self {
        Self {
            web3,
            settlement_contract,
            balance_overrides,
            tenderly,
            chain_id,
        }
    }

    pub async fn simulate(
        &self,
        order: &Order,
        create_tenderly_simulation: bool,
    ) -> Result<SimulationResult> {
        let settlement_address = *self.settlement_contract.address();

        // Build settlement tx calldata with optional wrappers
        let (settlement, wrappers) = self.build_settlement(order)?;
        let overrides = self.build_state_overrides(order).await?;
        let settle_call = legacy_settlement_to_alloy(settlement);
        let settle_calldata = settle_call.abi_encode();
        let (call_target, final_calldata, used_wrapper) = if !wrappers.is_empty() {
            let wrapper_data = encode_wrapper_data(&wrappers);
            let wrapped_calldata = ICowWrapper::ICowWrapper::wrappedSettleCall {
                settleData: settle_calldata.into(),
                wrapperData: wrapper_data.into(),
            }
            .abi_encode();
            (wrappers[0].0, wrapped_calldata, true)
        } else {
            (settlement_address, settle_calldata, false)
        };

        let block_number = self
            .web3
            .provider
            .get_block_number()
            .await
            .context("failed to get block number")?;
        let tx = alloy::rpc::types::TransactionRequest {
            from: Some(FAKE_SOLVER),
            to: Some(call_target.into()),
            input: alloy::rpc::types::TransactionInput::new(final_calldata.clone().into()),
            gas: Some(10_000_000),
            ..Default::default()
        };
        // Execute eth_call
        let result = self
            .web3
            .provider
            .call(tx.clone())
            .overrides(overrides.clone())
            .block(block_number.into())
            .await;

        let (succeeded, gas_used, revert_reason) = match &result {
            Ok(_) => {
                let gas = self
                    .web3
                    .provider
                    .estimate_gas(tx.clone())
                    .overrides(overrides.clone())
                    .block(block_number.into())
                    .await
                    .ok();
                (true, gas, None)
            }
            Err(err) => {
                let reason = extract_revert_reason(err);
                (false, None, Some(reason))
            }
        };

        // Create Tenderly simulation if configured
        let tenderly_url = if let Some(tenderly) = &self.tenderly {
            // Successful txs don't usually need, but provide it if user explicitly
            // requested it
            if !succeeded || create_tenderly_simulation {
                match self
                    .create_tenderly_simulation(
                        tenderly,
                        &final_calldata,
                        call_target,
                        &overrides,
                        block_number,
                    )
                    .await
                {
                    Ok(url) => Some(url),
                    Err(err) => {
                        tracing::warn!(?err, "failed to create Tenderly simulation");
                        None
                    }
                }
            } else {
                None
            }
        } else {
            None
        };

        Ok(SimulationResult {
            succeeded,
            gas_estimate: gas_used,
            revert_reason,
            tenderly_url,
            from: FAKE_SOLVER,
            call_target,
            used_wrapper,
            block_number,
            calldata: final_calldata,
            state_overrides: overrides,
        })
    }

    fn build_settlement(&self, order: &Order) -> Result<(EncodedSettlement, WrapperCalls)> {
        let tokens = vec![order.data.sell_token, order.data.buy_token];
        let clearing_prices = vec![order.data.buy_amount, order.data.sell_amount];

        let trade = encode_trade(
            &order.data,
            &order.signature,
            order.metadata.owner,
            0,
            1,
            U256::ZERO,
        );

        let pre_hooks: Vec<EncodedInteraction> = order
            .interactions
            .pre
            .iter()
            .map(Interaction::encode)
            .collect();
        let post_hooks: Vec<EncodedInteraction> = order
            .interactions
            .post
            .iter()
            .map(Interaction::encode)
            .collect();

        let settlement = EncodedSettlement {
            tokens,
            clearing_prices,
            trades: vec![trade],
            interactions: [pre_hooks, vec![], post_hooks],
        };

        let wrappers = parse_wrappers(order);

        Ok((settlement, wrappers))
    }

    async fn build_state_overrides(&self, order: &Order) -> Result<StateOverride> {
        let mut overrides: AddressMap<AccountOverride> = AddressMap::default();

        // Override authenticator with AnyoneAuthenticator so our fake solver is
        // accepted.
        let authenticator = self
            .settlement_contract
            .authenticator()
            .call()
            .await
            .context("failed to fetch authenticator address")?;
        overrides.insert(
            authenticator,
            AccountOverride {
                code: Some(Bytes::from(
                    AnyoneAuthenticator::AnyoneAuthenticator::DEPLOYED_BYTECODE.to_vec(),
                )),
                ..Default::default()
            },
        );

        // Give the fake solver some ETH balance.
        overrides.insert(
            FAKE_SOLVER,
            AccountOverride {
                balance: Some(U256::from(1_000_000_000_000_000_000u128)),
                ..Default::default()
            },
        );

        // Fund the settlement contract with enough buy tokens to pay out.
        if let Some((token_addr, balance_override)) = self
            .balance_overrides
            .state_override(BalanceOverrideRequest {
                token: order.data.buy_token,
                holder: *self.settlement_contract.address(),
                amount: order.data.buy_amount,
            })
            .await
        {
            overrides.insert(token_addr, balance_override);
        }

        Ok(overrides)
    }

    async fn create_tenderly_simulation(
        &self,
        tenderly: &Arc<dyn TenderlyApi>,
        calldata: &[u8],
        target: Address,
        overrides: &StateOverride,
        block_number: u64,
    ) -> Result<String> {
        let state_objects = overrides
            .iter()
            .map(|(key, value)| Ok((*key, StateObject::try_from(value.clone())?)))
            .collect::<Result<_>>()?;

        let request = SimulationRequest {
            network_id: self.chain_id.to_string(),
            block_number: Some(block_number),
            transaction_index: Some(-1),
            from: FAKE_SOLVER,
            to: target,
            input: calldata.to_vec(),
            save: Some(true),
            save_if_fails: Some(true),
            state_objects: Some(state_objects),
            ..Default::default()
        };

        let response = tenderly
            .simulate(request)
            .await
            .context("Tenderly simulation failed")?;

        let sim_id = &response.simulation.id;
        let url = if tenderly.share_simulation(sim_id).await.is_ok() {
            tenderly.shared_simulation_url(sim_id)
        } else {
            tracing::warn!(sim_id, "failed to make Tenderly simulation public");
            tenderly.simulation_url(sim_id)
        };

        Ok(url.to_string())
    }
}

/// Parse wrapper calls from the order's fullAppData.
fn parse_wrappers(order: &Order) -> WrapperCalls {
    let Some(full_app_data) = &order.metadata.full_app_data else {
        return vec![];
    };
    let Ok(root) = serde_json::from_str::<AppDataRoot>(full_app_data) else {
        return vec![];
    };
    let Some(metadata) = root.metadata() else {
        return vec![];
    };
    metadata
        .wrappers
        .iter()
        .map(|w| (w.address, w.data.clone()))
        .collect()
}

/// Encode wrapper metadata for wrappedSettle calls.
///
/// Format: for each wrapper after the first, 20 bytes address is prepended.
/// For each wrapper: 2 bytes data length (big-endian u16) + data bytes.
/// The first wrapper's address is omitted (it's the transaction target).
fn encode_wrapper_data(wrappers: &WrapperCalls) -> Vec<u8> {
    let mut result = Vec::new();
    for (index, (address, data)) in wrappers.iter().enumerate() {
        if index != 0 {
            result.extend(address.as_slice());
        }
        result.extend((data.len() as u16).to_be_bytes());
        result.extend(data);
    }
    result
}

fn extract_revert_reason(
    err: &alloy::transports::RpcError<alloy::transports::TransportErrorKind>,
) -> String {
    format!("{err:#}")
}
