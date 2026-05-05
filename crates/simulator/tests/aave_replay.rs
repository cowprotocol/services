use {
    alloy_primitives::{U256, address, hex},
    app_data::{AppDataHash, hash_full_app_data},
    model::{
        order::{BuyTokenDestination, OrderData, OrderKind, SellTokenSource},
        signature::Signature,
    },
    simulator::simulation_builder::{
        self,
        Block,
        EthCallInputs,
        ExecutionAmount,
        PriceEncoding,
        SettlementSimulator,
        Solver,
    },
    std::{str::FromStr, sync::Arc},
};

/// Full `app_data` JSON the trader signed for the replayed Aave v3 debt-swap
/// order. Source-level whitespace is for readability only - run through
/// `canonicalise_app_data` before hashing or passing downstream.
const APP_DATA: &str = r#"{
    "appCode": "aave-v3-interface-debt-swap",
    "metadata": {
        "flashloan": {
            "amount": "4475596734006878742",
            "liquidityProvider": "0x87870Bca3F3fD6335C3F4ce8392D69350B4fA4E2",
            "protocolAdapter": "0xdeCC46a4b09162F5369c5C80383AAa9159bCf192",
            "receiver": "0xdeCC46a4b09162F5369c5C80383AAa9159bCf192",
            "token": "0xc02aaa39b223fe8d0a0e5c4f27ead9083c756cc2"
        },
        "hooks": {
            "post": [{
                "callData": "0xad3da559000000000000000000000000000000000000000000000000444d51cbc68377680000000000000000000000000000000000000000000000000000000069f323f8000000000000000000000000000000000000000000000000000000000000001c445675473b3e0941842eb5405ec1d9cb93c7d64b513b30d928d7ea42067440cb00fbafa80085d754964d5d70f616d899049f4e75c240fda7c2f108a99c882e8b",
                "dappId": "cow-sdk://flashloans/aave/v3/debt-swap",
                "gasLimit": "1000000",
                "target": "0xe58aCB86761699c1cBC665e6b7E0271503f6336C"
            }],
            "pre": [{
                "callData": "0xb1b6308b00000000000000000000000073e7af13ef172f13d8fefebfd90c7a65300963440000000000000000000000006276ac03090f2bb8be680178343ac368f713b4e8000000000000000000000000e58acb86761699c1cbc665e6b7e0271503f6336c000000000000000000000000c02aaa39b223fe8d0a0e5c4f27ead9083c756cc200000000000000000000000040d16fc0246ad3160ccc09b8d0d3a2cd28ae6c2f0000000000000000000000000000000000000000000000003e14904047a25ee600000000000000000000000000000000000000000000021e4382edd5a86c00006ed88e868af0a1983e3886d5f3e95a2fafbd6c3450bc229e27342283dc429ccc0000000000000000000000000000000000000000000000000000000069f323f80000000000000000000000000000000000000000000000003e1c83845060e2160000000000000000000000000000000000000000000000000007f34408be83300000000000000000000000000000000000000000000000003e1c83845060e21600000000000000000000000000000000000000000000021e4382edd5a86c0000",
                "dappId": "cow-sdk://flashloans/aave/v3/debt-swap",
                "gasLimit": "300000",
                "target": "0xdeCC46a4b09162F5369c5C80383AAa9159bCf192"
            }]
        },
        "orderClass": {"orderClass": "market"},
        "partnerFee": {"recipient": "0x464C71f6c2F760DdA6093dCB91C24c39e5d6e18c", "volumeBps": 0},
        "quote": {"slippageBips": 140, "smartSlippage": true},
        "utm": {
            "utmCampaign": "developer-cohort",
            "utmContent": "",
            "utmMedium": "cow-sdk@7.3.4",
            "utmSource": "cowmunity",
            "utmTerm": "js"
        }
    },
    "version": "1.14.0"
}"#;

/// Returns `app_data` minified with keys sorted alphabetically. The output
/// matches the signed production bytes byte-for-byte because that payload
/// is already alphabetically keyed at every level.
fn canonicalise_app_data(app_data: &str) -> String {
    let value: serde_json::Value =
        serde_json::from_str(app_data).expect("APP_DATA must be valid JSON");
    serde_json::to_string(&value).expect("re-serialising must succeed")
}

