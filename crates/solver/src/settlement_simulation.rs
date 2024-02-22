use {
    anyhow::Result,
    contracts::GPv2Settlement,
    ethcontract::{
        contract::MethodBuilder,
        dyns::{DynMethodBuilder, DynTransport},
        errors::ExecutionError,
        transaction::TransactionBuilder,
        Account,
    },
    gas_estimation::GasPrice1559,
    itertools::Itertools,
    primitive_types::U256,
    shared::{conversions::into_gas_price, encoded_settlement::EncodedSettlement},
    web3::types::AccessList,
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

fn settle_method(
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
        shared::{ethrpc::create_env_test_transport, http_solver::model::InternalizationStrategy},
        web3::Web3,
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
}
