use {
    anyhow::{anyhow, Context, Error, Result},
    contracts::GPv2Settlement,
    ethcontract::{
        batch::CallBatch,
        contract::MethodBuilder,
        dyns::{DynMethodBuilder, DynTransport},
        errors::ExecutionError,
        transaction::TransactionBuilder,
        Account,
    },
    ethrpc::Web3,
    futures::FutureExt,
    gas_estimation::GasPrice1559,
    itertools::Itertools,
    primitive_types::{H160, H256, U256},
    shared::{
        conversions::into_gas_price,
        encoded_settlement::EncodedSettlement,
        tenderly_api::{SimulationRequest, TenderlyApi},
    },
    web3::types::{AccessList, BlockId},
};

const SIMULATE_BATCH_SIZE: usize = 10;

pub async fn simulate_and_estimate_gas_at_current_block(
    settlements: impl Iterator<Item = (Account, EncodedSettlement, Option<AccessList>)>,
    contract: &GPv2Settlement,
    gas_price: GasPrice1559,
) -> Result<Vec<Result<U256, ExecutionError>>> {
    // Collect into Vec to not rely on Itertools::chunk which would make this future
    // !Send.
    let settlements: Vec<_> = settlements.collect();

    // Force settlement simulations to be done in smaller batches. They can be
    // quite large and exert significant node pressure.
    let mut results = Vec::new();
    for chunk in settlements.chunks(SIMULATE_BATCH_SIZE) {
        let calls = chunk
            .iter()
            .map(|(account, settlement, access_list)| {
                let tx = settle_method(gas_price, contract, settlement.clone(), account.clone()).tx;
                let tx = match access_list {
                    Some(access_list) => tx.access_list(access_list.clone()),
                    None => tx,
                };
                tx.estimate_gas()
            })
            .collect::<Vec<_>>();
        let chuck_results = futures::future::join_all(calls).await;
        results.extend(chuck_results);
    }

    Ok(results)
}

pub async fn simulate_and_error_with_tenderly_link(
    settlements: impl Iterator<Item = (Account, EncodedSettlement, Option<AccessList>)>,
    contract: &GPv2Settlement,
    web3: &Web3,
    gas_price: GasPrice1559,
    network_id: &str,
    block: u64,
    simulation_gas_limit: u128,
) -> Vec<Result<()>> {
    let mut batch = CallBatch::new(web3.transport());
    let futures = settlements
        .map(|(account, settlement, access_list)| {
            let method = settle_method(gas_price, contract, settlement, account);
            let method = match access_list {
                Some(access_list) => method.access_list(access_list),
                None => method,
            };
            let transaction_builder = method.tx.clone();
            let view = method
                .view()
                .block(BlockId::Number(block.into()))
                // Since we now supply the gas price for the simulation, make sure to also
                // set a gas limit so we don't get failed simulations because of insufficient
                // solver balance. The limit should be below the current block gas
                // limit of 30M gas
                .gas(simulation_gas_limit.into());
            (view.batch_call(&mut batch), transaction_builder)
        })
        .collect::<Vec<_>>();
    batch.execute_all(SIMULATE_BATCH_SIZE).await;

    futures
        .into_iter()
        .map(|(future, transaction_builder)| {
            future.now_or_never().unwrap().map(|_| ()).map_err(|err| {
                Error::new(err).context(tenderly_link(
                    block,
                    network_id,
                    transaction_builder,
                    Some(gas_price),
                    None,
                ))
            })
        })
        .collect()
}

