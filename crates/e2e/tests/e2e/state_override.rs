use {
    alloy::{
        primitives::{Address, B256, U256, address, hex, map::B256Map},
        providers::{Provider, ProviderBuilder},
        rpc::types::{
            TransactionRequest,
            state::{AccountOverride, StateOverride},
        },
    },
    chain::Chain,
    e2e::nodes::{NODE_HOST, Node},
    ethrpc::Web3,
    gas_price_estimation::FakeGasPriceEstimator,
    simulator::Ethereum,
    std::{str::FromStr, sync::Arc},
};

/// Runtime bytecode of a contract that reverts in `gate()` (selector
/// `0x7a0ebc88`) unless storage slot 0 is nonzero. Compiled with forge.
const SLOT_GATE_INIT: &str = "0x6080604052348015600e575f5ffd5b506101238061001c5f395ff3fe6080604052348015600e575f5ffd5b50600436106026575f3560e01c80637a0ebc8814602a575b5f5ffd5b60306032565b005b5f5f5490505f5f1b81036078576040517f08c379a0000000000000000000000000000000000000000000000000000000008152600401606f9060d1565b60405180910390fd5b50565b5f82825260208201905092915050565b7f736c6f74207a65726f00000000000000000000000000000000000000000000005f82015250565b5f60bd600983607b565b915060c682608b565b602082019050919050565b5f6020820190508181035f83015260e68160b3565b905091905056fea2646970667358221220538cd705c3b870d0412519ce867cae3b5484bd1b0acf57a8ec779199e45097ec64736f6c634300081c0033";
const GATE_SELECTOR: &str = "0x7a0ebc88";
const DEFAULT_SENDER: Address = address!("f39Fd6e51aad88F6F4ce6aB8827279cffFb92266");
const SLOT_0: &str = "0x0000000000000000000000000000000000000000000000000000000000000000";

async fn deploy() -> Address {
    let provider = ProviderBuilder::new().connect_http(NODE_HOST.parse().unwrap());
    let tx = TransactionRequest {
        from: Some(DEFAULT_SENDER),
        input: hex::decode(SLOT_GATE_INIT.trim_start_matches("0x"))
            .unwrap()
            .into(),
        gas: Some(0x200000),
        ..Default::default()
    };
    let pending = provider.send_transaction(tx).await.unwrap();
    let receipt = pending.get_receipt().await.unwrap();
    receipt.contract_address.unwrap()
}

fn gate_call(to: Address) -> TransactionRequest {
    TransactionRequest {
        from: Some(DEFAULT_SENDER),
        to: Some(to.into()),
        input: hex::decode(GATE_SELECTOR.trim_start_matches("0x"))
            .unwrap()
            .into(),
        ..Default::default()
    }
}

fn slot0_override(contract: Address) -> StateOverride {
    let mut override_map = StateOverride::default();
    let mut diff = B256Map::default();
    diff.insert(B256::from_str(SLOT_0).unwrap(), B256::from(U256::from(1)));
    override_map.insert(
        contract,
        AccountOverride {
            state_diff: Some(diff),
            ..Default::default()
        },
    );
    override_map
}

async fn ethereum() -> Ethereum {
    let web3 = Web3::new_from_url(NODE_HOST);
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
#[ignore = "requires anvil; run via `just test-e2e local_node_estimate_gas_state_override`"]
async fn local_node_estimate_gas_state_override() {
    let mut node = Node::new().await;
    let contract = deploy().await;
    let eth = ethereum().await;

    let without = eth.estimate_gas(gate_call(contract), None).await;
    assert!(
        without.is_err(),
        "gate() should revert without the slot-0 override, got {without:?}",
    );

    let with = eth
        .estimate_gas(gate_call(contract), Some(slot0_override(contract)))
        .await;
    assert!(
        with.is_ok(),
        "gate() should succeed with the slot-0 override, got {with:?}",
    );

    node.kill().await;
}
