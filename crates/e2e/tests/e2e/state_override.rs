use {
    alloy::{
        primitives::{Address, B256, U256},
        rpc::types::state::{AccountOverride, StateOverride},
        sol,
    },
    chain::Chain,
    e2e::setup::run_test,
    ethrpc::Web3,
    gas_price_estimation::FakeGasPriceEstimator,
    simulator::Ethereum,
    std::sync::Arc,
};

sol! {
    #[sol(rpc, bytecode="6080604052348015600e575f5ffd5b506101238061001c5f395ff3fe6080604052348015600e575f5ffd5b50600436106026575f3560e01c80637a0ebc8814602a575b5f5ffd5b60306032565b005b5f5f5490505f5f1b81036078576040517f08c379a0000000000000000000000000000000000000000000000000000000008152600401606f9060d1565b60405180910390fd5b50565b5f82825260208201905092915050565b7f736c6f74207a65726f00000000000000000000000000000000000000000000005f82015250565b5f60bd600983607b565b915060c682608b565b602082019050919050565b5f6020820190508181035f83015260e68160b3565b905091905056fea2646970667358221220538cd705c3b870d0412519ce867cae3b5484bd1b0acf57a8ec779199e45097ec64736f6c634300081c0033")]
    contract Gate {
        function gate() { /*revert if slot_0 is 0*/ }
    }
}

fn unlock_override(contract: Address) -> StateOverride {
    let mut override_map = StateOverride::default();
    override_map.insert(
        contract,
        AccountOverride::default().with_state_diff([(B256::ZERO, B256::with_last_byte(1))]),
    );
    override_map
}

async fn ethereum(web3: Web3) -> Ethereum {
    let block_stream = ethrpc::block_stream::mock_single_block(Default::default());
    Ethereum::new(
        web3,
        Chain::Mainnet,
        configs::simulator::Addresses::default(),
        Arc::new(FakeGasPriceEstimator::default()),
        block_stream,
        U256::from(30_000_000),
    )
}

#[tokio::test]
#[ignore]
async fn local_node_estimate_gas_state_override() {
    run_test(estimate_gas_state_override).await;
}

async fn estimate_gas_state_override(web3: Web3) {
    let gated_contract = Gate::GateInstance::deploy(web3.provider.clone())
        .await
        .unwrap();

    let eth = ethereum(web3).await;

    let gated_call = gated_contract.gate().into_transaction_request();
    let without = eth.estimate_gas(gated_call.clone(), None).await;
    assert!(
        without.is_err(),
        "gate() should revert without the slot-0 override, got {without:?}",
    );

    let with = eth
        .estimate_gas(gated_call, Some(unlock_override(*gated_contract.address())))
        .await;
    assert!(
        with.is_ok(),
        "gate() should succeed with the slot-0 override, got {with:?}",
    );
}
