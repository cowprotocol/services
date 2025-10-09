//! An `eth_call` simulation-based balance reading implementation. This allows
//! balances and allowances to be fetched as well as transfers to be verified
//! from a node in a single round-trip, while accounting for pre-interactions.

use {
    super::{BalanceFetching, Query, TransferSimulationError},
    crate::account_balances::{BalanceSimulator, SimulationError},
    anyhow::{Context, Result},
    contracts::{BalancerV2Vault, erc20::Contract},
    ethcontract::{Bytes, H160, U256, contract::MethodBuilder, dyns::DynTransport},
    ethrpc::Web3,
    futures::future,
    model::order::SellTokenSource,
    tracing::instrument,
    web3::{Transport, types::CallRequest},
};

pub struct Balances {
    web3: Web3,
    balance_simulator: BalanceSimulator,
}

impl Balances {
    pub fn new(web3: &Web3, balance_simulator: BalanceSimulator) -> Self {
        // Note that the balances simulation **will fail** if the `vault`
        // address is not a contract and the `source` is set to one of
        // `SellTokenSource::{External, Internal}` (i.e. the Vault contract is
        // needed). This is because Solidity generates code to verify that
        // contracts exist at addresses that get called. This allows us to
        // properly check if the `source` is not supported for the deployment
        // work without additional code paths :tada:!
        let web3 = ethrpc::instrumented::instrument_with_label(web3, "balanceFetching".into());

        Self {
            web3,
            balance_simulator,
        }
    }

    fn vault_relayer(&self) -> H160 {
        self.balance_simulator.vault_relayer
    }

    fn vault(&self) -> H160 {
        self.balance_simulator.vault
    }

    async fn tradable_balance_simulated(&self, query: &Query) -> Result<U256> {
        let simulation = self.simulate_with_access_list(query, None).await?;
        Ok(if simulation.can_transfer {
            simulation.effective_balance
        } else {
            U256::zero()
        })
    }

    async fn tradable_balance_simple(&self, query: &Query, token: &Contract) -> Result<U256> {
        let usable_balance = match query.source {
            SellTokenSource::Erc20 => {
                let balance = token.balance_of(query.owner).call();
                let allowance = token.allowance(query.owner, self.vault_relayer()).call();
                let (balance, allowance) = futures::try_join!(balance, allowance)?;
                std::cmp::min(balance, allowance)
            }
            SellTokenSource::External => {
                let vault = BalancerV2Vault::at(&self.web3, self.vault());
                let balance = token.balance_of(query.owner).call();
                let approved = vault
                    .methods()
                    .has_approved_relayer(query.owner, self.vault_relayer())
                    .call();
                let allowance = token.allowance(query.owner, self.vault()).call();
                let (balance, approved, allowance) =
                    futures::try_join!(balance, approved, allowance)?;
                match approved {
                    true => std::cmp::min(balance, allowance),
                    false => 0.into(),
                }
            }
            SellTokenSource::Internal => {
                let vault = BalancerV2Vault::at(&self.web3, self.vault());
                let balance = vault
                    .methods()
                    .get_internal_balance(query.owner, vec![query.token])
                    .call();
                let approved = vault
                    .methods()
                    .has_approved_relayer(query.owner, self.vault_relayer())
                    .call();
                let (balance, approved) = futures::try_join!(balance, approved)?;
                match approved {
                    true => balance[0], // internal approvals are always U256::MAX
                    false => 0.into(),
                }
            }
        };
        Ok(usable_balance)
    }

    async fn simulate_with_access_list(
        &self,
        query: &Query,
        amount: Option<U256>,
    ) -> std::result::Result<crate::account_balances::Simulation, SimulationError> {
        let should_add_access_list = !query.interactions.is_empty();
        let web3 = self.web3.clone();

        self.balance_simulator
            .simulate(
                query.owner,
                query.token,
                query.source,
                &query.interactions,
                amount,
                move |delegate_call| {
                    let web3 = web3.clone();
                    async move {
                        if should_add_access_list {
                            Self::apply_access_list(web3, delegate_call).await
                        } else {
                            delegate_call
                        }
                    }
                },
                query.balance_override.clone(),
            )
            .await
    }