pub async fn simulate_before_after_access_list(
    web3: &Web3,
    tenderly: &dyn TenderlyApi,
    network_id: String,
    transaction_hash: H256,
) -> Result<f64> {
    let transaction = web3
        .eth()
        .transaction(transaction_hash.into())
        .await?
        .context("no transaction found")?;

    if transaction.access_list.is_none() {
        return Err(anyhow!(
            "no need to analyze access list since no access list was found in mined transaction"
        ));
    }

    let (block_number, from, to, transaction_index) = (
        transaction
            .block_number
            .context("no block number field exist")?
            .as_u64(),
        transaction.from.context("no from field exist")?,
        transaction.to.context("no to field exist")?,
        transaction
            .transaction_index
            .context("no transaction_index field exist")?
            .as_u64(),
    );

    let request = SimulationRequest {
        network_id,
        block_number: Some(block_number),
        transaction_index: Some(transaction_index),
        from,
        input: transaction.input.0,
        to,
        gas: Some(transaction.gas.as_u64()),
        ..Default::default()
    };

    let gas_used_without_access_list = tenderly.simulate(request).await?.transaction.gas_used;
    let gas_used_with_access_list = web3
        .eth()
        .transaction_receipt(transaction_hash)
        .await?
        .context("no transaction receipt")?
        .gas_used
        .context("no gas used field")?;

    Ok(gas_used_without_access_list as f64 - gas_used_with_access_list.to_f64_lossy())
}

pub fn settle_method(
    gas_price: GasPrice1559,
    contract: &GPv2Settlement,
    settlement: EncodedSettlement,
    account: Account,
) -> MethodBuilder<DynTransport, ()> {
    settle_method_builder(contract, settlement, account).gas_price(into_gas_price(&gas_price))
}

pub fn settle_method_builder(
    contract: &GPv2Settlement,
    settlement: EncodedSettlement,
    from: Account,
) -> DynMethodBuilder<()> {
    contract
        .settle(
            settlement.tokens,
            settlement.clearing_prices,
            settlement.trades,
            settlement.interactions,
        )
        .from(from)
}

/// The call data of a settle call with this settlement.
pub fn call_data(settlement: EncodedSettlement) -> Vec<u8> {
    let contract = GPv2Settlement::at(&ethrpc::dummy::web3(), H160::default());
    let method = contract.settle(
        settlement.tokens,
        settlement.clearing_prices,
        settlement.trades,
        settlement.interactions,
    );
    // Unwrap because there should always be calldata.
    method.tx.data.unwrap().0
}

// Creates a simulation link in the gp-v2 tenderly workspace
pub fn tenderly_link(
    current_block: u64,
    network_id: &str,
    tx: TransactionBuilder<DynTransport>,
    gas_price: Option<GasPrice1559>,
    access_list: Option<AccessList>,
) -> String {
    // Tenderly simulates transactions for block N at transaction index 0, while
    // `eth_call` simulates transactions "on top" of the block (i.e. after the
    // last transaction index). Therefore, in order for the Tenderly simulation
    // to be as close as possible to the `eth_call`, we want to create it on the
    // next block (since `block_N{tx_last} ~= block_(N+1){tx_0}`).
    let next_block = current_block + 1;
    let gas_price = gas_price
        .map(|gas_price| U256::from_f64_lossy(gas_price.effective_gas_price()))
        .unwrap_or_default();
    let link = format!(
        "https://dashboard.tenderly.co/gp-v2/staging/simulator/new?block={}&blockIndex=0&from={:#x}&gas=8000000&gasPrice={}&value=0&contractAddress={:#x}&network={}&rawFunctionInput=0x{}",
        next_block,
        tx.from.unwrap().address(),
        gas_price,
        tx.to.unwrap(),
        network_id,
        hex::encode(tx.data.unwrap().0)
    );
    if let Some(access_list) = access_list {
        let access_list = access_list
            .into_iter()
            .map(|item| {
                (
                    format!("0x{:x}", item.address),
                    item.storage_keys
                        .into_iter()
                        .map(|key| format!("0x{key:x}"))
                        .collect_vec(),
                )
            })
            .collect_vec();
        format!(
            "{link}&accessList={}",
            serde_json::to_string(&access_list).unwrap()
        )
    } else {
        link
    }
}