/// Production EIP-1271 signature blob for the replayed order. The trader's
/// signer contract decodes it and validates against the order hash.
const SIGNATURE_HEX: &str = "0x000000000000000000000000c02aaa39b223fe8d0a0e5c4f27ead9083c756cc200000000000000000000000040d16fc0246ad3160ccc09b8d0d3a2cd28ae6c2f000000000000000000000000e58acb86761699c1cbc665e6b7e0271503f6336c0000000000000000000000000000000000000000000000003e14904047a25ee600000000000000000000000000000000000000000000021e4382edd5a86c00000000000000000000000000000000000000000000000000000000000069f323f8a1435054976e030f531f620f051bbabe34ef387901808b8677cf7c9304c21f3c00000000000000000000000000000000000000000000000000000000000000006ed88e868af0a1983e3886d5f3e95a2fafbd6c3450bc229e27342283dc429ccc00000000000000000000000000000000000000000000000000000000000000005a28e9363bb942b639270062aa6bb295f434bcdfc42c97267bf003f272060dc95a28e9363bb942b639270062aa6bb295f434bcdfc42c97267bf003f272060dc900000000000000000000000000000000000000000000000000000000000001a00000000000000000000000000000000000000000000000000000000000000041bb5488854dd5149f8843514851b7e25499917ca742af77061d2355681f3b608157bb34a59f9632e2228ea869c6d571f822295ee2eb03904dd8dd874245478f3b1b00000000000000000000000000000000000000000000000000000000000000";

/// Replay of a real Aave v3 debt-swap order against a historical mainnet
/// block, exercising the prototype's `SettlementSimulator`-based simulation
/// path end-to-end without involving the orderbook's wall-clock validity
/// check.
///
/// Why this test exists:
///
/// - A full `OrderValidator::validate_and_construct_order` flow uses
///   `SystemTime::now()` to bound `valid_to`, which makes any historical order
///   replay non-deterministic (the test rots as the order expires).
/// - The prototype's value lives in the simulation, not the validity check, so
///   we exercise the simulation directly: build a `SettlementSimulator` against
///   a real RPC, pin the simulation to the block right before settlement, and
///   assert it does not revert.
///
/// Order replayed: an `aave-v3-interface-debt-swap` order
/// `0x7f5df255b55f5eba3034f74acb8e91a04aaf61a755b88c61ad7c61068856f3b2e58acb86761699c1cbc665e6b7e0271503f6336c69f323f8`,
/// sell WETH, buy GHO. The owner is an EIP-1167 minimal proxy that the
/// pre-hook deploys just-in-time.
#[tokio::test]
#[ignore]
async fn aave_debt_swap_replay() {
    let Ok(rpc_url) = std::env::var("MAINNET_RPC_URL") else {
        eprintln!("MAINNET_RPC_URL not set - skipping replay test");
        return;
    };

    let canonical_app_data = canonicalise_app_data(APP_DATA);
    let inputs = build_replay_simulation(&rpc_url, &canonical_app_data).await;

    inputs
        .simulate()
        .await
        .expect("simulation must not revert for a healthy production order");
}

/// Same order, but the `flashloan.amount` in `app_data` is rewritten to a
/// value Aave's WETH pool cannot satisfy. The wrapper call to the Aave Pool
/// must revert, and the simulation must propagate that revert.
///
/// This proves the prototype actually executes the flashloan path: if the
/// wrapper call were a silent no-op (e.g. wrong router address), the
/// simulation would not depend on Aave's liquidity at all and would not
/// fail here.
#[tokio::test]
#[ignore]
async fn aave_debt_swap_replay_fails_when_flashloan_oversubscribed() {
    let Ok(rpc_url) = std::env::var("MAINNET_RPC_URL") else {
        eprintln!("MAINNET_RPC_URL not set - skipping replay test");
        return;
    };

    let mut value: serde_json::Value =
        serde_json::from_str(APP_DATA).expect("APP_DATA must be valid JSON");
    // Way more WETH than Aave can lend. Aave reverts with insufficient
    // liquidity (or similar) before any settlement runs.
    value["metadata"]["flashloan"]["amount"] = serde_json::Value::String(U256::MAX.to_string());
    let tampered_app_data = serde_json::to_string(&value).unwrap();

    let inputs = build_replay_simulation(&rpc_url, &tampered_app_data).await;

    let err = inputs
        .simulate()
        .await
        .expect_err("simulation must revert when the flashloan exceeds Aave liquidity");
    let msg = format!("{err:?}");
    assert!(
        msg.contains("execution reverted"),
        "expected an EVM revert, got: {msg}",
    );
}

