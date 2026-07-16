use {
    alloy::{
        primitives::{Address, B256, Bytes, TxKind, U256, b256, bytes, map::B256Map},
        providers::Provider,
        rpc::types::{
            TransactionRequest,
            state::{AccountOverride, StateOverride},
        },
    },
    chain::Chain,
    e2e::setup::run_test,
    ethrpc::{AlloyProvider, Web3},
    gas_price_estimation::FakeGasPriceEstimator,
    simulator::Ethereum,
    std::sync::Arc,
};

/// Runtime bytecode of a contract that reverts in `gate()` (selector
/// `0x7a0ebc88`) unless storage slot 0 is nonzero. Compiled with forge.
const SLOT_GATE_INIT: Bytes = bytes!("6080604052348015600e575f5ffd5b506101238061001c5f395ff3fe6080604052348015600e575f5ffd5b50600436106026575f3560e01c80637a0ebc8814602a575b5f5ffd5b60306032565b005b5f5f5490505f5f1b81036078576040517f08c379a0000000000000000000000000000000000000000000000000000000008152600401606f9060d1565b60405180910390fd5b50565b5f82825260208201905092915050565b7f736c6f74207a65726f00000000000000000000000000000000000000000000005f82015250565b5f60bd600983607b565b915060c682608b565b602082019050919050565b5f6020820190508181035f83015260e68160b3565b905091905056fea2646970667358221220538cd705c3b870d0412519ce867cae3b5484bd1b0acf57a8ec779199e45097ec64736f6c634300081c0033");
const GATE_SELECTOR: Bytes = bytes!("7a0ebc88");
const SLOT_0: B256 = b256!("0x0000000000000000000000000000000000000000000000000000000000000000");

async fn deploy(provider: &AlloyProvider) -> Address {
    let tx = TransactionRequest {
        input: SLOT_GATE_INIT.into(),
        gas: Some(0x200000),
        to: Some(TxKind::Create),
        ..Default::default()
    };
    let pending = provider.send_transaction(tx).await.unwrap();
    let receipt = pending.get_receipt().await.unwrap();
    receipt.contract_address.unwrap()
}

fn gate_call(to: Address) -> TransactionRequest {
    TransactionRequest {
        to: Some(to.into()),
        input: GATE_SELECTOR.into(),
        ..Default::default()
    }
}

fn unlock_override(contract: Address) -> StateOverride {
    let mut override_map = StateOverride::default();
    let mut diff = B256Map::default();
    diff.insert(SLOT_0, B256::with_last_byte(1));
    override_map.insert(
        contract,
        AccountOverride {
            state_diff: Some(diff),
            ..Default::default()
        },
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
    let gated_contract = deploy(&web3.provider).await;
    let eth = ethereum(web3).await;

    let without = eth.estimate_gas(gate_call(gated_contract), None).await;
    assert!(
        without.is_err(),
        "gate() should revert without the slot-0 override, got {without:?}",
    );

    let with = eth
        .estimate_gas(
            gate_call(gated_contract),
            Some(unlock_override(gated_contract)),
        )
        .await;
    assert!(
        with.is_ok(),
        "gate() should succeed with the slot-0 override, got {with:?}",
    );
}