    async fn apply_access_list(
        web3: Web3,
        delegate_call: MethodBuilder<DynTransport, Bytes<Vec<u8>>>,
    ) -> MethodBuilder<DynTransport, Bytes<Vec<u8>>> {
        match Self::fetch_access_list(web3, &delegate_call).await {
            Ok(Some(access_list)) if !access_list.is_empty() => {
                delegate_call.access_list(access_list)
            }
            Ok(_) => delegate_call,
            Err(err) => {
                tracing::debug!(
                    ?err,
                    "failed to generate access list for balance simulation"
                );
                delegate_call
            }
        }
    }

    async fn fetch_access_list(
        web3: Web3,
        delegate_call: &MethodBuilder<DynTransport, Bytes<Vec<u8>>>,
    ) -> Result<Option<web3::types::AccessList>> {
        let request = Self::call_request(delegate_call);
        let params = serde_json::to_value(&request)
            .context("serialize delegate call for eth_createAccessList")?;
        let response = web3
            .transport()
            .execute("eth_createAccessList", vec![params])
            .await
            .context("eth_createAccessList RPC call failed")?;

        if let Some(error) = response.get("error") {
            anyhow::bail!("eth_createAccessList returned error: {error}");
        }

        response
            .get("accessList")
            .cloned()
            .map(|value| {
                serde_json::from_value(value)
                    .context("failed to deserialize eth_createAccessList response")
            })
            .transpose()
    }

    fn call_request(delegate_call: &MethodBuilder<DynTransport, Bytes<Vec<u8>>>) -> CallRequest {
        let resolved_gas_price = delegate_call
            .tx
            .gas_price
            .map(|gas_price| gas_price.resolve_for_transaction())
            .unwrap_or_default();

        CallRequest {
            from: delegate_call
                .tx
                .from
                .as_ref()
                .map(|account| account.address()),
            to: delegate_call.tx.to,
            gas: delegate_call.tx.gas,
            gas_price: resolved_gas_price.gas_price,
            value: delegate_call.tx.value,
            data: delegate_call.tx.data.clone(),
            transaction_type: resolved_gas_price.transaction_type,
            access_list: delegate_call.tx.access_list.clone(),
            max_fee_per_gas: resolved_gas_price.max_fee_per_gas,
            max_priority_fee_per_gas: resolved_gas_price.max_priority_fee_per_gas,
            ..Default::default()
        }
    }
}

#[async_trait::async_trait]
impl BalanceFetching for Balances {
    #[instrument(skip_all)]
    async fn get_balances(&self, queries: &[Query]) -> Vec<Result<U256>> {
        // TODO(nlordell): Use `Multicall` here to use fewer node round-trips
        let futures = queries
            .iter()
            .map(|query| async {
                if query.interactions.is_empty() {
                    let token = contracts::ERC20::at(&self.web3, query.token);
                    self.tradable_balance_simple(query, &token).await
                } else {
                    self.tradable_balance_simulated(query).await
                }
            })
            .collect::<Vec<_>>();

        future::join_all(futures).await
    }