/// Builds a simulation pinned to the block right before the Aave debt-swap
/// settlement. The caller controls `full_app_data` so the same wiring
/// supports a positive replay (untouched) and a negative replay (tampered).
async fn build_replay_simulation(rpc_url: &str, full_app_data: &str) -> EthCallInputs {
    // One block before the on-chain settlement transaction. At this block
    // the helper-clone owner contract has no code yet (the pre-hook deploys
    // it), the protocol-adapter factory is live, and Aave v3 has WETH
    // liquidity.
    let fork_block_mainnet = 24_992_051u64;
    let order_owner = address!("e58aCB86761699c1cBC665e6b7E0271503f6336C");
    let sell_token_weth = address!("c02aaa39b223fe8d0a0e5c4f27ead9083c756cc2");
    let buy_token_gho = address!("40d16fc0246ad3160ccc09b8d0d3a2cd28ae6c2f");
    let sell_amount = U256::from_str("4473358935639875302").unwrap();
    let buy_amount = U256::from_str("10003000000000000000000").unwrap();
    let valid_to = 1_777_542_136u32; // 2026-04-30 09:42:16 UTC

    let web3 = ethrpc::Web3::new_from_url(rpc_url);
    let provider = web3.provider.clone();
    let chain_id = 1u64;

    let settlement = contracts::GPv2Settlement::Instance::deployed(&provider)
        .await
        .expect("settlement contract not deployed on mainnet?");

    let flash_loan_router = contracts::FlashLoanRouter::deployment_address(&chain_id)
        .expect("FlashLoanRouter deployment address");
    let hooks_trampoline = contracts::HooksTrampoline::deployment_address(&chain_id)
        .expect("HooksTrampoline deployment address");

    let balance_overrider = Arc::new(balance_overrides::BalanceOverrides::new(web3));
    let block_stream = ethrpc::block_stream::mock_single_block(Default::default());

    let simulator = SettlementSimulator::new(
        settlement,
        flash_loan_router,
        hooks_trampoline,
        sell_token_weth,
        30_000_000u64,
        balance_overrider,
        block_stream,
        None,
    )
    .await
    .expect("failed to create SettlementSimulator");

    let signature_bytes = hex::decode(SIGNATURE_HEX.trim_start_matches("0x"))
        .expect("SIGNATURE_HEX must be valid hex");
    let app_data_hash = AppDataHash(hash_full_app_data(full_app_data.as_bytes()));

    let order_data = OrderData {
        sell_token: sell_token_weth,
        buy_token: buy_token_gho,
        receiver: Some(order_owner),
        sell_amount,
        buy_amount,
        valid_to,
        app_data: app_data_hash,
        fee_amount: U256::ZERO,
        kind: OrderKind::Buy,
        partially_fillable: false,
        sell_token_balance: SellTokenSource::Erc20,
        buy_token_balance: BuyTokenDestination::Erc20,
    };

    simulator
        .new_simulation_builder()
        .with_orders([simulation_builder::Order::new(order_data)
            .with_signature(order_owner, Signature::Eip1271(signature_bytes))
            .fill_at(ExecutionAmount::Full, PriceEncoding::LimitPrice)])
        .parameters_from_app_data(full_app_data)
        .expect("parameters_from_app_data should parse the app data")
        .from_solver(Solver::Fake(None))
        .provide_sufficient_buy_tokens()
        .at_block(Block::Number(fork_block_mainnet))
        .build()
        .await
        .expect("failed to build simulation")
}