#[cfg(test)]
mod tests {
    use {
        super::*,
        crate::settlement::Settlement,
        ethcontract::{Account, PrivateKey},
        shared::{
            ethrpc::create_env_test_transport,
            http_solver::model::InternalizationStrategy,
            tenderly_api::TenderlyHttpApi,
        },
        std::str::FromStr,
    };

    // cargo test -p solver settlement_simulation::tests::mainnet -- --ignored
    // --nocapture
    #[tokio::test]
    #[ignore]
    async fn mainnet() {
        // Create some bogus settlements to see that the simulation returns an error.
        observe::tracing::initialize(
            "info,solver=debug,shared=debug,shared::transport=trace",
            tracing::Level::ERROR.into(),
        );
        let transport = create_env_test_transport();
        let web3 = Web3::new(transport);
        let block = web3.eth().block_number().await.unwrap().as_u64();
        let network_id = web3.net().version().await.unwrap();
        let contract = GPv2Settlement::deployed(&web3).await.unwrap();
        let account = Account::Offline(PrivateKey::from_raw([1; 32]).unwrap(), None);

        let settlements = vec![
            (
                account.clone(),
                Settlement::with_trades(Default::default(), vec![Default::default()])
                    .encode(InternalizationStrategy::SkipInternalizableInteraction),
                None,
            ),
            (
                account.clone(),
                Settlement::new(Default::default())
                    .encode(InternalizationStrategy::SkipInternalizableInteraction),
                None,
            ),
        ];
        let result = simulate_and_error_with_tenderly_link(
            settlements.iter().cloned(),
            &contract,
            &web3,
            Default::default(),
            network_id.as_str(),
            block,
            15000000u128,
        )
        .await;
        let _ = dbg!(result);

        let result = simulate_and_estimate_gas_at_current_block(
            settlements.iter().cloned(),
            &contract,
            Default::default(),
        )
        .await
        .unwrap();
        let _ = dbg!(result);

        let result = simulate_and_estimate_gas_at_current_block(
            std::iter::empty(),
            &contract,
            Default::default(),
        )
        .await
        .unwrap();
        let _ = dbg!(result);
    }

    // cargo test -p solver settlement_simulation::tests::mainnet_chunked --
    // --ignored --nocapture
    #[tokio::test]
    #[ignore]
    async fn mainnet_chunked() {
        observe::tracing::initialize(
            "info,solver=debug,shared=debug,shared::transport=trace",
            tracing::Level::ERROR.into(),
        );
        let transport = create_env_test_transport();
        let web3 = Web3::new(transport);
        let contract = GPv2Settlement::deployed(&web3).await.unwrap();
        let account = Account::Offline(PrivateKey::from_raw([1; 32]).unwrap(), None);

        // 12 so that we hit more than one chunk.
        let settlements = vec![
            (
                account.clone(),
                Settlement::new(Default::default())
                    .encode(InternalizationStrategy::SkipInternalizableInteraction),
                None
            );
            SIMULATE_BATCH_SIZE + 2
        ];
        let result = simulate_and_estimate_gas_at_current_block(
            settlements.iter().cloned(),
            &contract,
            GasPrice1559::default(),
        )
        .await
        .unwrap();
        let _ = dbg!(result);
    }

    #[tokio::test]
    #[ignore]
    async fn simulate_before_after_access_list_test() {
        let transport = create_env_test_transport();
        let web3 = Web3::new(transport);
        let transaction_hash =
            H256::from_str("e337fcd52afd6b98847baab279cda6c3980fcb185da9e959fd489ffd210eac60")
                .unwrap();
        let tenderly_api = TenderlyHttpApi::test_from_env();
        let gas_saved = simulate_before_after_access_list(
            &web3,
            &*tenderly_api,
            "1".to_string(),
            transaction_hash,
        )
        .await
        .unwrap();

        dbg!(gas_saved);
    }

    #[test]
    fn calldata_works() {
        let settlement = EncodedSettlement::default();
        let data = call_data(settlement);
        assert!(!data.is_empty());
    }
}