    async fn can_transfer(
        &self,
        query: &Query,
        amount: U256,
    ) -> Result<(), TransferSimulationError> {
        let simulation = self
            .simulate_with_access_list(query, Some(amount))
            .await
            .map_err(|err| TransferSimulationError::Other(err.into()))?;

        if simulation.token_balance < amount {
            return Err(TransferSimulationError::InsufficientBalance);
        }
        if simulation.allowance < amount {
            return Err(TransferSimulationError::InsufficientAllowance);
        }
        if !simulation.can_transfer {
            return Err(TransferSimulationError::TransferFailed);
        }

        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use {
        super::*,
        crate::price_estimation::trade_verifier::balance_overrides::DummyOverrider,
        ethcontract::common::abi::{Function, StateMutability},
        ethcontract::{
            contract::MethodBuilder,
            dyns::DynTransport,
            transaction::{Account, GasPrice},
        },
        ethrpc::{
            Web3,
            mock::{self, MockTransport},
        },
        model::order::SellTokenSource,
        primitive_types::H256,
        serde_json::{Value, json},
        std::sync::Arc,
        web3::types::{AccessListItem, Bytes as Web3Bytes, CallRequest, U64},
    };

    const ACCESS_LIST_ADDRESS: H160 = H160([0x11; 20]);

    fn dummy_function() -> Function {
        #[allow(deprecated)]
        {
            Function {
                name: "dummy".into(),
                inputs: vec![],
                outputs: vec![],
                constant: None,
                state_mutability: StateMutability::NonPayable,
            }
        }
    }

    fn method_builder(transport: &MockTransport) -> MethodBuilder<DynTransport, Bytes<Vec<u8>>> {
        let dyn_transport = DynTransport::new(transport.clone());
        let method_web3 = web3::Web3::new(dyn_transport);
        MethodBuilder::new(
            method_web3,
            dummy_function(),
            ACCESS_LIST_ADDRESS,
            Web3Bytes(vec![0xde, 0xad, 0xbe, 0xef]),
        )
    }

    fn mock_web3_with_transport() -> (Web3, MockTransport) {
        let mock = mock::web3();
        let transport = mock.legacy.transport().clone();
        let dyn_transport = DynTransport::new(transport.clone());
        (
            Web3 {
                legacy: web3::Web3::new(dyn_transport),
                alloy: mock.alloy,
                wallet: mock.wallet,
            },
            transport,
        )
    }

    fn expect_access_list_request(
        transport: &MockTransport,
        expected_request: CallRequest,
        response: Value,
    ) {
        transport
            .mock()
            .expect_execute()
            .once()
            .returning(move |method, params| {
                assert_eq!(method, "eth_createAccessList");
                assert_eq!(params.len(), 1);
                let actual: CallRequest = serde_json::from_value(params[0].clone()).unwrap();
                assert_eq!(actual, expected_request);
                Ok(response.clone())
            });
    }

    #[test]
    fn call_request_copies_transaction_fields() {
        let transport = MockTransport::new();
        let from = H160([0x22; 20]);
        let delegate_call = method_builder(&transport)
            .from(Account::Local(from, None))
            .gas(123.into())
            .gas_price(GasPrice::Eip1559 {
                max_fee_per_gas: 50.into(),
                max_priority_fee_per_gas: 4.into(),
            })
            .value(456.into())
            .nonce(789.into())
            .access_list(vec![AccessListItem {
                address: H160([0x33; 20]),
                storage_keys: vec![H256::from_low_u64_be(1)],
            }]);

        let request = Balances::call_request(&delegate_call);
        assert_eq!(request.from, Some(from));
        assert_eq!(request.to, Some(ACCESS_LIST_ADDRESS));
        assert_eq!(request.gas, Some(123.into()));
        assert_eq!(request.value, Some(456.into()));
        assert_eq!(request.data, delegate_call.tx.data.clone());
        assert_eq!(
            request.access_list,
            Some(vec![AccessListItem {
                address: H160([0x33; 20]),
                storage_keys: vec![H256::from_low_u64_be(1)],
            }])
        );
        assert_eq!(request.transaction_type, Some(U64::from(2))); // EIP-1559
        assert_eq!(request.max_fee_per_gas, Some(50.into()));
        assert_eq!(request.max_priority_fee_per_gas, Some(4.into()));
        assert_eq!(request.gas_price, None); // Should be None for EIP-1559
    }

    #[tokio::test]
    async fn fetch_access_list_deserializes_response() {
        let (web3, transport) = mock_web3_with_transport();
        let delegate_transport = MockTransport::new();
        let delegate_call = method_builder(&delegate_transport).access_list(vec![AccessListItem {
            address: H160([0x44; 20]),
            storage_keys: vec![H256::from_low_u64_be(2)],
        }]);
        let expected_request = Balances::call_request(&delegate_call);
        let expected_access_list = vec![AccessListItem {
            address: H160([0x55; 20]),
            storage_keys: vec![H256::from_low_u64_be(3)],
        }];
        let response = json!({ "accessList": expected_access_list });

        expect_access_list_request(&transport, expected_request, response);

        let result = Balances::fetch_access_list(web3, &delegate_call)
            .await
            .expect("rpc call succeeds");
        assert_eq!(result, Some(expected_access_list));
    }

    #[tokio::test]
    async fn fetch_access_list_propagates_rpc_errors() {
        let (web3, transport) = mock_web3_with_transport();
        let delegate_call = method_builder(&MockTransport::new());
        let expected_request = Balances::call_request(&delegate_call);
        let error_response = json!({ "error": "boom" });

        expect_access_list_request(&transport, expected_request, error_response);

        let err = Balances::fetch_access_list(web3, &delegate_call)
            .await
            .expect_err("error is propagated");
        assert!(
            err.to_string()
                .contains("eth_createAccessList returned error")
        );
    }

    #[tokio::test]
    async fn apply_access_list_sets_non_empty_result() {
        let (web3, transport) = mock_web3_with_transport();
        let delegate_call = method_builder(&MockTransport::new());
        let expected_request = Balances::call_request(&delegate_call);
        let expected_access_list = vec![AccessListItem {
            address: H160([0x66; 20]),
            storage_keys: vec![H256::from_low_u64_be(4)],
        }];
        let response = json!({ "accessList": expected_access_list.clone() });

        expect_access_list_request(&transport, expected_request, response);

        let updated = Balances::apply_access_list(web3, delegate_call).await;
        assert_eq!(updated.tx.access_list, Some(expected_access_list));
    }

    #[tokio::test]
    async fn apply_access_list_ignores_empty_or_failed_lists() {
        let (web3_empty, transport_empty) = mock_web3_with_transport();
        let delegate_call_empty = method_builder(&MockTransport::new());
        let expected_request_empty = Balances::call_request(&delegate_call_empty);
        expect_access_list_request(
            &transport_empty,
            expected_request_empty,
            json!({ "accessList": [] }),
        );
        let updated_empty = Balances::apply_access_list(web3_empty, delegate_call_empty).await;
        assert!(updated_empty.tx.access_list.is_none());

        let (web3_error, transport_error) = mock_web3_with_transport();
        let delegate_call_error = method_builder(&MockTransport::new());
        let expected_request_error = Balances::call_request(&delegate_call_error);
        expect_access_list_request(
            &transport_error,
            expected_request_error,
            json!({ "error": "nope" }),
        );
        let updated_error = Balances::apply_access_list(web3_error, delegate_call_error).await;
        assert!(updated_error.tx.access_list.is_none());
    }

    #[ignore]
    #[tokio::test]
    async fn test_for_user() {
        let web3 = Web3::new_from_env();
        let settlement =
            contracts::GPv2Settlement::at(&web3, addr!("9008d19f58aabd9ed0d60971565aa8510560ab41"));
        let balances = contracts::support::Balances::at(
            &web3,
            addr!("3e8C6De9510e7ECad902D005DE3Ab52f35cF4f1b"),
        );
        let balances = Balances::new(
            &web3,
            BalanceSimulator::new(
                settlement,
                balances,
                addr!("C92E8bdf79f0507f65a392b0ab4667716BFE0110"),
                Some(addr!("BA12222222228d8Ba445958a75a0704d566BF2C8")),
                Arc::new(DummyOverrider),
            ),
        );

        let owner = addr!("b0a4e99371dfb0734f002ae274933b4888f618ef");
        let token = addr!("d909c5862cdb164adb949d92622082f0092efc3d");
        let amount = 50000000000000000000000_u128.into();
        let source = SellTokenSource::Erc20;

        balances
            .can_transfer(
                &Query {
                    owner,
                    token,
                    source,
                    interactions: vec![],
                    balance_override: None,
                },
                amount,
            )
            .await
            .unwrap();
        println!("{owner:?} can transfer {amount} {token:?}!");
    }
}
